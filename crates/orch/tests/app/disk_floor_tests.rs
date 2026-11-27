//! The free-space floor on the volume of the work root (ADR-0140), driven as a Chair's hire and the owner's message drive
//! a session's start: below the floor a worker's session does not start and its card fails with the free space and the
//! floor in words; above it, it starts; the Assistant (and the Chair, by the same seat rule) starts below it.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use chains::hire_input::HireInput;
use model::{Card, CardStatus, EventStatus, SystemNote};
use orch::chains::ports::{Host, HostReading};

use crate::tests::{Agents, NOW, Rig, owner, rig_with};

/// A host whose work-root volume has the bytes it is given free; nothing else of it can be read.
struct DiskOf(u64);

impl Host for DiskOf {
    fn reading(&self) -> Option<HostReading> {
        None
    }

    fn free_space(&self, _dir: &Path) -> Option<u64> {
        Some(self.0)
    }
}

fn rig_on(free: u64, agents: &Arc<Agents>) -> Rig {
    rig_with(
        |c| c.core.host = Some(Arc::new(DiskOf(free))),
        agents.clone(),
        Arc::new(|| NOW),
    )
}

/// A chain's Chair hires a worker on a host with free bytes free; the worker's card.
async fn hire_worker(r: &Rig) -> Card {
    let chain = r.create("rq_floor", "Build the release").await;
    let id = chain.chair_id.clone();
    let chair = r.o.store.read(move |s| s.card(&id)).await.unwrap();
    let hire = HireInput {
        agent: "research".to_owned(),
        brief: "Build the crate and say what fails.".to_owned(),
        why: "the owner wants a release".to_owned(),
        deliverable: "the build's result".to_owned(),
        model: "opus".to_owned(),
        ..HireInput::default()
    };
    r.o.hire(&chair, hire).await.unwrap()
}

/// Waits until card's status is want; a test fails after 5s.
async fn until_status(r: &Rig, card: &str, want: &CardStatus) -> Card {
    for _ in 0..250 {
        let id = card.to_owned();
        let c = r.o.store.read(move |s| s.card(&id)).await.unwrap();
        if c.status() == want {
            return c;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("card {card} never {want}");
}

#[tokio::test(flavor = "multi_thread")]
async fn work_root_disk_floor() {
    // above the floor a worker starts
    let agents = Arc::new(Agents::default());
    let r = rig_on(100 << 30, &agents);
    let worker = hire_worker(&r).await;
    agents.until(&format!("start {}", worker.id)).await;

    // below it the worker is refused, saying the free space and the floor
    let agents = Arc::new(Agents::default());
    let r = rig_on(1 << 30, &agents);
    let worker = hire_worker(&r).await;
    until_status(&r, &worker.id, &CardStatus::Failed).await;
    let started = format!("start {}", worker.id);
    assert!(!agents.all().contains(&started), "a worker started below the floor");
    let id = worker.id.clone();
    let events = r.o.store.read(move |s| s.card_events(&id, 1000)).await.unwrap();
    let error = events
        .iter()
        .find(|e| e.status == EventStatus::System(SystemNote::Error))
        .expect("the refusal is in the card's stream");
    assert!(
        error.text.contains("1.0 GB free") && error.text.contains("floor of 5.0 GB"),
        "{}",
        error.text
    );

    // below it the Assistant, who decides that card's retry, still starts
    let core = &r.o.core;
    core.ensure_assistant().await.unwrap();
    let assistant = core.assistant_card().await.unwrap();
    let message = orch::chains::outbox::owner_message(&assistant.id, "Your message", "Hello", owner("prompt").by);
    core.send_to_standalone(&assistant, message, Default::default())
        .await
        .unwrap();
    agents.until(&format!("start {}", assistant.id)).await;
}
