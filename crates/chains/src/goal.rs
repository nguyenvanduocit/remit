//! The goal check as the chain core keeps it (ADR-0067): what the step writes when a Chair asks for the check, and the
//! deadline that ends a check no verdict came for. The judge and its verdicts are shipping's (goal.rs there).

use model::Duration;
use model::{ActionKind, Finding, OwnerKind, Subscription, SubscriptionAction, SubscriptionOwner, Time};
use model::{Approval, Check, CheckKind, FindingStatus, KRStatus, VerdictRole};

use crate::outbox::by_remit;
use crate::timewait::once_at;

pub const MAX_GOAL_CHECKS: i64 = 3;

/// How long the judge may take: it runs commands, so more than a text answer, but a check no one is waiting on cannot hold a
/// chain open for ever.
pub const GOAL_CHECK_LIMIT: Duration = Duration::minutes(20);

/// What the goal judge's turn is about, as the owner reads it.
pub const GOAL_JUDGE_TASK: &str = "Check the finish against the goal";

/// Each finding in a line: the item and its evidence.
pub fn finding_lines(findings: &[Finding]) -> Vec<String> {
    findings
        .iter()
        .map(|f| format!("{} ({})", f.item, f.evidence))
        .collect()
}

/// The owner of the subscriptions Remit makes for chain id (a goal check's deadline): they end with the chain.
pub fn owned_by(id: &str) -> SubscriptionOwner {
    SubscriptionOwner {
        kind: OwnerKind::Remit,
        id: id.to_owned(),
    }
}

/// The id of the deadline subscription of chain id's goal check: one at a time, since a chain has one check.
pub fn goal_deadline_id(id: &str) -> String {
    format!("sub_check_{id}")
}

/// The subscription that ends the goal check chain id's Chair requested at `at`, when `GOAL_CHECK_LIMIT` has passed with no
/// verdict, a schedule that acts once, then: stored with the request, ended with the check.
pub fn goal_deadline(id: &str, at: Time) -> Subscription {
    let end =
        at.0.checked_add(time::Duration::nanoseconds(GOAL_CHECK_LIMIT.0))
            .map_or(at, Time);
    once_at(
        Subscription {
            id: goal_deadline_id(id),
            owner: owned_by(id),
            action: SubscriptionAction {
                kind: ActionKind::End,
                subject: id.to_owned(),
                why: "A goal check that gives no verdict in time ends, and the chain finishes without it.".to_owned(),
                ..SubscriptionAction::default()
            },
            by: by_remit(),
            created_at: at,
            updated_at: at,
            ..Subscription::default()
        },
        end,
    )
}

/// How a key result is checked, for a brief: c is its check, none when none is declared yet.
pub fn check_words(c: Option<&Check>) -> String {
    let Some(c) = c else {
        return "no check is declared yet".to_owned();
    };
    let mut words = match c.kind {
        CheckKind::Submit => {
            let mut w = format!(
                "the owner reviews the change submitted for it, because no executable check can exist: {}",
                c.reason
            );
            if !c.how.is_empty() {
                w.push_str(&format!("; he looks at {}", c.how));
            }
            w
        }
        CheckKind::Reading => format!("the signal {} must meet {} over {}", c.signal, c.condition, c.window),
        CheckKind::Command => format!("the command `{}` run in the worktree of {} must pass", c.name, c.repo),
        CheckKind::Unverifiable => format!("it cannot be verified: {}", c.reason),
    };
    if !c.telemetry.is_empty() {
        words.push_str(&format!("; once it runs it is seen through {}", c.telemetry));
    }
    if !c.incident.is_empty() {
        words.push_str(&format!("; if it fails in production: {}", c.incident));
    }
    words
}

/// What a chain's latest goal verdict (of verdicts, oldest first) says of key result id, the one rule a key result's
/// status and the progress rail read: met, not_met, or unverified when it says neither. A chain serves
/// one key result, so a finding that names none is about that one; a finding that names another is not.
pub fn verdict_on(id: &str, verdicts: &[Approval]) -> KRStatus {
    let Some(latest) = verdicts.last() else {
        return KRStatus::Unverified;
    };
    let mut latest = latest.clone();
    latest
        .findings
        .retain(|f| f.key_result.is_empty() || f.key_result == id);
    if latest.findings.iter().any(|f| f.status == FindingStatus::NotMet) {
        return KRStatus::NotMet;
    }
    if !latest.findings.is_empty() && latest.confirms() {
        return KRStatus::Met;
    }
    KRStatus::Unverified
}

/// The verdicts of the chain's goal checks, in the order they were given: a check is numbered by how many there were
/// before it, in the card's name and on the board alike.
pub fn goal_approvals<'a>(approvals: impl IntoIterator<Item = &'a Approval>) -> Vec<&'a Approval> {
    approvals.into_iter().filter(|a| a.role == VerdictRole::Judge).collect()
}
