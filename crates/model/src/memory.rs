use serde::{Deserialize, Serialize};

use crate::Time;
use crate::invalid::Invalid;
use crate::text_enum;

text_enum! {
    /// What a memory is: a lesson (what to do or not do next time), a pitfall (what went wrong and how it showed), a
    /// preference (what a person likes done their way), a principle (a standing rule a person stated), a decision (what was
    /// chosen and why), an intent (a goal that outlives the work), a fact (how a thing is, where it lives), a procedure (a
    /// routine of several steps that worked, in their order).
    #[derive(Default)]
    pub enum MemoryKind {
        #[default]
        Lesson = "lesson",
        Pitfall = "pitfall",
        Preference = "preference",
        Principle = "principle",
        Decision = "decision",
        Intent = "intent",
        Fact = "fact",
        Procedure = "procedure",
    }
}

text_enum! {
    /// Who reads a memory besides its holder: own, the holder alone; project, every agent at work in the memory's project
    /// (what was learned of the work, not of the worker); lineage, the employees hired from the holder's template; everyone.
    /// They are in the order of how far a memory reaches.
    #[derive(Default, PartialOrd, Ord)]
    pub enum MemoryScope {
        #[default]
        Own = "own",
        Project = "project",
        Lineage = "lineage",
        Everyone = "everyone",
    }
}

text_enum! {
    /// Whether a person stands behind a memory: auto, a librarian wrote it from a teaching; confirmed, a person did, or
    /// confirmed it.
    #[derive(Default)]
    pub enum MemoryTrust {
        #[default]
        Auto = "auto",
        Confirmed = "confirmed",
    }
}

text_enum! {
    /// Whether a memory is recalled: active; superseded, a later memory corrects or joins it (`superseded_by`); retracted,
    /// the owner overturned the decision that taught it, or a review found it wrong or misleading. Neither of the last two
    /// is recalled, and both stay as the record, with when and why (`ended_at`, `ended_why`).
    #[derive(Default)]
    pub enum MemoryStatus {
        #[default]
        Active = "active",
        Superseded = "superseded",
        Retracted = "retracted",
    }
}

text_enum! {
    /// How one memory bears on another: because, it rests on it; refines, it narrows or adds to it; contradicts, it says
    /// otherwise (both stand until one is superseded); related, the same situation; supersedes, it corrects it, set by the
    /// platform when a memory is superseded.
    pub enum MemoryLinkKind {
        Because = "because",
        Refines = "refines",
        Contradicts = "contradicts",
        Related = "related",
        Supersedes = "supersedes",
    }
}

/// One edge of the memory graph from a memory: how it bears on the memory `to`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryLink {
    pub kind: MemoryLinkKind,
    pub to: String,
}

/// Whether a memory is still recalled, and when it is not, when and why it stopped: one value, so an active memory has no
/// end, a retracted one has no successor, and every ended one says when. `by` of a superseded memory is the memory that
/// superseded it: empty only for one the memory daemon superseded without saying by what, or whose successor the owner
/// had forgotten.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum MemoryEnd {
    #[default]
    Active,
    Superseded {
        by: String,
        at: Time,
        why: String,
    },
    Retracted {
        at: Time,
        why: String,
    },
}

impl MemoryEnd {
    pub fn status(&self) -> MemoryStatus {
        match self {
            Self::Active => MemoryStatus::Active,
            Self::Superseded { .. } => MemoryStatus::Superseded,
            Self::Retracted { .. } => MemoryStatus::Retracted,
        }
    }

    /// When it stopped being recalled; none while it is.
    pub fn at(&self) -> Option<Time> {
        match self {
            Self::Active => None,
            Self::Superseded { at, .. } | Self::Retracted { at, .. } => Some(*at),
        }
    }

    /// Why it stopped being recalled; empty while it is.
    pub fn why(&self) -> &str {
        match self {
            Self::Active => "",
            Self::Superseded { why, .. } | Self::Retracted { why, .. } => why,
        }
    }

    /// The memory that superseded it; empty for any other.
    pub fn superseded_by(&self) -> &str {
        match self {
            Self::Superseded { by, .. } => by,
            Self::Active | Self::Retracted { .. } => "",
        }
    }
}

