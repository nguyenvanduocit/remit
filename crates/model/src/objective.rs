use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::text_enum;
use crate::{Invalid, Time};

/// The owner as the one who answers: the one who answers when no employee was named.
pub const ACCOUNTABLE_OWNER: &str = "owner";

/// Who answers for something: the owner (ACCOUNTABLE_OWNER), or the id of an employee (ADR-0032).
/// A newtype, so serde writes it as the id itself.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Accountable(pub String);

impl Accountable {
    pub fn owner() -> Self {
        Self(ACCOUNTABLE_OWNER.to_owned())
    }

    pub fn is_owner(&self) -> bool {
        self.0 == ACCOUNTABLE_OWNER
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Accountable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

text_enum! {
    /// Where an objective stands: a draft waits for the owner's Apply, an active one is worked, a closed one is done.
    #[derive(Default)]
    pub enum ObjectiveStatus {
        #[default]
        Draft = "draft",
        Active = "active",
        Closed = "closed",
    }
}

text_enum! {
    /// How a key result is judged (ADR-0031).
    pub enum CheckKind {
        /// The owner's eyes on the landed change: allowed only with the reason no executable check can exist (ADR-0068).
        Submit = "submit",
        /// A signal, read over a window, meets a condition.
        Reading = "reading",
        /// A command run in a repository's worktree passes.
        Command = "command",
        /// Nothing can confirm it; the reason is said, and the result stays unverified.
        Unverifiable = "unverifiable",
    }
}

/// How one key result is confirmed. Each kind holds exactly its own fields and no other (`valid`): submit holds `how` and
/// needs `reason`, why no executable check can exist; reading holds `signal`, `condition`, `window` and `expiry`; command
/// holds `repo` and `name`, the command line run in that repository's worktree; unverifiable holds `reason`. `incident` is
/// the plan for when the result fails in production (what breaks, how it is seen, who reverses it and how), and
/// `telemetry` the signal a submit or command result is seen through once it runs; a reading is seen through its own
/// signal (ADR-0068). A check always names its kind: a key result with no check holds none (`KeyResult::check`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub kind: CheckKind,
    #[serde(default)]
    pub how: String,
    #[serde(default)]
    pub signal: String,
    #[serde(default)]
    pub condition: String,
    #[serde(default)]
    pub window: String,
    #[serde(default)]
    pub repo: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expiry: Option<Time>,
    #[serde(default)]
    pub telemetry: String,
    #[serde(default)]
    pub incident: String,
}

impl Check {
    /// A check of kind with none of its fields set yet.
    pub fn of(kind: CheckKind) -> Self {
        Self {
            kind,
            how: String::new(),
            signal: String::new(),
            condition: String::new(),
            window: String::new(),
            repo: String::new(),
            name: String::new(),
            reason: String::new(),
            expiry: None,
            telemetry: String::new(),
            incident: String::new(),
        }
    }

    /// Whether it is a check of its kind: the fields its kind needs are set and no field of another kind is. A check
    /// that is not valid confirms nothing: it is no check, and a chain whose key result has none does not finish
    /// (ADR-0068).
    pub fn valid(&self) -> Result<(), Invalid> {
        // The fields each kind of check holds, and those of them it must set.
        let kind = self.kind;
        let (holds, needs): (&[&str], &[&str]) = match kind {
            CheckKind::Submit => (&["how", "reason", "telemetry", "incident"], &["reason"]),
            CheckKind::Reading => (
                &["signal", "condition", "window", "expiry", "incident"],
                &["signal", "condition", "window", "expiry"],
            ),
            CheckKind::Command => (&["repo", "name", "telemetry", "incident"], &["repo", "name"]),
            CheckKind::Unverifiable => (&["reason"], &["reason"]),
        };
        let set = [
            ("how", !self.how.is_empty()),
            ("signal", !self.signal.is_empty()),
            ("condition", !self.condition.is_empty()),
            ("window", !self.window.is_empty()),
            ("repo", !self.repo.is_empty()),
            ("name", !self.name.is_empty()),
            ("reason", !self.reason.is_empty()),
            ("expiry", self.expiry.is_some()),
            ("telemetry", !self.telemetry.is_empty()),
            ("incident", !self.incident.is_empty()),
        ];
        let missing = needs
            .iter()
            .filter(|name| !set.iter().any(|(field, is_set)| field == *name && *is_set))
            .map(|name| format!("a {kind} check needs its {name}"));
        let foreign = set
            .iter()
            .filter(|(name, is_set)| *is_set && !holds.contains(name))
            .map(|(name, _)| format!("a {kind} check holds no {name}"));
        let errors: Vec<String> = missing.chain(foreign).collect();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(Invalid::new(errors.join("\n")))
        }
    }

