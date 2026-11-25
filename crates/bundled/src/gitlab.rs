//! The gitlab plugin ships what Remit knows of GitLab's webhooks (ADR-0049): the event signal whose door GitLab posts to, and the
//! rules that say what a pipeline that finished on a commit means for the chain that landed it (ADR-0114). It runs no code of its own: the rules are data (a core
//! action and the expressions that fill its parameters over the event as GitLab wrote it, `header` and `body`), which Remit
//! evaluates as it does for any subscription, so the plugin cannot do more than tell a Chair or have the forge asked. Its signal
//! runs from the first start: it waits for no credential, since the sender is pointed at the door with the door's key.

use plugin_sdk::{Definition, Signal, Subscription};

/// The note of the event signal `gitlab`.
const NOTE: &str = "GitLab: has Remit read the checks on what a chain landed when its pipeline finishes, and tells the Chair of the chain when it failed.";

/// The bundled plugin that ships GitLab's event signal and its rules.
pub(crate) fn plugin() -> Definition {
    Definition {
        label: "GitLab".into(),
        capability: "data".into(),
        signals: vec![Signal {
            name: "gitlab".into(),
            note: NOTE.into(),
            ..Default::default()
        }],
        subscriptions: rules(),
        ..Default::default()
    }
}

/// A rule on `event:gitlab` whose `action` is filled by `with`.
fn rule(name: &str, note: &str, expr: &str, action: &str, job: &str, with: &[(&str, &str)]) -> Subscription {
    Subscription {
        name: name.into(),
        note: note.into(),
        r#match: "event:gitlab".into(),
        expr: expr.into(),
        action: action.into(),
        job: job.into(),
        with: with.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect(),
        ..Default::default()
    }
}

fn rules() -> Vec<Subscription> {
    // A pipeline is about the commit it ran on: Remit finds the chain whose landing pushed or landed onto it.
    let landed = "[body.object_attributes.sha]";
    vec![
        rule(
            "sync-pipeline-finished",
            "Have Remit read the checks on what a chain landed when the forge's pipeline on it finishes.",
            "'X-Gitlab-Event' in header && body.object_kind == 'pipeline' && body.object_attributes.status in ['success', 'failed', 'canceled']",
            "run",
            "sync-landing",
            &[("chain", landed)],
        ),
        rule(
            "tell-pipeline-failed",
            "Tell the Chair of the chain that a pipeline failed on what it landed.",
            "'X-Gitlab-Event' in header && body.object_kind == 'pipeline' && body.object_attributes.status == 'failed'",
            "tell",
            "",
            &[
                ("chain", landed),
                (
                    "quote",
                    "'failed jobs: ' + body.builds.filter(b, b.status == 'failed').map(b, b.name + ' (' + b.stage + ')').join(', ')",
                ),
                (
                    "summary",
                    "'pipeline ' + string(int(body.object_attributes.id)) + ' failed on ' + body.object_attributes.ref",
                ),
            ],
        ),
    ]
}