/// One thing an agent remembers (ADR-0117). `holder` is whom it was learned for: an employee's id, a mentor's (what they
/// taught), or `assistant`; `template` the holder's template, which its lineage shares. `project` is the project (ADR-0108)
/// of the chain it was learned in, empty for none, `work` that chain's title (the work that taught it), and `chain` and
/// `card` the ids of that chain and the card taught, empty when the teaching named none. `evidence` quotes the teaching's
/// words it rests on and `source` names the teaching (`card:<id> accepted`, `card:<id> sent back`, `mr:<n> <a>..<b>`,
/// `decision:<id> overturned by decision:<id>`), set by the platform, with `source_label` the same in words a person reads
/// ("an accepted card: Engineer · sword slash"); `teacher` who taught it, empty for what an accepted card's own session
/// taught. `about` is what it is about, each a subject (`subject_of`: `file:<path>`,
/// `crate:<name>`, `repo:<name>`, `tool:<name>`, `person:<name>`, `topic:<words>`) the graph joins memories by.
/// `retaught` are the later teachings that taught it again. `recalled` is how many turns were given it, `last_recalled_at`
/// when the last was, `helped` and `misled` how often an agent that used it said it helped or misled (`MemoryUse`): its
/// use, which the store counts beside it and never writes with it. A memory is never rewritten in place to say something
/// else: a correction supersedes it, so `end` says when and why one stopped being recalled, and the record keeps what was
/// believed when. It is written as a `MemoryRecord`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(into = "MemoryRecord", try_from = "MemoryRecord")]
pub struct Memory {
    pub id: String,
    pub holder: String,
    pub template: String,
    pub project: String,
    pub work: String,
    pub chain: String,
    pub card: String,
    pub kind: MemoryKind,
    pub scope: MemoryScope,
    pub trust: MemoryTrust,
    pub end: MemoryEnd,
    pub title: String,
    pub body: String,
    pub evidence: String,
    pub source: String,
    pub source_label: String,
    pub teacher: String,
    pub about: Vec<String>,
    pub links: Vec<MemoryLink>,
    pub retaught: Vec<String>,
    pub recalled: u64,
    pub last_recalled_at: Option<Time>,
    pub helped: u64,
    pub misled: u64,
    pub created_at: Time,
    pub updated_at: Time,
}

impl Memory {
    pub fn status(&self) -> MemoryStatus {
        self.end.status()
    }

    pub fn is_active(&self) -> bool {
        self.end == MemoryEnd::Active
    }

    /// Ends it with end at now: it is no longer recalled, and stays as the record.
    pub fn end_as(&mut self, end: MemoryEnd, now: Time) {
        self.end = end;
        self.updated_at = now;
    }

    /// The text it is embedded and judged as, read in its context (contextual retrieval): its kind and claim, its body,
    /// and what places it, what it is about, its project and the work that taught it. A change to any of them is embedded
    /// again.
    pub fn passage(&self) -> String {
        let mut out = format!("{}: {}. {}", self.kind, self.title.trim(), self.body.trim());
        if !self.about.is_empty() {
            out.push_str(&format!("\nAbout: {}", self.about.join(", ")));
        }
        if !self.project.is_empty() {
            out.push_str(&format!("\nProject: {}", self.project));
        }
        if !self.work.trim().is_empty() {
            out.push_str(&format!("\nLearned in: {}", self.work.trim()));
        }
        out
    }
}

/// A memory as it is written, stored and sent: its end as the four fields its screens read (`status`, `superseded_by`,
/// `ended_at`, `ended_why`). A record whose fields say two things at once (an active memory with an end, a retracted one
/// with a successor, an ended one with no time) is refused when it is read.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MemoryRecord {
    pub id: String,
    pub holder: String,
    pub template: String,
    pub project: String,
    pub work: String,
    pub chain: String,
    pub card: String,
    pub kind: MemoryKind,
    pub scope: MemoryScope,
    pub trust: MemoryTrust,
    pub status: MemoryStatus,
    pub title: String,
    pub body: String,
    pub evidence: String,
    pub source: String,
    pub source_label: String,
    pub teacher: String,
    pub about: Vec<String>,
    pub links: Vec<MemoryLink>,
    pub superseded_by: String,
    pub retaught: Vec<String>,
    pub recalled: u64,
    pub last_recalled_at: Option<Time>,
    pub helped: u64,
    pub misled: u64,
    pub created_at: Time,
    pub updated_at: Time,
    pub ended_at: Option<Time>,
    pub ended_why: String,
}

