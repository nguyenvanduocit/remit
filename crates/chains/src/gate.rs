//! The gates a change passes, as the chain core reads them (ADR-0087): which approvals it still lacks of the gates it
//! needs (ADR-0131), what a goal judge says is still missing. Recording a verdict is shipping's (gate.rs there).

use model::{Approval, Finding, FindingBlocker, FindingStatus, Gate, Verdict};

use crate::chain_core::first_line;

/// Which of gates the given change still lacks; the latest verdict of each gate's role counts.
pub fn missing_approvals(approvals: &[Approval], repo: &str, change: &str, gates: &[Gate]) -> Vec<String> {
    let mut missing = Vec::new();
    for &g in gates {
        match approvals.iter().rfind(|a| a.repo == repo && a.role == g.role()) {
            None => missing.push(format!("{g}: no verdict yet")),
            Some(last) if last.verdict != Verdict::Approve => {
                missing.push(format!(
                    "{g}: {} asked for changes: {}",
                    last.name,
                    first_line(&last.notes, 300)
                ));
            }
            Some(last) if approved_earlier(last, change) => {
                missing.push(format!(
                    "{g}: {} approved an earlier version; the change has moved since",
                    last.name
                ));
            }
            Some(last) if review_needs_evidence(last, gates) => {
                missing.push(format!("{g}: record the behaviour evidence for this version"));
            }
            Some(_) => {}
        }
    }
    missing
}

/// A review on its own must cover behaviour. Recorded two-gate versions also have an independent QC verdict.
pub fn review_needs_evidence(a: &Approval, gates: &[Gate]) -> bool {
    a.role == Gate::Reviewer.role()
        && a.verdict == Verdict::Approve
        && a.behaviour_evidence.is_none()
        && !gates.contains(&Gate::Qc)
}

/// Whether a is an approval of another version of a change than change, the one it has now.
pub fn approved_earlier(a: &Approval, change: &str) -> bool {
    a.verdict == Verdict::Approve && a.change != change
}

/// What the goal judge says the Chair must still settle: a key result not met, or one left unverified on a blocker that is
/// not the owner's to settle after the chain ends (owners, `owners_to_settle`). The team proves it, or asks the owner and
/// subscribes (ADR-0067).
pub fn still_missing(findings: &[Finding], owners: &[FindingBlocker]) -> Vec<Finding> {
    findings
        .iter()
        .filter(|f| {
            !(f.status == FindingStatus::Met
                || f.status == FindingStatus::Unverified && f.blocker.is_some_and(|b| owners.contains(&b)))
        })
        .cloned()
        .collect()
}

/// A gate's verdict as the Chair reads it under the gate's report: the report is short by rule, so the evidence the Chair
/// judges it on is the verdict's record (every finding and the notes). Pure.
pub fn verdict_record(a: &Approval) -> String {
    let mut b = format!(
        "\n\nITS RECORDED VERDICT ({} on change {}):\nFindings:",
        a.verdict, a.change
    );
    for f in &a.findings {
        b.push_str(&format!("\n- [{}] {}: {}", f.status, f.item, f.evidence));
    }
    b.push_str(&format!("\nNotes:\n{}", a.notes));
    if let Some(evidence) = &a.behaviour_evidence {
        b.push_str(&format!("\nBehaviour evidence:\n{evidence}"));
    }
    b
}

/// Refuses a reviewer or QC hired without the work it judges.
pub const ERR_GATE_INPUTS: &str = "a reviewer or QC judges someone's work: list the cards whose work it judges in \
                                          inputs (e.g. the engineer who made the change)";
