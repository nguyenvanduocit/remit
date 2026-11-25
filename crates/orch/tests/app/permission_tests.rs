//! The asker never decides its own permission request (ADR-0059, ADR-0087), driven through the whole application.

use std::sync::Arc;

use model::{Card, InboxStatus, PermissionWait, Settings};
use wire::runtime::PermissionAnswer;

use crate::tests::{Agents, Rig, rig_of_agents};

/// Card card_id's session asks to run command, as its process does; the answer it reads when one is given.
fn ask(agents: &Agents, card_id: &str, command: &str) -> tokio::task::JoinHandle<PermissionAnswer> {
    let h = agents.handler(card_id);
    let call = wire::runtime::ToolCall {
        id: format!("t_{command}"),
        name: "Bash".to_owned(),
        kind: wire::runtime::ToolKind::Execute,
        title: command.to_owned(),
        status: model::ToolStatus::Pending,
        input: serde_json::Map::from_iter([("command".to_owned(), serde_json::Value::from(command))]),
        output: String::new(),
    };
    tokio::spawn(async move {
        h.on_permission(wire::runtime::PermissionRequest {
            tool: call,
            reason: String::new(),
            classifier: String::new(),
            agent_id: String::new(),
        })
        .await
    })
}

/// The card card_id as stored, once it waits on whom until says; a test fails after 5s.
async fn card_until(r: &Rig, card_id: &str, until: impl Fn(Option<&PermissionWait>) -> bool) -> Card {
    for _ in 0..250 {
        let id = card_id.to_owned();
        let c = r.o.store.read(move |s| s.card(&id)).await.unwrap();
        if until(c.permission_wait()) {
            return c;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("card {card_id} never came to the wait asked for");
}

/// The id of the one request card_id's session waits on.
fn request_of(r: &Rig, card_id: &str) -> String {
    let run = r.o.run_of(card_id).unwrap();
    let held: Vec<String> = run.state().permissions.keys().cloned().collect();
    assert_eq!(held.len(), 1, "{held:?}");
    held.into_iter().next().unwrap()
}

// A session with no Chair above it (here the Chair's own) asks the owner's inbox, where the Assistant decides for him;
// when nobody decided by the limit, the rule refuses it, not the session itself, and the owner's item closes saying so.
#[tokio::test]
async fn a_session_with_no_chair_above_it_is_refused_its_own_ask_at_the_limit() {
    let agents = Arc::new(Agents::default());
    let r = rig_of_agents(&agents);
    let ch = r.create("r1", "Fix the login page").await;
    agents.until(&format!("prompt {}", ch.chair_id)).await;

    let asked = ask(&agents, &ch.chair_id, "kubectl rollout restart deploy/web");
    card_until(&r, &ch.chair_id, |w| w == Some(&PermissionWait::Owner)).await;
    let request = request_of(&r, &ch.chair_id);
    tokio::time::pause();
    let wait = u64::try_from(Settings::default().permission_wait().0).unwrap();
    tokio::time::advance(std::time::Duration::from_nanos(wait.saturating_add(1_000_000_000))).await;
    let answer = asked.await.unwrap();
    let PermissionAnswer::Deny { message } = answer else {
        panic!("the session's own ask was allowed: {answer:?}");
    };
    assert!(message.starts_with("Refused by a rule."), "{message}");
    card_until(&r, &ch.chair_id, |w| w.is_none()).await;
    let item = r.o.store.read(move |s| s.inbox_item(&request)).await.unwrap();
    assert_eq!(
        (item.status, item.answer.as_str(), item.answered_by.map(|b| b.kind)),
        (
            InboxStatus::Cancelled,
            "decided by a rule",
            Some(model::ActorKind::Remit)
        )
    );
}
