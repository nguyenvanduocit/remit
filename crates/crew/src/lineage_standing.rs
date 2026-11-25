//! A lineage's standing in a kind of work (ADR-0069): what the outside said of the work of its employees that their Chair
//! accepted, read only from signals no agent of that work writes (its change landed, a landing taken back by the forge's
//! pipeline, a guardrail or the owner, the goal judge's latest verdict, a reading key result holding), with the duel rating
//! (duel_rating.rs) deciding only between lineages the outside leaves in one band. The rules are pure (`standings_of`,
//! `unsettled_why`, `outsides_of`); the shell reads the record once (`standings_all`).

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use model::{
    Approval, Card, Chain, ChainStatus, CheckKind, DecisionKind, Duel, FindingStatus, KRStatus, KeyResult, Landing,
    LandingState, RevertCause, Serves, SignalKey, Signs, Time, Verdict, VerdictRole,
};

use crate::Crew;
use crate::duel_rating::{EVEN_RATING, lineage_ratings};
use crate::objective::reading_status;
use chains::Result;
use chains::chain_core::optional;
use chains::health::elapsed;
use wire::app::{LineageStanding, OutsideEvidence, OutsideResult};

/// The fewest chains the outside must have decided on for a lineage's score to place it: under it, the lineage stands in
/// the even band with every other unknown one, and duels decide.
pub(crate) const MIN_OUTSIDE: u32 = 3;

/// How many points of score make one band: lineages whose scores round to the same band are told apart by duels.
const STANDING_BAND: i64 = 10;

/// How far back the outside's results are read: a lineage stands by how it works now.
pub(crate) const STANDING_WINDOW: Duration = Duration::from_secs(90 * 24 * 60 * 60);

/// x rounded half away from zero, as an integer.
#[expect(
    clippy::as_conversions,
    reason = "no From<f64> for i64; the values rounded here are scores and ratings, small whole numbers a float holds exactly"
)]
pub(crate) fn rounded(x: f64) -> i64 {
    x.round() as i64
}

/// The band s stands in: its score rounded to a band, or the even band while it is not settled.
fn band(s: &LineageStanding) -> i64 {
    if !s.settled {
        return 50;
    }
    // a score is 0 to 100: rounded half up to its band, as `rounded` does a non-negative one
    s.score
        .saturating_add(STANDING_BAND / 2)
        .div_euclid(STANDING_BAND)
        .saturating_mul(STANDING_BAND)
}

/// How many of s's chains the outside decided on.
fn decided(s: &LineageStanding) -> u32 {
    s.held.saturating_add(s.corrected).saturating_add(s.failed)
}

/// Orders standings best first: by band, then by duel rating (unrated reads as even), then by score. A lineage with no
/// standing is the default `LineageStanding`: even band, even rating.
pub(crate) fn rank_order(a: &LineageStanding, b: &LineageStanding) -> Ordering {
    let rating = |s: &LineageStanding| if s.duel.rating == 0 { EVEN_RATING } else { s.duel.rating };
    band(b)
        .cmp(&band(a))
        .then_with(|| rating(b).cmp(&rating(a)))
        .then_with(|| b.score.cmp(&a.score))
}

/// `rank_order`, then by name.
fn standing_order(a: &LineageStanding, b: &LineageStanding) -> Ordering {
    rank_order(a, b).then_with(|| a.template.cmp(&b.template))
}

/// Standings by template.
pub fn by_template(standings: Vec<LineageStanding>) -> HashMap<String, LineageStanding> {
    standings.into_iter().map(|s| (s.template.clone(), s)).collect()
}

/// The standing as a Chair reads it beside an employee: the outside first, the duels after, and the newest failure.
pub fn standing_words(s: &LineageStanding) -> String {
    let mut words = format!("lineage {} in {}: ", s.template, s.work);
    if decided(s) == 0 {
        words.push_str("nothing decided outside yet");
    } else {
        words.push_str(&format!(
            "score {} of 100 from the outside ({} held, {} corrected by the owner, {} failed",
            s.score, s.held, s.corrected, s.failed
        ));
        if !s.settled {
            words.push_str(&format!("; under {MIN_OUTSIDE} decided, so read as even"));
        }
        words.push(')');
        if let Some(e) = s.evidence.iter().find(|e| e.result == OutsideResult::Failed) {
            words.push_str(&format!(", last failure: {} in chain {}", e.why, e.chain));
        }
    }
    if s.duel.duels > 0 {
        words.push_str(&format!(
            "; duels, which only break ties: rated {} over {}, won {}",
            s.duel.rating, s.duel.duels, s.duel.won
        ));
    }
    words
}