    /// The signal the result of a check is seen through once it runs: a reading's own signal, else its telemetry.
    pub fn seen(&self) -> &str {
        if self.kind == CheckKind::Reading {
            &self.signal
        } else {
            &self.telemetry
        }
    }
}

/// One result an objective is reached by: what must be true, who answers for it, who judges it, and how. An amendment
/// that drops a key result keeps it as retired: the chains serving it finish, and no new chain is opened for it. A key
/// result is an entity of its own (ADR-0115, table `key_results`, keyed by its objective and its id): one with no parent
/// is what the owner committed to; one with a parent refines it, written by whoever answers for the parent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct KeyResult {
    pub id: String,
    pub text: String,
    pub owner: Accountable,
    pub judge: Accountable,
    /// None until its check is declared: a direct request's key result starts with none, and its Chair states one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check: Option<Check>,
    pub retired: bool,
    /// The id of the key result of the same objective it refines; none for one the owner committed to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// The last lift of the progress rail on it (ADR-0115): only chains queued after it can stall it again, since work
    /// already under way when it was lifted is the work the lift looked past.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lifted: Option<Lift>,
}

/// Key result id of tree and the key results it refines, nearest first, up to the one the owner committed to. A parent
/// is written before what refines it, so the walk ends within the tree's length even if a parent is missing.
pub fn lineage<'a>(tree: &'a [KeyResult], id: &'a str) -> Vec<&'a str> {
    let mut up = Vec::new();
    let mut at = Some(id);
    while let Some(a) = at.filter(|_| up.len() <= tree.len()) {
        up.push(a);
        at = tree.iter().find(|kr| kr.id == a).and_then(|kr| kr.parent.as_deref());
    }
    up
}

/// Who lifted a rail, when and why.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Lift {
    pub at: Time,
    pub by: crate::Actor,
    pub why: String,
}

/// A measure the work must not push past its bound (ADR-0045): the signal that reads it and the condition on that
/// signal's value that holds while the work stays inside the bound, written in the language of a subscription's
/// condition.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, rename_all = "camelCase")]
pub struct Guardrail {
    pub kpi: String,
    pub signal: String,
    pub condition: String,
}

/// Stands in for a measure that cannot be read yet: who owns it, what it reads, and until when it holds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, rename_all = "camelCase")]
pub struct Surrogate {
    pub kpi: String,
    pub owner: Accountable,
    pub reads: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<Time>,
}

/// How far ahead the objective looks: in words, and the day or moment it looks to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Horizon {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until: Option<Time>,
}

/// Something the objective does not know yet, and the question that would settle it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, rename_all = "camelCase")]
pub struct Unknown {
    pub text: String,
    pub question: String,
}

/// What the owner wants reached, as a set of key results with accountable owners and judges (ADR-0031). It is written
/// over the `rev` it was read at. `task` says it is the objective of one direct request, made by Remit.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Objective {
    pub id: String,
    pub rev: i64,
    pub task: bool,
    pub status: ObjectiveStatus,
    /// The id of the project it belongs to (ADR-0108); none for work about no product. A chain's project is this one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    pub scope: String,
    pub customer: String,
    pub outcome: String,
    pub north_star: String,
    pub guardrails: Vec<Guardrail>,
    pub surrogates: Vec<Surrogate>,
    pub horizon: Horizon,
    pub unknown: Vec<Unknown>,
    pub owner: Accountable,
    /// The key results the owner committed to, the ones with no parent: drafted, applied and amended with the objective
    /// over its revision. Each is stored as a row of `key_results`, not in the objective's document; the key results that
    /// refine them are read beside it (`ObjectiveView::key_results`).
    pub key_results: Vec<KeyResult>,
    pub created_at: Time,
}

/// Which key result of which objective a chain works toward.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Serves {
    pub objective: String,
    pub key_result: String,
}

text_enum! {
    /// Where a key result stands, derived from what judged it and never stored: unserved when no chain works toward it.
    pub enum KRStatus {
        Met = "met",
        NotMet = "not_met",
        Unverified = "unverified",
        Unserved = "unserved",
    }
}

/// An objective as the owner is shown it: its fields, with every key result of its tree in `key_results` (the owner's
/// and those that refine them, parents before children; the objective's own list is moved there), where each stands,
/// and why each stalled one holds its starts (ADR-0115), both by key result id.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ObjectiveView {
    pub objective: Objective,
    pub key_results: Vec<KeyResult>,
    pub status: BTreeMap<String, KRStatus>,
    pub stalled: BTreeMap<String, String>,
}