impl From<Memory> for MemoryRecord {
    fn from(m: Memory) -> Self {
        Self {
            status: m.end.status(),
            superseded_by: m.end.superseded_by().to_owned(),
            ended_at: m.end.at(),
            ended_why: m.end.why().to_owned(),
            id: m.id,
            holder: m.holder,
            template: m.template,
            project: m.project,
            work: m.work,
            chain: m.chain,
            card: m.card,
            kind: m.kind,
            scope: m.scope,
            trust: m.trust,
            title: m.title,
            body: m.body,
            evidence: m.evidence,
            source: m.source,
            source_label: m.source_label,
            teacher: m.teacher,
            about: m.about,
            links: m.links,
            retaught: m.retaught,
            recalled: m.recalled,
            last_recalled_at: m.last_recalled_at,
            helped: m.helped,
            misled: m.misled,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

impl TryFrom<MemoryRecord> for Memory {
    type Error = Invalid;

    fn try_from(r: MemoryRecord) -> Result<Self, Invalid> {
        let end = match (r.status, r.ended_at) {
            (MemoryStatus::Active, None) if r.superseded_by.is_empty() && r.ended_why.is_empty() => MemoryEnd::Active,
            (MemoryStatus::Active, _) => {
                return Err(Invalid::new(format!("memory {} is active and says it ended", r.id)));
            }
            (_, None) => return Err(Invalid::new(format!("memory {} ended and says not when", r.id))),
            (MemoryStatus::Superseded, Some(at)) => MemoryEnd::Superseded {
                by: r.superseded_by,
                at,
                why: r.ended_why,
            },
            (MemoryStatus::Retracted, Some(_)) if !r.superseded_by.is_empty() => {
                return Err(Invalid::new(format!(
                    "memory {} is retracted and says it was superseded",
                    r.id
                )));
            }
            (MemoryStatus::Retracted, Some(at)) => MemoryEnd::Retracted { at, why: r.ended_why },
        };
        Ok(Self {
            id: r.id,
            holder: r.holder,
            template: r.template,
            project: r.project,
            work: r.work,
            chain: r.chain,
            card: r.card,
            kind: r.kind,
            scope: r.scope,
            trust: r.trust,
            end,
            title: r.title,
            body: r.body,
            evidence: r.evidence,
            source: r.source,
            source_label: r.source_label,
            teacher: r.teacher,
            about: r.about,
            links: r.links,
            retaught: r.retaught,
            recalled: r.recalled,
            last_recalled_at: r.last_recalled_at,
            helped: r.helped,
            misled: r.misled,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
    }
}

text_enum! {
    /// What an agent that used a memory says of it: helped, it was true and bore on the work; misled, it was wrong, stale
    /// or beside the point, which also asks the librarian to review it.
    pub enum MemoryUse {
        Helped = "helped",
        Misled = "misled",
    }
}

/// The kinds of subject a memory is about, the nodes the graph joins memories by.
pub const SUBJECT_KINDS: [&str; 6] = ["file", "crate", "repo", "tool", "person", "topic"];

/// raw as a subject (`<kind>:<name>`, the kind one of `SUBJECT_KINDS`), written one way: the kind and a topic in lower
/// case, inner space collapsed, a path with no leading `./` or trailing `/`. None for anything else. Pure.
pub fn subject_of(raw: &str) -> Option<String> {
    let (kind, name) = raw.trim().split_once(':')?;
    let kind = kind.trim().to_lowercase();
    if !SUBJECT_KINDS.contains(&kind.as_str()) {
        return None;
    }
    let mut name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if kind == "topic" {
        name = name.to_lowercase();
    }
    if kind == "file" {
        name = name.trim_start_matches("./").trim_end_matches('/').to_owned();
    }
    if name.is_empty() {
        return None;
    }
    Some(format!("{kind}:{name}"))
}

text_enum! {
    /// What the memory may learn from an employee's session it is let read: accepted, everything in it, the employee's own
    /// turns included (its work was accepted); teaching, only what its teacher said or changed (a person or a judge
    /// corrected or steered it), never the employee's own conclusions; review, no new knowledge: memories the platform
    /// found to merge, settle, abstract or check again (near duplicates, a contradiction both sides of which stand, several
    /// on one subject, one that misled, one no turn was given for long), which the librarian may only merge, link,
    /// abstract or retract.
    #[derive(Default)]
    pub enum LearnRole {
        #[default]
        Accepted = "accepted",
        Teaching = "teaching",
        Review = "review",
    }
}

text_enum! {
    /// Where a teaching stands: waiting for the librarian, or settled (it wrote what `written` names, or nothing, and
    /// `note` says why).
    #[derive(Default)]
    pub enum TeachingState {
        #[default]
        Waiting = "waiting",
        Settled = "settled",
    }
}

/// One thing the librarian is to learn from (ADR-0117): what the gate let through of a teaching. For a card's session,
/// `card` and `role` say what of it may be read (accepted: all of it; teaching: only what was said to the agent), from event
/// `from_event` on (0: its start) to `to_event`, the session's last event when it was recorded (0 for one recorded before
/// that was kept: its end); for a teaching outside any session (an edit, an overturn, a retrospective, a marked reply, a
/// steer's words), `text` holds it. `holder`, `template` and `project` are what each memory learned from it carries; `chain`
/// and `card` where it was taught; `source` names it and `source_label` says it in words, both carried by its memories.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Teaching {
    pub id: String,
    pub holder: String,
    pub template: String,
    pub project: String,
    pub chain: String,
    pub card: String,
    pub role: LearnRole,
    pub from_event: i64,
    pub to_event: i64,
    pub text: String,
    pub teacher: String,
    pub source: String,
    pub source_label: String,
    pub state: TeachingState,
    /// The memories it wrote or retaught.
    pub written: Vec<String>,
    pub note: String,
    pub created_at: Time,
    pub settled_at: Option<Time>,
}

impl Teaching {
    /// Whether it gives a card's session to read, rather than a text.
    pub fn reads_session(&self) -> bool {
        self.text.is_empty()
    }

    /// Whether its session range holds event: from `from_event` on, up to `to_event` when it is set. A text holds none.
    pub fn covers(&self, event: i64) -> bool {
        self.reads_session() && event >= self.from_event && (self.to_event == 0 || event <= self.to_event)
    }
}
