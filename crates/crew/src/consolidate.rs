//! Memory kept whole over time (ADR-0117): what grows by teaching alone grows duplicates, contradictions left standing,
//! many small lessons where one would say what they share, and claims no turn needs any more. Remit finds them, the
//! librarian settles them: each finding is a teaching of role review, under which it may only merge, abstract, link or
//! retract (librarian.rs). The findings are pure (`reviews`); the job that records them, measures recall
//! (recall_eval.rs), and the agent's word that a memory misled it are the shell.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use model::{Judgment, LearnRole, Memory, MemoryLinkKind, MemoryTrust, MemoryUse, Teaching, Time, by_agent};
use schemars::JsonSchema;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::Crew;
use crate::librarian::may_change;
use crate::memory::{EMBED_MODEL, quoted};
use crate::recall::cosine;
use chains::tools::{Effect, SelfCard, Tools};
use chains::{Error, Result};

/// The cosine at which two memories may say the same thing, under `EMBED_MODEL`: the rule the review joins duplicates by,
/// and the one `remember` falls back to when no decider judges a pair (memory_judge.rs). Measured on the owner's 45 memories
/// (2026-11-19): the three pairs at 0.84 or more say one thing twice or one inside the other ("record_verdict requires a
/// clean git worktree" and "Uncommitted changes in worktree block automated verdict recording", 0.915); the next, 0.815, are
/// two lessons about the same work. The librarian decides; this only asks.
pub(crate) const DUPLICATE_COSINE: f64 = 0.84;

/// How many memories one review of duplicates names at most.
const REVIEW_MAX: usize = 6;

/// How many active memories one reach holds about one subject before the librarian is asked what they share, and how many
/// one such review names at most.
const ABSTRACT_MIN: usize = 4;
const ABSTRACT_MAX: usize = 8;

/// How long a memory no turn was given (nor a search found) stands before the librarian is asked whether it still holds:
/// two seasons of work, long enough that a lesson of a rare task is not lost to a quiet month.
const UNUSED_DAYS: i64 = 180;

/// Why memories are reviewed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Finding {
    /// They may say the same thing.
    Duplicates,
    /// One contradicts the other by a link, and both are recalled.
    Contradiction,
    /// The deciders say one may contradict the other (`judged_contradictions`), no link says so, and both are recalled.
    PossibleContradiction,
    /// Agents said it misled them more often than it helped.
    Misled,
    /// Several are about one subject: what they share may be one memory that rests on them.
    Abstract,
    /// No turn was given it, and no search found it, for `UNUSED_DAYS`.
    Unused,
}

impl Finding {
    fn ask(self) -> &'static str {
        match self {
            Self::Duplicates => {
                "These memories may say the same thing. Join those that do into one (remember, superseding them), link those that only bear on each other (link_memories), and settle."
            }
            Self::Contradiction => {
                "These memories contradict each other and both are recalled. Decide which holds, from their evidence and sources: join them into the one that holds (remember, superseding both), or retract the one that does not."
            }
            Self::PossibleContradiction => {
                "These memories may contradict each other and both are recalled. If they do, link them as contradicting (link_memories, contradicts), then decide which holds, from their evidence and sources: join them into the one that holds (remember, superseding both), or retract the one that does not. If both can be followed together, link them (link_memories, related) and settle."
            }
            Self::Misled => {
                "Agents that were given this memory said it misled them more often than it helped. Read their words and its evidence: retract it if it is wrong or stale, or correct it (remember, superseding it)."
            }
            Self::Abstract => {
                "These memories are all about one subject. If two or more of them are cases of one rule or one procedure, write it (remember, a principle or a procedure, linking because to each case it rests on, superseding none): an agent then recalls the rule with its cases. If they share nothing more than the subject, settle and say so."
            }
            Self::Unused => {
                "No turn was given this memory, and no search found it, for half a year. Decide from its evidence whether it still holds: retract it if what it names is gone or it no longer bears on any work, or settle and keep it if it is a rare case worth keeping."
            }
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Duplicates => "duplicates",
            Self::Contradiction => "contradiction",
            Self::PossibleContradiction => "possible-contradiction",
            Self::Misled => "misled",
            Self::Abstract => "abstract",
            Self::Unused => "unused",
        }
    }
}

/// A set of memories to review, and why.
#[derive(Debug, Clone, PartialEq)]
struct Review {
    finding: Finding,
    memories: Vec<Memory>,
    /// What else names the finding, so it is asked again when it changes: how often one misled, the half-year one was
    /// found unused in.
    mark: String,
}

