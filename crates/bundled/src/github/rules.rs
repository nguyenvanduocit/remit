//! What the github plugin ships besides its tools (ADR-0049): the event signal whose door GitHub posts to, and the rules that say
//! what a workflow run that finished on a commit means for the chain that landed it (ADR-0114). The rules are data (a core
//! action and the expressions that fill its parameters over the event as GitHub wrote it, `header` and `body`), which Remit
//! evaluates as it does for any subscription, so the plugin cannot do more than tell a Chair or have the forge asked. GitHub
//! signs each post with the door's key, which the owner pastes as its secret; the signal runs from the first start, since it
//! waits for no credential of the plugin. GitHub's ping when its hook is made is held by none.

use std::collections::BTreeMap;

use plugin_sdk::Subscription;

/// The note of the event signal `github`.
pub(super) const NOTE: &str = "GitHub: has Remit read the checks on what a chain landed when a workflow run on it finishes, and tells the Chair of the chain when one failed.";

/// A rule on `event:github` whose `action` is filled by `with`.
fn rule(name: &str, note: &str, expr: &str, action: &str, job: &str, with: &[(&str, &str)]) -> Subscription {
    Subscription {
        name: name.into(),
        note: note.into(),
        r#match: "event:github".into(),
        expr: expr.into(),
        action: action.into(),
        job: job.into(),
        with: with
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect::<BTreeMap<_, _>>(),
        ..Default::default()
    }
}

pub(super) fn rules() -> Vec<Subscription> {
    // A workflow run is about the commit it ran on: Remit finds the chain whose landing pushed or landed onto it.
    let landed = "[body.workflow_run.head_sha]";
    vec![
        rule(
            "sync-workflow-finished",
            "Have Remit read the checks on what a chain landed when the forge's pipeline on it finishes.",
            "header['X-Github-Event'] == 'workflow_run' && body.action == 'completed'",
            "run",
            "sync-landing",
            &[("chain", landed)],
        ),
        rule(
            "tell-workflow-failed",
            "Tell the Chair of the chain that a workflow run failed on what it landed.",
            "header['X-Github-Event'] == 'workflow_run' && body.action == 'completed' && body.workflow_run.conclusion in ['failure', 'timed_out', 'startup_failure']",
            "tell",
            "",
            &[
                ("chain", landed),
                (
                    "summary",
                    "'workflow ' + body.workflow_run.name + ' failed on ' + body.workflow_run.head_branch",
                ),
                ("quote", "'run: ' + body.workflow_run.html_url"),
            ],
        ),
    ]
}