/// The standings of every lineage with an outside result in kind of work work within `STANDING_WINDOW` of now, or a
/// judged duel there, best first (`standing_order`). Pure.
pub(crate) fn standings_of(outsides: &[ChainOutside], duels: &[Duel], work: &str, now: Time) -> Vec<LineageStanding> {
    let mut by: BTreeMap<String, LineageStanding> = BTreeMap::new();
    let of = |by: &mut BTreeMap<String, LineageStanding>, t: &str| -> LineageStanding {
        by.remove(t).unwrap_or_else(|| LineageStanding {
            template: t.to_owned(),
            work: work.to_owned(),
            ..LineageStanding::default()
        })
    };
    for c in outsides {
        if c.work != work {
            continue;
        }
        let Some(e) = c.evidence() else { continue };
        if elapsed(e.at, now) > STANDING_WINDOW {
            continue;
        }
        for t in &c.lineages {
            let mut s = of(&mut by, t);
            match e.result {
                OutsideResult::Held => s.held = s.held.saturating_add(1),
                OutsideResult::Corrected => s.corrected = s.corrected.saturating_add(1),
                OutsideResult::Failed => s.failed = s.failed.saturating_add(1),
            }
            s.evidence.push(e.clone());
            by.insert(t.clone(), s);
        }
    }
    for (t, r) in lineage_ratings(duels, work) {
        let mut s = of(&mut by, &t);
        s.duel = r;
        by.insert(t, s);
    }
    let mut out: Vec<LineageStanding> = by
        .into_values()
        .map(|mut s| {
            let decided = decided(&s);
            // One upheld and one failed chain stand in for what is not known yet, as the duel rating's even reference
            // does.
            s.score =
                rounded(100.0 * (f64::from(s.held) + f64::from(s.corrected) / 2.0 + 1.0) / (f64::from(decided) + 2.0));
            s.settled = decided >= MIN_OUTSIDE;
            s.evidence
                .sort_by(|a, b| b.at.cmp(&a.at).then_with(|| a.chain.cmp(&b.chain)));
            s
        })
        .collect();
    out.sort_by(standing_order);
    out
}

/// Why kind of work work is not settled, none once it is: its two best lineages (standings, best first) are each settled
/// and stand in different bands. A hire in an unsettled kind of work runs on two lineages (`duel_by_default`). Pure.
pub(crate) fn unsettled_why(work: &str, standings: &[LineageStanding]) -> Option<String> {
    let [first, second, ..] = standings else {
        return Some(format!("fewer than two lineages have a standing in {work}"));
    };
    if !first.settled || !second.settled {
        return Some(format!(
            "in {work}, {} and {} are not both settled: the outside decided on fewer than {MIN_OUTSIDE} of the chains of one of them",
            first.template, second.template
        ));
    }
    if band(first) == band(second) {
        return Some(format!(
            "in {work}, {} and {} stand in the same band (scores {} and {} of 100): only duels tell them apart",
            first.template, second.template, first.score, second.score
        ));
    }
    None
}

/// What the outside said of one chain: its kind of work, the lineages whose work its Chair accepted, the goal judge's
/// latest verdict (none before one), its landings, and how the reading key result it serves stands (none when it serves
/// none, or is not done).
#[derive(Debug, Clone, Default)]
pub(crate) struct ChainOutside {
    pub chain: Chain,
    pub work: String,
    pub lineages: Vec<String>,
    pub verdict: Option<Approval>,
    pub landings: Vec<Landing>,
    pub reading: Option<KRStatus>,
}

impl ChainOutside {
    /// What the outside said of the chain, none while it said nothing: failed when any signal went against it,
    /// corrected when the owner took a landing of it back, held when at least one signal was in its favour. Pure.
    pub(crate) fn evidence(&self) -> Option<OutsideEvidence> {
        let (mut good, mut bad) = (Vec::new(), Vec::new());
        let mut at = None;
        let mut overturned = Vec::new();
        for l in self.landings.iter().filter(|l| l.reverts.is_none()) {
            match &l.state {
                LandingState::Reverted { cause, why, .. } if *cause == RevertCause::Overturn => {
                    overturned.push(format!("{} landed, then the owner took it back: {why}", l.repo));
                    at = at.max(l.reverted_at);
                }
                LandingState::Reverted { why, .. } => {
                    bad.push(format!("{} landed, then was taken back: {why}", l.repo));
                    at = at.max(l.reverted_at);
                }
                LandingState::Landed => {
                    good.push(format!("{} landed", l.repo));
                    at = at.max(l.landed_at);
                }
                // a version on its way, or one the queue sent back to be fixed, says nothing yet
                _ => {}
            }
        }
        if let Some(v) = &self.verdict {
            if v.confirms() {
                good.push("the goal judge confirmed every key result".to_owned());
            } else if v.verdict != Verdict::Approve || v.findings.iter().any(|f| f.status == FindingStatus::NotMet) {
                bad.push("the goal judge found it not met".to_owned());
            }
            at = at.max(Some(v.at));
        }
        match self.reading {
            Some(KRStatus::Met) => good.push("the reading of the key result it serves holds".to_owned()),
            Some(KRStatus::NotMet) => bad.push("the reading of the key result it serves does not hold".to_owned()),
            // an unverified or unserved key result says nothing
            _ => {}
        }
        let at = at.unwrap_or(self.chain.updated_at);
        let (result, why) = if !bad.is_empty() {
            (OutsideResult::Failed, bad.join("; "))
        } else if !overturned.is_empty() {
            (OutsideResult::Corrected, overturned.join("; "))
        } else if !good.is_empty() {
            (OutsideResult::Held, good.join("; "))
        } else {
            return None;
        };
        Some(OutsideEvidence {
            chain: self.chain.id.clone(),
            result,
            why,
            at,
        })
    }
}

