//! The columns of the boards whose cards arrive one by one (wire's `Boards`): the Subscriptions board's by what happens to
//! a subscription, the Feedback board's by kind, and each Crew board's by the groups its records name. Laid out by the
//! server and pushed whole when they move: a playbook, agent, skill or source written may add a group or take its last
//! card away (views_watched.rs marks them). Kept beside the views they are computed with (views.rs's `Computed`). Pure:
//! `boards_of`. At the edge: `Orchestrator::boards`.

use model::{FeedbackKind, GROUP_GENERAL};
use wire::app::{
    Boards, FeedbackColumn, GroupColumn, SubscriptionColumn, SubscriptionGroup, SubscriptionKind, SubscriptionKindSaid,
};

use crate::orch::Orchestrator;
use chains::Result;

/// The Subscriptions board's columns, in the order the owner looks at them: what should have acted, what will, what will
/// not. Each: its group, label, icon and what it says while it holds none.
const SUBSCRIPTION_COLUMNS: [(SubscriptionGroup, &str, &str, &str); 3] = [
    (
        SubscriptionGroup::Attention,
        "Held back or due now",
        "alert",
        "A subscription stands here once its time has come, or when a firing of it failed or something holds it back.",
    ),
    (
        SubscriptionGroup::Coming,
        "Fires at its time or at a reading",
        "clock",
        "A subscription stands here while it waits for its time or its signal, and while the chain it started runs.",
    ),
    (
        SubscriptionGroup::Idle,
        "Paused or over: fires no more",
        "pause",
        "A subscription stands here while it is paused, and once it has no next time.",
    ),
];

/// Every kind of subscription: the kind, its plural, its noun, what one does, its icon, and whether the board leaves it
/// out until the owner asks for it. The automations Remit runs on its own are many and rarely the owner's to look at.
const SUBSCRIPTION_KINDS: [(SubscriptionKind, &str, &str, &str, &str, bool); 5] = [
    (
        SubscriptionKind::CheckBack,
        "Check-backs",
        "Check-back",
        "Checks back on a chain",
        "clock",
        false,
    ),
    (
        SubscriptionKind::Watch,
        "Watches",
        "Watch",
        "Watches a signal",
        "eye",
        false,
    ),
    (
        SubscriptionKind::Routine,
        "Routines",
        "Routine",
        "Starts a chain",
        "repeat",
        false,
    ),
    (
        SubscriptionKind::Event,
        "On events",
        "On events",
        "Acts on an event",
        "inbox",
        false,
    ),
    (
        SubscriptionKind::Automation,
        "Automations",
        "Automation",
        "Does something on its own",
        "spark",
        true,
    ),
];

/// The Feedback board's columns: how the agent is asked to work, what got in its way, what it complains of, what it would
/// change. Each: its kind, label, icon and what it says while it holds none.
const FEEDBACK_COLUMNS: [(FeedbackKind, &str, &str, &str); 4] = [
    (
        FeedbackKind::Practice,
        "How I work",
        "gear",
        "What an agent says of how it is asked to work lands here.",
    ),
    (
        FeedbackKind::Difficulty,
        "Difficulty",
        "alert",
        "What got in an agent's way lands here.",
    ),
    (
        FeedbackKind::Complaint,
        "Complaint",
        "comment",
        "What an agent complains of lands here.",
    ),
    (
        FeedbackKind::Suggestion,
        "Suggestion",
        "spark",
        "What an agent would change lands here.",
    ),
];

/// The groups of each Crew board's records, in the order the store lists the records.
pub(crate) struct CrewGroups<'a> {
    pub playbooks: Vec<&'a str>,
    pub agents: Vec<&'a str>,
    pub skills: Vec<&'a str>,
    /// Each source's capability.
    pub sources: Vec<&'a str>,
}

/// Every board's columns, the Crew boards' laid out from groups; rev is left to who keeps them.
pub(crate) fn boards_of(groups: &CrewGroups<'_>) -> Boards {
    Boards {
        subscriptions: SUBSCRIPTION_COLUMNS
            .iter()
            .map(|&(key, label, icon, empty)| SubscriptionColumn {
                key,
                label: label.to_owned(),
                icon: icon.to_owned(),
                empty: empty.to_owned(),
            })
            .collect(),
        subscription_kinds: SUBSCRIPTION_KINDS
            .iter()
            .map(
                |&(kind, plural, noun, says, icon, starts_hidden)| SubscriptionKindSaid {
                    kind,
                    plural: plural.to_owned(),
                    noun: noun.to_owned(),
                    says: says.to_owned(),
                    icon: icon.to_owned(),
                    starts_hidden,
                },
            )
            .collect(),
        feedback: FEEDBACK_COLUMNS
            .iter()
            .map(|&(key, label, icon, empty)| FeedbackColumn {
                key,
                label: label.to_owned(),
                icon: icon.to_owned(),
                empty: empty.to_owned(),
            })
            .collect(),
        playbooks: work_columns(&groups.playbooks, "playbook"),
        agents: work_columns(&groups.agents, "agent"),
        skills: work_columns(&groups.skills, "skill"),
        sources: source_columns(&groups.sources),
        rev: 0,
    }
}

/// One column per kind of work the records name, by name, with `general` last.
fn work_columns(groups: &[&str], what: &str) -> Vec<GroupColumn> {
    let mut keys = groups.to_vec();
    keys.sort_by_key(|g| (*g == GROUP_GENERAL, *g));
    keys.dedup();
    keys.into_iter()
        .map(|key| GroupColumn {
            key: key.to_owned(),
            label: key.to_owned(),
            icon: "list".to_owned(),
            empty: format!("No {what} for {key} work matches."),
        })
        .collect()
}

/// One column per capability the sources give, in the order the sources come.
fn source_columns(capabilities: &[&str]) -> Vec<GroupColumn> {
    let mut keys: Vec<&str> = Vec::new();
    for c in capabilities {
        if !keys.contains(c) {
            keys.push(c);
        }
    }
    keys.into_iter()
        .map(|key| GroupColumn {
            key: key.to_owned(),
            label: key.to_owned(),
            icon: "terminal".to_owned(),
            empty: format!("No source that gives {key} matches."),
        })
        .collect()
}

impl Orchestrator {
    /// The boards' columns as the records stand now: kept, and pushed at a new revision when they moved.
    pub(crate) async fn boards(&self) -> Result<Boards> {
        let mut comp = self.views.compute.lock().await;
        let (playbooks, agents, skills) = self
            .store
            .read(|r| Ok::<_, store::Error>((r.playbooks()?, r.agents()?, r.skills()?)))
            .await?;
        let sources = self.reach.source_entries().await?;
        let mut next = boards_of(&CrewGroups {
            playbooks: playbooks.iter().map(|p| p.group.as_str()).collect(),
            agents: agents.iter().map(|a| a.group.as_str()).collect(),
            skills: skills.iter().map(|s| s.group.as_str()).collect(),
            sources: sources.iter().map(|s| s.capability.as_str()).collect(),
        });
        next.rev = comp.boards.rev;
        if comp.boards == next {
            return Ok(next);
        }
        comp.rev = comp.rev.saturating_add(1);
        next.rev = comp.rev;
        comp.boards.clone_from(&next);
        self.notify.boards_updated(&next);
        Ok(next)
    }
}