impl Review {
    fn of(finding: Finding, memories: Vec<Memory>) -> Self {
        Self {
            finding,
            memories,
            mark: String::new(),
        }
    }
}

/// Whether two memories may say the same thing, by their vectors under `EMBED_MODEL` (each of its passage, uninstructed):
/// what the review asks the librarian to join. Pure.
fn may_say_the_same(a: &[f32], b: &[f32]) -> bool {
    f64::from(cosine(a, b)) >= DUPLICATE_COSINE
}

/// Whether a and b could become one memory, or rest one on the other, under a review held by either: each is one the
/// other's holder may change in its project (`may_change`), so the librarian may supersede or link both. Pure.
fn joinable(a: &Memory, b: &Memory) -> bool {
    may_change(&a.holder, &a.project, b) && may_change(&b.holder, &b.project, a)
}

/// Whether a review held by m's holder may change m: what reviews other than a contradiction name. Pure.
fn reviewable(m: &Memory) -> bool {
    may_change(&m.holder, &m.project, m) && m.trust != MemoryTrust::Confirmed
}

/// The memories among active that may say the same thing: joinable, by `may_say_the_same`, grown into groups of
/// `REVIEW_MAX` at most. Pure.
fn duplicates(active: &[(Memory, Option<Vec<f32>>)]) -> Vec<Review> {
    let mut out = Vec::new();
    let mut grouped: HashSet<usize> = HashSet::new();
    for (i, (a, va)) in active.iter().enumerate() {
        let Some(va) = va else { continue };
        if grouped.contains(&i) {
            continue;
        }
        let mut group = vec![i];
        for (j, (b, vb)) in active.iter().enumerate().skip(i.saturating_add(1)) {
            let Some(vb) = vb else { continue };
            if group.len() < REVIEW_MAX && !grouped.contains(&j) && joinable(a, b) && may_say_the_same(va, vb) {
                group.push(j);
            }
        }
        if group.len() > 1 {
            grouped.extend(group.iter().copied());
            let memories = group
                .iter()
                .filter_map(|k| active.get(*k).map(|(m, _)| m.clone()))
                .collect();
            out.push(Review::of(Finding::Duplicates, memories));
        }
    }
    out
}

/// The subjects `ABSTRACT_MIN` or more reviewable memories one reach holds are about, each with them (`ABSTRACT_MAX` at
/// most, those taught again most first): none where a memory already rests (because) on two or more of them, which is
/// their abstraction. Pure.
fn abstractions(active: &[&Memory]) -> Vec<Review> {
    let mut by_subject: BTreeMap<&str, Vec<&Memory>> = BTreeMap::new();
    for m in active.iter().filter(|m| reviewable(m)) {
        for s in &m.about {
            by_subject.entry(s.as_str()).or_default().push(m);
        }
    }
    let mut out = Vec::new();
    let mut asked: HashSet<BTreeSet<&str>> = HashSet::new();
    for mems in by_subject.values() {
        let mut left: Vec<&Memory> = mems.clone();
        while let Some(first) = left.first().copied() {
            let (mut reach, rest): (Vec<&Memory>, Vec<&Memory>) =
                left.iter().partition(|m| m.id == first.id || joinable(first, m));
            left = rest;
            if reach.len() < ABSTRACT_MIN {
                continue;
            }
            let ids: HashSet<&str> = reach.iter().map(|m| m.id.as_str()).collect();
            let abstracted = active.iter().any(|m| {
                m.links
                    .iter()
                    .filter(|l| l.kind == MemoryLinkKind::Because && ids.contains(l.to.as_str()))
                    .count()
                    >= 2
            });
            if abstracted {
                continue;
            }
            reach.sort_by(|a, b| b.retaught.len().cmp(&a.retaught.len()).then_with(|| a.id.cmp(&b.id)));
            reach.truncate(ABSTRACT_MAX);
            if asked.insert(reach.iter().map(|m| m.id.as_str()).collect()) {
                out.push(Review::of(Finding::Abstract, reach.into_iter().cloned().collect()));
            }
        }
    }
    out
}

/// The pairs of active memories the deciders acted on as contradicting (`judged`: judgments of `memory.contradicts` still
/// unproved), each once and not where a link already says so (that is a review of its own). Pure.
fn judged_contradictions(active: &[&Memory], judged: &[Judgment]) -> Vec<Review> {
    let linked = |a: &Memory, b: &Memory| {
        [(a, b), (b, a)].iter().any(|(x, y)| {
            x.links
                .iter()
                .any(|l| l.kind == MemoryLinkKind::Contradicts && l.to == y.id)
        })
    };
    let mut seen: HashSet<(&str, &str)> = HashSet::new();
    let mut out = Vec::new();
    for (a, b) in judged.iter().filter_map(|j| j.subject.pair()) {
        let (Some(a), Some(b)) = (active.iter().find(|m| m.id == a), active.iter().find(|m| m.id == b)) else {
            continue;
        };
        if !linked(a, b) && seen.insert((a.id.as_str(), b.id.as_str())) {
            out.push(Review::of(
                Finding::PossibleContradiction,
                vec![(*a).clone(), (*b).clone()],
            ));
        }
    }
    out
}