/// What the outside said of every chain in rec, of which it holds the Chairs' accept and rework decisions: works names
/// each chain's kind of work, readings how the reading key result a chain
/// serves stands. A lineage is in a chain when the Chair's newest word on a card of one of its employees, working (no
/// gate's or judge's card), accepted it. Pure.
pub(crate) fn outsides_of(
    rec: &store::Record,
    works: &HashMap<String, String>,
    readings: &HashMap<String, KRStatus>,
) -> Vec<ChainOutside> {
    // each card's newest accept or rework
    let mut word: HashMap<&str, &model::Decision> = HashMap::new();
    for d in &rec.decisions {
        if !matches!(d.kind, DecisionKind::Accept | DecisionKind::Rework) {
            continue;
        }
        if word
            .get(d.card_id.as_str())
            .is_none_or(|w| d.created_at >= w.created_at)
        {
            word.insert(&d.card_id, d);
        }
    }
    let accepted = |c: &Card| word.get(c.id.as_str()).is_some_and(|d| d.kind == DecisionKind::Accept);
    let mut lineages: HashMap<&str, Vec<String>> = HashMap::new();
    for c in &rec.cards {
        if c.employee.is_empty() || !c.on_team() || c.signs != Signs::None || !accepted(c) {
            continue;
        }
        let of = lineages.entry(&c.home).or_default();
        if !of.contains(&c.agent) {
            of.push(c.agent.clone());
        }
    }
    let mut verdicts: HashMap<&str, &Approval> = HashMap::new();
    for a in &rec.approvals {
        if a.role == VerdictRole::Judge && verdicts.get(a.chain_id.as_str()).is_none_or(|v| a.at >= v.at) {
            verdicts.insert(&a.chain_id, a);
        }
    }
    let mut landings: HashMap<&str, Vec<Landing>> = HashMap::new();
    for l in &rec.landings {
        landings.entry(&l.chain_id).or_default().push(l.clone());
    }
    rec.chains
        .iter()
        .filter_map(|ch| {
            let of = lineages.get(ch.id.as_str()).filter(|l| !l.is_empty())?;
            Some(ChainOutside {
                chain: ch.clone(),
                work: works.get(&ch.id).cloned().unwrap_or_default(),
                lineages: of.clone(),
                verdict: verdicts.get(ch.id.as_str()).map(|v| (*v).clone()),
                landings: landings.remove(ch.id.as_str()).unwrap_or_default(),
                reading: readings.get(&ch.id).cloned(),
            })
        })
        .collect()
}

/// How the key result serves names stands by its reading, none when it is no reading key result or is gone.
fn served_reading(
    r: &store::Reader<'_>,
    key_results: &HashMap<String, Vec<KeyResult>>,
    serves: &Serves,
    now: Time,
) -> Result<Option<KRStatus>> {
    let Some(kr) = key_results
        .get(&serves.objective)
        .and_then(|krs| krs.iter().find(|kr| kr.id == serves.key_result))
    else {
        return Ok(None);
    };
    let Some(c) = kr.check.as_ref().filter(|c| c.kind == CheckKind::Reading) else {
        return Ok(None);
    };
    let head = optional(r.last_reading(&SignalKey(c.signal.clone())))?;
    Ok(Some(reading_status(c, head.as_ref(), now)))
}

impl Crew {
    /// The kind of work chain ch does: its kind (ADR-0113).
    pub(crate) fn work_of(ch: &Chain) -> String {
        ch.kind.as_str().to_owned()
    }

    /// What standings are read from: what the outside said of every chain, and every duel.
    pub(crate) async fn standings_all(&self) -> Result<(Vec<ChainOutside>, Vec<Duel>)> {
        let rec = self
            .store
            .record(vec![DecisionKind::Accept, DecisionKind::Rework])
            .await?;
        let chains = rec.chains.clone();
        let now = self.now();
        let (works, readings, duels) = self
            .store
            .read(move |r| -> Result<_> {
                let key_results = r.every_key_result()?;
                let (mut works, mut readings) = (HashMap::new(), HashMap::new());
                for ch in &chains {
                    works.insert(ch.id.clone(), Self::work_of(ch));
                    if let Some(serves) = ch.serves.as_ref().filter(|_| *ch.status() == ChainStatus::Done)
                        && let Some(status) = served_reading(r, &key_results, serves, now)?
                    {
                        readings.insert(ch.id.clone(), status);
                    }
                }
                Ok((works, readings, r.duels()?))
            })
            .await?;
        Ok((outsides_of(&rec, &works, &readings), duels))
    }

    /// The standings of the lineages in the kind of work chain ch does, best first.
    pub async fn standings_in(&self, ch: &Chain) -> Result<Vec<LineageStanding>> {
        let work = Self::work_of(ch);
        let (outsides, duels) = self.standings_all().await?;
        Ok(standings_of(&outsides, &duels, &work, self.now()))
    }
}
