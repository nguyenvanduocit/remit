//! The rules of duels, pure: a lineage's rating in a kind of work and the judge's agreement with the owner, each computed
//! from the duels as stored, so an overturn moves both at once and nothing kept beside the duels can drift from them. The
//! rating only breaks ties between lineages the outside leaves in one band (lineage_standing.rs, ADR-0069).

use std::collections::BTreeMap;

use model::{Duel, Time};

use crate::lineage_standing::{ChainOutside, rounded, standings_of, unsettled_why};
use wire::app::{LineageRating, WorkDuels};

/// The rating of a lineage as strong as the reference every lineage is measured against.
pub(crate) const EVEN_RATING: i64 = 1500;

/// Bounds the fit; it settles well before this on any count of duels a kind of work holds.
const RATING_ROUNDS: usize = 200;

/// The ratings of every lineage that fought a judged duel in kind of work work, by template. The fit is Bradley–Terry by
/// minorization-maximization (Hunter 2004), each lineage also given one win and one loss against an even reference, so a
/// lineage that won or lost every duel still has a finite rating that the next duel moves.
pub(crate) fn lineage_ratings(duels: &[Duel], work: &str) -> BTreeMap<String, LineageRating> {
    let mut games: BTreeMap<(String, String), u32> = BTreeMap::new();
    let mut out: BTreeMap<String, LineageRating> = BTreeMap::new();
    for d in duels.iter().filter(|d| d.work == work && d.judged()) {
        let [first, second] = &d.sides;
        let (win, lose) = if d.winner == second.card {
            (&second.template, &first.template)
        } else {
            (&first.template, &second.template)
        };
        let w = out.entry(win.clone()).or_default();
        (w.duels, w.won) = (w.duels.saturating_add(1), w.won.saturating_add(1));
        let l = out.entry(lose.clone()).or_default();
        l.duels = l.duels.saturating_add(1);
        let pair = (win.min(lose).clone(), win.max(lose).clone());
        let n = games.entry(pair).or_default();
        *n = n.saturating_add(1);
    }
    let mut strength: BTreeMap<String, f64> = out.keys().map(|t| (t.clone(), 1.0)).collect();
    for _ in 0..RATING_ROUNDS {
        let mut next = BTreeMap::new();
        for (t, s) in &strength {
            let mut denom = 2.0 / (s + 1.0); // the reference's two games
            for ((a, b), n) in &games {
                let rival = if t == a {
                    b
                } else if t == b {
                    a
                } else {
                    continue;
                };
                denom += f64::from(*n) / (s + strength.get(rival).copied().unwrap_or(1.0));
            }
            let won = out.get(t).map_or(0, |r| r.won);
            next.insert(t.clone(), (f64::from(won) + 1.0) / denom);
        }
        strength = next;
    }
    for (t, r) in &mut out {
        r.template.clone_from(t);
        work.clone_into(&mut r.work);
        let s = strength.get(t).copied().unwrap_or(1.0);
        r.rating = EVEN_RATING.saturating_add(rounded(400.0 * s.log10()));
    }
    out
}

/// How many of a kind of work's newest judged duels the judge's agreement with the owner is read over: his verdict on the
/// judge as it judges now, not on its first picks.
const DUEL_AGREEMENT_WINDOW: usize = 10;

/// How the owner took the judge's picks in one kind of work: of the newest judged duels (at most `DUEL_AGREEMENT_WINDOW`),
/// how many he overturned.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Agreement {
    pub work: String,
    pub judged: u32,
    pub overturned: u32,
}

/// The judge's agreement with the owner in kind of work work, over its newest judged duels.
pub(crate) fn agreement_of(duels: &[Duel], work: &str) -> Agreement {
    let mut judged: Vec<&Duel> = duels.iter().filter(|d| d.work == work && d.judged()).collect();
    judged.sort_by_key(|d| std::cmp::Reverse(d.created_at)); // stable: the newest first
    let mut a = Agreement {
        work: work.to_owned(),
        ..Agreement::default()
    };
    for d in judged.into_iter().take(DUEL_AGREEMENT_WINDOW) {
        a.judged = a.judged.saturating_add(1);
        if !d.overturn.is_empty() {
            a.overturned = a.overturned.saturating_add(1);
        }
    }
    a
}

impl Agreement {
    /// The share of the judge's picks the owner left standing; 1 before any was judged.
    fn share(&self) -> f64 {
        if self.judged == 0 {
            return 1.0;
        }
        f64::from(self.judged.saturating_sub(self.overturned)) / f64::from(self.judged)
    }

    /// Why duels of the kind of work are paused, or none when they go on: the owner left fewer of the judge's picks
    /// standing than least, the share the coach's brief sets.
    pub(crate) fn paused_why(&self, least: f64) -> Option<String> {
        if self.share() >= least {
            return None;
        }
        Some(format!(
            "duels of {} are paused: the owner overturned {} of the judge's last {} picks there, so it agrees with him on {:.0}%, under the {:.0}% duels need. They go on when the coach sets a lower share",
            self.work,
            self.overturned,
            self.judged,
            100.0 * self.share(),
            100.0 * least
        ))
    }
}

/// Where lineages stand in every kind of work that has an outside result or a duel, by name, as of now, under least, the
/// share the coach's brief sets. Pure.
pub(crate) fn work_duels_of(outsides: &[ChainOutside], duels: &[Duel], least: f64, now: Time) -> Vec<WorkDuels> {
    let mut works: Vec<&str> = duels.iter().map(|d| d.work.as_str()).collect();
    works.extend(
        outsides
            .iter()
            .filter(|c| c.evidence().is_some())
            .map(|c| c.work.as_str()),
    );
    works.sort_unstable();
    works.dedup();
    works
        .into_iter()
        .filter_map(|w| {
            let standings = standings_of(outsides, duels, w, now);
            if standings.is_empty() {
                return None; // its only results are older than the window
            }
            let a = agreement_of(duels, w);
            Some(WorkDuels {
                work: w.to_owned(),
                unsettled: unsettled_why(w, &standings),
                judged: a.judged,
                overturned: a.overturned,
                paused: a.paused_why(least),
                standings,
            })
        })
        .collect()
}