/// What to review among the active memories, each with its vector when it has one, at now: those that may say the same
/// thing (`duplicates`), contradictions both sides of which stand, memories that misled more than they helped, subjects
/// several are about (`abstractions`), and memories unused for `UNUSED_DAYS`. Pure.
fn reviews(active: &[(Memory, Option<Vec<f32>>)], judged: &[Judgment], now: Time) -> Vec<Review> {
    let mut out = duplicates(active);
    let mems: Vec<&Memory> = active.iter().map(|(m, _)| m).collect();
    let unused_before = Time(now.0.saturating_sub(time::Duration::days(UNUSED_DAYS)));
    let half_year = now
        .0
        .unix_timestamp()
        .checked_div(time::Duration::days(UNUSED_DAYS).whole_seconds())
        .unwrap_or_default();
    for m in &mems {
        for l in m.links.iter().filter(|l| l.kind == MemoryLinkKind::Contradicts) {
            if let Some(other) = mems.iter().find(|o| o.id == l.to) {
                out.push(Review::of(Finding::Contradiction, vec![(*m).clone(), (*other).clone()]));
            }
        }
        if m.misled > m.helped && reviewable(m) {
            out.push(Review {
                mark: m.misled.to_string(),
                ..Review::of(Finding::Misled, vec![(*m).clone()])
            });
        }
        let last_used = m.last_recalled_at.unwrap_or(m.created_at);
        if last_used < unused_before && reviewable(m) {
            out.push(Review {
                mark: half_year.to_string(),
                ..Review::of(Finding::Unused, vec![(*m).clone()])
            });
        }
    }
    out.extend(judged_contradictions(&mems, judged));
    out.extend(abstractions(&mems));
    out
}

/// The teaching that asks the librarian for r, at now: held by its first memory's holder, in its project. Its id names
/// the finding, the memories and its mark, so a review settled once is not asked again until something changes. What it
/// says of each memory is escaped (`quoted`), so no memory's words close its frame. Pure.
fn review_teaching(r: &Review, now: Time) -> Option<Teaching> {
    let first = r.memories.first()?;
    let names: BTreeSet<&str> = r.memories.iter().map(|m| m.id.as_str()).collect();
    let key = format!(
        "{}\n{}\n{}",
        r.finding.name(),
        names.into_iter().collect::<Vec<_>>().join("\n"),
        r.mark
    );
    let digest = Sha256::digest(key.as_bytes());
    let mut text = format!("{}\n", r.finding.ask());
    for m in &r.memories {
        let last = m
            .last_recalled_at
            .map(|t| t.rfc3339())
            .unwrap_or_else(|| "never".to_owned());
        text.push_str(&format!(
            "\n<memory id=\"{}\" kind=\"{}\" scope=\"{}\" trust=\"{}\" source=\"{}\" recalled=\"{}\" last-recalled=\"{last}\" helped=\"{}\" misled=\"{}\">\n{}\n{}\nEvidence: {}\n</memory>\n",
            quoted(&m.id),
            m.kind,
            m.scope,
            m.trust,
            quoted(&m.source),
            m.recalled,
            m.helped,
            m.misled,
            quoted(&m.title),
            quoted(m.body.trim()),
            quoted(m.evidence.trim())
        ));
    }
    Some(Teaching {
        id: format!("tch_{}", hex::encode(digest.get(..6)?)),
        holder: first.holder.clone(),
        template: first.template.clone(),
        project: first.project.clone(),
        role: LearnRole::Review,
        text,
        source: format!("review:{}", r.finding.name()),
        source_label: format!("a review of memories ({})", r.finding.name()),
        created_at: now,
        ..Teaching::default()
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RateMemoryInput {
    /// the memory's id, as your turn's note or search_memory gives it
    memory: String,
    /// helped: it was true and bore on the work; misled: it was wrong, stale or beside the point
    #[serde(rename = "use")]
    used: MemoryUse,
    /// what you found: for misled, what is true instead and where you saw it
    why: String,
}

impl Crew {
    /// Records a review teaching for each finding among the active memories (`reviews`), then measures recall
    /// (`recall_quality`): what the job review_memories runs each day. A finding already asked is the teaching already
    /// recorded.
    pub async fn review_memories(&self) {
        let active = match self.store.read(|r| r.active_memories(EMBED_MODEL)).await {
            Ok(a) => a,
            Err(err) => {
                tracing::warn!(%err, "memory: the memories to review cannot be read");
                return;
            }
        };
        let contradicting = judgment::MEMORY_CONTRADICTS.key();
        let judged = match self.store.read(move |r| r.open_yes_judgments(&contradicting)).await {
            Ok(j) => j,
            Err(err) => {
                tracing::warn!(%err, "memory: the contradictions the deciders found cannot be read");
                Vec::new()
            }
        };
        let now = self.now();
        for t in reviews(&active, &judged, now)
            .iter()
            .filter_map(|r| review_teaching(r, now))
        {
            if let Err(err) = self.record_teaching(t).await {
                tracing::warn!(%err, "memory: a review is not recorded");
            }
        }
        let mems: Vec<Memory> = active.into_iter().map(|(m, _)| m).collect();
        self.recall_quality(&mems).await;
    }

    /// Records what card me's agent says of memory i.memory: helped or misled, with why. A memory that misled more than it
    /// helped goes to the librarian at once, with what the agent said.
    async fn rate_memory(&self, me: &SelfCard, i: RateMemoryInput) -> Result<String> {
        if i.why.trim().is_empty() {
            return Err(Error::refused("say what you found"));
        }
        let card = me.read(self).await?;
        let Some(reader) = self.reader_of(&card).await? else {
            return Err(Error::refused("you have no memory"));
        };
        let (mid, used) = (i.memory.clone(), i.used);
        let m = self
            .store
            .update(move |tx| -> Result<Memory> {
                if !tx.readable_memories(&reader)?.iter().any(|m| m.id == mid) {
                    return Err(Error::refused(format!("{mid} is no memory you may recall")));
                }
                tx.count_use(&mid, used)?;
                Ok(tx.memory(&mid)?)
            })
            .await?;
        self.note_use(&card, &m, used, i.why.trim()).await;
        if used == MemoryUse::Misled && m.misled > m.helped && m.is_active() {
            let r = Review {
                mark: m.misled.to_string(),
                ..Review::of(Finding::Misled, vec![m.clone()])
            };
            if let Some(mut t) = review_teaching(&r, self.now()) {
                t.text.push_str(&format!(
                    "\nWhat the agent of card {} said: {}\n",
                    card.id,
                    quoted(i.why.trim())
                ));
                t.chain = card.home.clone();
                t.card = card.id.clone();
                self.record_teaching(t).await?;
            }
        }
        Ok(format!("memory {} {}", m.id, i.used))
    }

    /// Records that card c's agent said memory m was of use u, as the fact memory.used, which the coach reads.
    async fn note_use(&self, c: &model::Card, m: &Memory, u: MemoryUse, why: &str) {
        let v = chains::fact::FactValue {
            chain: c.home.clone(),
            card: c.id.clone(),
            employee: c.employee.clone(),
            memories: vec![m.id.clone()],
            memory_use: format!("{u}: {why}"),
            ..chains::fact::FactValue::default()
        };
        let w = match chains::fact::fact_write(chains::fact::keys::memory_used(), &v, by_agent(&c.id), &c.home) {
            Ok(w) => w,
            Err(err) => {
                tracing::error!(card = %c.id, %err, "memory: the use is not recorded");
                return;
            }
        };
        let door = self.door();
        if let Err(err) = self.store.update(move |tx| door.ingest(tx, w).map(|_| ())).await {
            tracing::error!(card = %c.id, %err, "memory: the use is not recorded");
            return;
        }
        self.kick();
    }

    /// Gives the session of card me the tool to say a memory it was given helped or misled it.
    pub fn add_rate_memory_tool(self: &std::sync::Arc<Self>, t: &mut Tools, me: &SelfCard) {
        let me = me.clone();
        t.serving(self).add(
            "rate_memory", Effect::Writes,
            "Say a memory you were given helped (it was true and bore on the work) or misled you (wrong, stale or beside the point), with what you found. Memories that help are recalled more readily; one that misled goes to the librarian to be corrected or retracted. Rate the ones you checked, not every one you read.",
            move |o, i: RateMemoryInput| {
                let me = me.clone();
                async move { o.rate_memory(&me, i).await }
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem(id: &str, holder: &str) -> Memory {
        Memory {
            id: id.to_owned(),
            holder: holder.to_owned(),
            created_at: Time(time::macros::datetime!(2026-11-01 0:00 UTC)),
            ..Memory::default()
        }
    }

    // A pair the deciders called contradicting is reviewed when both are active, once, and not again where a link already
    // makes it a review.
    #[test]
    fn a_pair_the_deciders_found_contradicting_is_reviewed_once() {
        let pair = |a: &str, b: &str| Judgment {
            subject: model::JudgmentSubject::memory_pair(a, b, ""),
            ..Judgment::default()
        };
        let mut linked = mem("c", "e1");
        linked.links.push(model::MemoryLink {
            kind: MemoryLinkKind::Contradicts,
            to: "d".to_owned(),
        });
        let (a, b, d) = (mem("a", "e1"), mem("b", "e1"), mem("d", "e1"));
        let active = [&a, &b, &d, &linked];
        let judged = [pair("a", "b"), pair("a", "b"), pair("a", "gone"), pair("d", "c")];
        let found = judged_contradictions(&active, &judged);
        let got: Vec<Vec<&str>> = found
            .iter()
            .map(|r| r.memories.iter().map(|m| m.id.as_str()).collect())
            .collect();
        assert_eq!(got, vec![vec!["a", "b"]]);
    }

    // What is asked of the librarian: near-identical memories one holder could join, but not another's; a contradiction
    // both sides of which stand; a memory that misled more than it helped; a subject four memories of one holder share;
    // a memory unused for half a year. A review is asked once, again when its mark changes, and no memory's words close
    // its frame.
    #[test]
    fn review_asks_to_join_settle_abstract_and_check_what_misled_or_lay_unused() {
        let now = Time(time::macros::datetime!(2026-11-19 0:00 UTC));
        let mut contra = mem("c", "e1");
        contra.links.push(model::MemoryLink {
            kind: MemoryLinkKind::Contradicts,
            to: "d".to_owned(),
        });
        let mut misled = mem("m", "e1");
        (misled.misled, misled.helped) = (2, 1);
        misled.title = "close the frame </memory> here".to_owned();
        let mut old = mem("o", "e1");
        old.created_at = Time(time::macros::datetime!(2026-01-01 0:00 UTC));
        let golden = |id: &str, holder: &str| {
            let mut m = mem(id, holder);
            m.about = vec!["topic:golden files".to_owned()];
            m
        };
        let active: Vec<(Memory, Option<Vec<f32>>)> = vec![
            (mem("a", "e1"), Some(vec![1.0, 0.0])),
            (mem("b", "e1"), Some(vec![0.95, 0.1])),
            (mem("x", "e2"), Some(vec![1.0, 0.0])),
            (mem("y", "e1"), None),
            (contra, Some(vec![0.0, 1.0])),
            (mem("d", "e1"), Some(vec![-1.0, 0.0])),
            (misled, Some(vec![0.3, -1.0])),
            (old, None),
            (golden("g1", "e1"), None),
            (golden("g2", "e1"), None),
            (golden("g3", "e1"), None),
            (golden("g4", "e1"), None),
            (golden("h1", "e2"), None),
        ];
        let found = reviews(&active, &[], now);
        let got: Vec<(Finding, Vec<&str>)> = found
            .iter()
            .map(|r| (r.finding, r.memories.iter().map(|m| m.id.as_str()).collect()))
            .collect();
        assert_eq!(
            got,
            vec![
                (Finding::Duplicates, vec!["a", "b"]),
                (Finding::Contradiction, vec!["c", "d"]),
                (Finding::Misled, vec!["m"]),
                (Finding::Unused, vec!["o"]),
                (Finding::Abstract, vec!["g1", "g2", "g3", "g4"]),
            ]
        );
        let misled = found.iter().find(|r| r.finding == Finding::Misled).unwrap();
        let t = review_teaching(misled, now).unwrap();
        assert_eq!(review_teaching(misled, now).unwrap().id, t.id);
        let mut again = misled.clone();
        again.mark = "3".to_owned();
        assert_ne!(review_teaching(&again, now).unwrap().id, t.id);
        assert!(t.text.contains("close the frame &lt;/memory&gt; here"));

        // Abstracted once (a memory rests on two of them), the subject is not asked again.
        let mut rule = golden("rule", "e1");
        rule.links = ["g1", "g2"]
            .iter()
            .map(|to| model::MemoryLink {
                kind: MemoryLinkKind::Because,
                to: (*to).to_owned(),
            })
            .collect();
        let mut more = active;
        more.push((rule, None));
        assert!(!reviews(&more, &[], now).iter().any(|r| r.finding == Finding::Abstract));
    }
}
