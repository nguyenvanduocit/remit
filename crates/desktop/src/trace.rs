//! `--trace <dir>` (ADR-0128 decision 6): the window driven headless against the fake server, the core running as it does
//! over QUIC, each frame recorded as a PNG with its report. The seed is a busy owner's board: chains that wait for him,
//! that run, wait, watch, queue and finished, and something that waits from no chain.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::Path;
use std::rc::Rc;

use gui::{Scenario, ScenarioError};
use replica::{Ask, Input};
use serde_json::{Value, json};
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime, UtcOffset};
use wire::app::Call;

use crate::app::{App, ComeIn};
use crate::core::{Pump, TAB, now, options};
use crate::fake::Fake;
use crate::store::Store;
use crate::way_in::Credential;

/// The core over the fake server, on the window's own thread: what the window asks waits for `settle`.
struct Faked {
    pump: RefCell<Pump<Fake>>,
    asks: RefCell<VecDeque<Ask>>,
    store: RefCell<Option<Store>>,
}

impl Faked {
    /// Hands the core what the window asked, and the window what the core told it, until neither has more.
    fn settle(&self) {
        loop {
            let ask = self.asks.borrow_mut().pop_front();
            let Some(ask) = ask else { return };
            self.give(Input::Ask { tab: TAB, ask });
        }
    }

    fn give(&self, input: Input) {
        let told = self.pump.borrow_mut().give(now(), input);
        if let Some(store) = self.store.borrow().clone() {
            store.apply(told);
        }
    }

    /// The server goes down: the session is lost and every dial fails.
    fn lose(&self) {
        let arrived = {
            let mut pump = self.pump.borrow_mut();
            pump.wire.go_down();
            crate::core::Wire::arrived(&mut pump.wire)
        };
        arrived.into_iter().for_each(|input| self.give(input));
    }

    /// The server is back, and the core's timer comes: it dials again.
    fn come_back(&self) {
        self.pump.borrow_mut().wire.come_up();
        self.give(Input::Tick);
    }
}

pub fn run(dir: &Path, offset: UtcOffset) -> Result<(), ScenarioError> {
    let faked = Rc::new(Faked {
        pump: RefCell::new(Pump::new(options("trace".to_owned()), Fake::new(Box::new(answer)))),
        asks: RefCell::new(VecDeque::new()),
        store: RefCell::new(None),
    });
    let come_in: ComeIn = {
        let faked = Rc::clone(&faked);
        Rc::new(move |app: &App, _: Credential| {
            let queue = Rc::clone(&faked);
            let toaster = app.toaster.clone();
            let store = Store::new(
                Rc::new(move |ask| queue.asks.borrow_mut().push_back(ask)),
                Rc::new(|_| {}),
                Rc::new(move |notice| crate::app::say(&toaster, notice)),
            );
            *faked.store.borrow_mut() = Some(store.clone());
            Ok(store)
        })
    };
    let app = App::new(offset, "trace".to_owned(), come_in);
    app.keep_time();
    let mut s = Scenario::new(app.view(), dir, 1440, 900, 2.0)?;
    // The window draws, asks for the models it shows, and is told them: until it shows what the core holds.
    let answered = |s: &mut Scenario<_>| -> Result<(), ScenarioError> {
        for _ in 0..4 {
            s.act("the core answers", || faked.settle());
            s.settle()?;
        }
        Ok(())
    };

    s.frame("way in")?;
    let (x, y) = s.locate("http://127.0.0.1:4320/#key=…")?;
    s.click(x, y);
    s.type_text("not a link");
    s.click_text("Come in")?;
    s.frame("not a link")?;

    let (x, y) = s.locate("http://127.0.0.1:4320/#key=…")?;
    s.click(x, y);
    s.type_text("http://127.0.0.1:4320/#key=trace");
    s.click_text("Come in")?;
    answered(&mut s)?;
    s.frame("board")?;

    s.act("the server goes down", || faked.lose());
    answered(&mut s)?;
    s.frame("link down")?;

    s.act("the server is back", || faked.come_back());
    answered(&mut s)?;
    s.frame("link back")?;

    s.click_text("Dashboard totals drift after a refund")?;
    answered(&mut s)?;
    s.frame("a chain open on its Chair")?;

    s.click_text("Pause")?;
    s.frame("pause asks why")?;
    s.click_text("Cancel")?;

    s.click_text("Engineer")?;
    answered(&mut s)?;
    s.frame("the Engineer, asking the owner")?;

    s.click_text("From Chair")?;
    s.frame("the Chair's message to it, opened")?;

    s.click_text("Stop")?;
    s.frame("stop asks first")?;
    s.click_text("Cancel")?;

    let (x, y) = s.locate("Message Engineer… (⌘↩ sends)")?;
    s.click(x, y);
    s.type_text("Use the order day, as finance does.");
    s.click_text("Send")?;
    answered(&mut s)?;
    s.frame("a message sent, queued behind the turn")?;

    s.click_text("Board")?;
    answered(&mut s)?;
    s.click_text("Dashboard totals drift after a refund")?;
    answered(&mut s)?;
    s.frame("the chain opened again")?;

    s.click_text("Assistant")?;
    answered(&mut s)?;
    s.frame("the Assistant beside the chain")?;

    s.click_text("Decisions")?;
    answered(&mut s)?;
    s.frame("what the Assistant decided")?;

    s.click_text("Overturn")?;
    s.frame("overturn asks what instead")?;
    s.click_text("Cancel")?;

    s.click_text("Work")?;
    s.click_text("Signals")?;
    s.frame("an area the desktop does not draw yet")?;
    Ok(())
}

/// What the fake server answers: the seed's snapshot, and the one chain it opens.
fn answer(call: &Call) -> Option<Value> {
    match call {
        Call::app_snapshot {} => Some(snapshot()),
        Call::chain_get { id } if id == "run1" => Some(opened()),
        Call::chain_events { id } if id == "run1" => Some(json!({ "events": events(), "truncated": false })),
        // What the owner types is saved as the server saves it: its next revision.
        Call::draft_save { draft, base_rev, .. } => Some(record(model::Draft {
            id: draft.id.clone(),
            home: draft.home.clone(),
            text: draft.text.clone(),
            card_id: draft.card_id.clone(),
            rev: base_rev.saturating_add(1),
            ..model::Draft::default()
        })),
        Call::draft_delete { .. } | Call::seen_mark { .. } | Call::overturn { .. } | Call::card_restart { .. } => {
            Some(json!({}))
        }
        Call::card_events { card_id, .. } if card_id == "a-card" => {
            Some(json!({ "events": conversation(), "truncated": false }))
        }
        // The one message the trace sends, queued behind the Engineer's turn.
        Call::card_prompt { .. } => Some(sent()),
        _ => None,
    }
}

fn record(value: impl serde::Serialize) -> Value {
    serde_json::to_value(value).unwrap_or_default()
}

/// The agents of the chain the trace opens: its Chair, an Engineer asking the owner a question, a Reviewer judging.
fn cards() -> Vec<Value> {
    let card = |id: &str, name: &str, origin: &str| {
        let mut card = record(model::Card::default());
        if let Some(c) = card.as_object_mut() {
            for (field, value) in [("id", id), ("home", "run1"), ("name", name), ("origin", origin)] {
                c.insert(field.to_owned(), json!(value));
            }
        }
        card
    };
    vec![
        card("chair", "Chair", "chair"),
        card("eng", "Engineer", "hired"),
        card("rev", "Reviewer", "judge"),
    ]
}

/// The Engineer's question to the owner.
fn question() -> model::InboxItem {
    let option = |id: &str, name: &str, kind: &str| model::InboxOption {
        id: id.to_owned(),
        name: name.to_owned(),
        kind: kind.to_owned(),
    };
    model::InboxItem {
        id: "q-refund".to_owned(),
        home: "run1".to_owned(),
        card_id: "eng".to_owned(),
        kind: model::InboxKind::Question,
        title: "Do refunds count against the day of the order or the day of the refund?".to_owned(),
        detail:
            "The dashboard subtracts them on the refund day; finance books them on the order day. Each choice moves \
                 last month's total by about 2%."
                .to_owned(),
        options: vec![
            option("order", "The order day", "allow_once"),
            option("refund", "The refund day", "allow_once"),
        ],
        ..model::InboxItem::default()
    }
}

fn events() -> Vec<Value> {
    let event =
        |id: i64, card: &str, status: model::EventStatus, title: &str, text: &str, by: Option<model::ActorKind>| {
            record(wire::app::EventRead {
                event: model::Event {
                    id,
                    home: "run1".to_owned(),
                    card_id: card.to_owned(),
                    status,
                    tool_name: if title.is_empty() {
                        String::new()
                    } else {
                        "Bash".to_owned()
                    },
                    title: title.to_owned(),
                    text: text.to_owned(),
                    by: by.map(|kind| model::Actor {
                        kind,
                        who: if kind == model::ActorKind::Agent {
                            "Chair".to_owned()
                        } else {
                            String::new()
                        },
                        via: String::new(),
                    }),
                    ..model::Event::default()
                },
                activity: None,
            })
        };
    use model::{ActorKind, EventStatus as S, PromptStatus, StopReason, ToolStatus};
    vec![
        event(
            1,
            "chair",
            S::Prompt(PromptStatus::Delivered),
            "",
            "Dashboard totals drift after a refund: find why and fix it.",
            Some(ActorKind::Owner),
        ),
        event(
            2,
            "chair",
            S::Message,
            "",
            "I hire an Engineer to find where the totals part ways, and a Reviewer to judge the fix.",
            None,
        ),
        event(3, "chair", S::Stop(StopReason::EndTurn), "", "", None),
        event(
            4,
            "eng",
            S::Prompt(PromptStatus::Delivered),
            "",
            "Find where the dashboard's daily total and finance's part ways after a refund.",
            Some(ActorKind::Agent),
        ),
        event(
            5,
            "eng",
            S::Thought,
            "",
            "The drift shows only on days with refunds, so the refunds table is the first place to read.",
            None,
        ),
        event(
            6,
            "eng",
            S::Tool(ToolStatus::Completed),
            "rg -n refund_at sales-engine/src",
            "",
            None,
        ),
        event(
            7,
            "eng",
            S::Tool(ToolStatus::Completed),
            "psql -c 'select day, sum(amount) from refunds group by day'",
            "",
            None,
        ),
        event(
            8,
            "eng",
            S::Message,
            "",
            "The dashboard subtracts a refund on the day it happens; finance subtracts it on the day of the order. Which is right is the owner's call, so I asked.",
            None,
        ),
        event(
            9,
            "eng",
            S::Tool(ToolStatus::InProgress),
            "cargo test -p sales-engine refunds",
            "",
            None,
        ),
    ]
}

fn sent() -> Value {
    record(wire::app::EventRead {
        event: model::Event {
            id: 10,
            home: "run1".to_owned(),
            card_id: "eng".to_owned(),
            status: model::EventStatus::Prompt(model::PromptStatus::Queued),
            text: "Use the order day, as finance does.".to_owned(),
            by: Some(model::Actor {
                kind: model::ActorKind::Owner,
                who: String::new(),
                via: String::new(),
            }),
            ..model::Event::default()
        },
        activity: None,
    })
}

/// The Assistant: its card, where it stands, what it may be asked.
fn assistant() -> Value {
    let mut card = record(model::Card::default());
    if let Some(c) = card.as_object_mut() {
        for (field, value) in [
            ("id", "a-card"),
            ("home", "asst"),
            ("name", "Assistant"),
            ("origin", "assistant"),
        ] {
            c.insert(field.to_owned(), json!(value));
        }
    }
    json!({
        "assistant": { "id": "asst", "cardId": "a-card", "createdAt": iso(5000) },
        "card": card,
        "agent": standing_of("a-card", "asst", "Idle", "idle", "", "", &[], &["message", "restart"]),
        "drafts": [],
    })
}

/// The owner's conversation with the Assistant so far.
fn conversation() -> Vec<Value> {
    let event = |id: i64, status: model::EventStatus, text: &str, by: Option<model::ActorKind>| {
        record(wire::app::EventRead {
            event: model::Event {
                id,
                home: "asst".to_owned(),
                card_id: "a-card".to_owned(),
                status,
                text: text.to_owned(),
                by: by.map(|kind| model::Actor {
                    kind,
                    who: String::new(),
                    via: String::new(),
                }),
                ..model::Event::default()
            },
            activity: None,
        })
    };
    use model::{ActorKind, EventStatus as S, PromptStatus, StopReason};
    vec![
        event(
            101,
            S::Prompt(PromptStatus::Delivered),
            "What is stuck this morning?",
            Some(ActorKind::Owner),
        ),
        event(
            102,
            S::Message,
            "One chain: sales-engine cannot start its Engineer (spawn claude ENOENT). I restarted it once; it failed the same way, so the machine lacks the claude binary. Everything else runs.",
            None,
        ),
        event(103, S::Stop(StopReason::EndTurn), "", None),
    ]
}

/// What the Assistant decided for the owner: an answer in its overturn window, one a second judge disputes, one he
/// overturned, and a question it left to him.
fn feed() -> Value {
    let decision = |id: &str, text: &str, why: &str, ago: i64| {
        let mut d = record(model::Decision::default());
        if let Some(o) = d.as_object_mut() {
            o.insert("id".to_owned(), json!(id));
            o.insert("text".to_owned(), json!(text));
            o.insert("why".to_owned(), json!(why));
            o.insert("createdAt".to_owned(), json!(iso(ago)));
        }
        d
    };
    let in_an_hour = OffsetDateTime::now_utc()
        .checked_add(Duration::hours(1))
        .and_then(|t| t.format(&Rfc3339).ok())
        .unwrap_or_default();
    json!({
        "lines": [
            { "decision": decision("d1", "Allowed the Engineer to drop the staging copy of the refunds table", "It is rebuilt every night and nothing reads it during the day.", 3), "answered": true, "takesEffectAt": in_an_hour },
            { "decision": decision("d2", "Answered the Chair: refunds count on the order day", "Finance books them on the order day; the dashboard should agree with finance.", 20), "answered": true,
              "dissent": decision("d2x", "Refunds count on the refund day", "The dashboard shows cash movement, which happens on the refund day.", 19) },
            { "decision": decision("d3", "Approved the merge of the retention change", "Both gates passed and the change is behind a flag.", 90), "answered": true,
              "overturn": decision("d3o", "Overturned", "Wait for the data team's review on Monday.", 60) },
            { "decision": decision("d4", "Left to you: whether to pay for a second Amplitude seat", "It spends money, which only you decide.", 240), "answered": false },
        ],
        "unseen": 2,
    })
}

/// Where an agent stands, as the server says it.
#[expect(clippy::too_many_arguments, reason = "one argument per field the seed varies")]
fn standing_of(
    card: &str,
    home: &str,
    label: &str,
    tone: &str,
    doing: &str,
    work: &str,
    waiting: &[&str],
    asks: &[&str],
) -> Value {
    json!({ "cardId": card, "home": home, "label": label, "tone": tone, "doing": doing, "doingTone": "busy", "work": work,
            "warms": false, "waiting": waiting, "children": [], "with": [], "peers": [],
            "branch": { "agents": 0, "turns": 0, "working": false }, "asks": asks,
            "stopSays": "It stops at once; what it was doing is lost and the Chair is told.", "remembers": true, "rev": 1 })
}

/// The chain the trace opens, as `chain_get` reads it.
fn opened() -> Value {
    let agent = |card: &str, label: &str, tone: &str, doing: &str, work: &str, waiting: &[&str], asks: &[&str]| {
        standing_of(card, "run1", label, tone, doing, work, waiting, asks)
    };
    // The board's "Dashboard totals drift after a refund", which the owner may pause.
    let seed = snapshot();
    let chain = seed.pointer("/chains/3").cloned().unwrap_or_default();
    let mut standing = seed.pointer("/standings/3").cloned().unwrap_or_default();
    if let Some(s) = standing.as_object_mut() {
        s.insert("asks".to_owned(), json!(["pause"]));
        // Read after the snapshot: the core keeps a standing's newer revision.
        s.insert("rev".to_owned(), json!(2));
    }
    json!({
        "chain": chain,
        "cards": cards(),
        "edges": [], "decisions": [], "drafts": [], "workflows": [], "helps": [],
        "agents": [
            agent("chair", "Waiting", "waiting", "waits for the Engineer", "", &[], &["message"]),
            agent("eng", "Needs you", "needs", "runs the refund tests", "Find why the totals drift", &["q-refund"], &["message", "stop"]),
            agent("rev", "Idle", "idle", "", "Judge the fix", &[], &["message"]),
        ],
        "verification": { "chainId": "run1", "gates": [], "judges": [], "rev": 1 },
        "standing": standing,
        "retrospectives": [], "landings": [], "datadog": [],
    })
}

fn iso(ago_minutes: i64) -> String {
    OffsetDateTime::now_utc()
        .checked_sub(Duration::minutes(ago_minutes))
        .and_then(|t| t.format(&Rfc3339).ok())
        .unwrap_or_default()
}

fn snapshot() -> Value {
    let chain = |id: &str, title: &str, status: &str, age: i64| json!({ "chain": { "id": id, "title": title, "status": status, "chairId": "chair", "createdAt": iso(age), "updatedAt": iso(age / 3) } });
    let wait = |id: &str, chain: &str, kind: &str, label: &str, ask: &str, since: i64| json!({ "id": id, "kind": kind, "label": label, "chainId": chain, "who": "Engineer", "ask": ask, "since": iso(since), "unseen": false });
    let standing = |chain: &str, text: &str, tone: &str, wait: Option<Value>, waits: i64, unseen: bool| {
        let mut s = json!({ "chainId": chain, "text": text, "tone": tone, "working": tone == "running", "waits": waits, "unseen": unseen, "asks": [], "from": "", "rev": 1 });
        if let (Some(w), Some(fields)) = (wait, s.as_object_mut()) {
            fields.insert("wait".to_owned(), w);
        }
        s
    };
    let column = |key: &str, label: &str, empty: &str, chains: &[&str], items: &[&str]| json!({ "key": key, "label": label, "empty": empty, "chains": chains, "items": items });
    let waits = [
        wait(
            "w1",
            "key",
            "key",
            "Key",
            "StarRocks needs a password for connection prod-read",
            30,
        ),
        wait(
            "w2",
            "stuck",
            "failed",
            "Failed",
            "Engineer failed: spawn claude ENOENT",
            45 * 60,
        ),
        wait(
            "w3",
            "signin",
            "sign_in",
            "Sign in",
            "Sign in to Amplitude for the growth project",
            8,
        ),
        json!({ "id": "left", "kind": "decide", "label": "Decide", "who": "Remit", "ask": "Keep or delete the files", "since": iso(90), "unseen": true }),
    ];
    json!({
        "who": { "kind": "owner" },
        "chains": [
            chain("key", "Read the orders from StarRocks", "running", 90),
            chain("stuck", "sales-engine: the Engineer cannot start", "running", 50 * 60),
            chain("signin", "Weekly Amplitude digest", "running", 20),
            chain("run1", "Dashboard totals drift after a refund", "running", 35),
            chain("run2", "Retention of old events", "running", 12),
            chain("wait1", "Cut the sync timeout to 30 seconds", "waiting", 240),
            chain("watch1", "Checkout error rate stays low after the coupon change", "waiting", 600),
            chain("q1", "Refund webhook retries forever", "queued", 4),
            chain("done1", "sales-engine: backfill misses the install-day orders (DEMO-5385)", "done", 300),
            chain("done2", "Scout · 25/10 09:31", "done", 900),
            chain("done3", "Hire a data analyst for the growth objective", "done", 2000),
        ],
        "inbox": [
            { "id": "left", "home": "", "kind": "error", "title": "A deleted chain left 2.1 GB of files in its work directory", "status": "open", "createdAt": iso(90) },
            record(question()),
        ],
        "standings": [
            standing("key", "Needs you", "needs", Some(waits[0].clone()), 1, false),
            standing("stuck", "Failed", "failed", Some(waits[1].clone()), 2, false),
            standing("signin", "Needs you", "needs", Some(waits[2].clone()), 1, false),
            standing("run1", "Running · the Engineer reads the refunds table", "running", None, 0, false),
            standing("run2", "Running · a test fails before the fix", "running", None, 0, false),
            standing("wait1", "Waiting · in the merge queue of g/r", "waiting", None, 0, false),
            standing("watch1", "Watching · the error rate stays under 2% for a day", "watching", None, 0, false),
            standing("q1", "Queued · second in line", "queued", None, 0, false),
            standing("done1", "Done · landed in g/r with a test", "done", None, 0, true),
            standing("done2", "Done", "done", None, 0, true),
            standing("done3", "Not done · the analyst's first report was not proven", "notdone", None, 0, false),
        ],
        "board": {
            "columns": [
                column("needs", "Needs you", "Nothing waits for you.", &["key", "stuck", "signin"], &["left"]),
                column("running", "Running", "No agent is at work.", &["run1", "run2"], &[]),
                column("waiting", "Waiting", "No chain waits.", &["wait1"], &[]),
                column("watching", "Watching", "No Chair watches a signal.", &["watch1"], &[]),
                column("queued", "Queued", "Nothing is queued.", &["q1"], &[]),
                column("finished", "Finished", "No chain has finished yet.", &["done1", "done2", "done3"], &[]),
            ],
            "archived": 0, "needsYou": 4, "waiting": waits, "rev": 1,
        },
        "playbooks": [], "agents": [], "skills": [], "sources": [], "capabilities": [], "insights": [], "events": [],
        "eventSignals": [], "sensors": [], "clientKeys": [], "extensions": [], "feedback": [], "objectives": [],
        "projects": [], "employees": [], "mentors": [], "mentorStandings": [],
        "health": { "ok": true, "accounts": [], "unreadable": 0, "rev": 1 },
        "workload": { "running": 2, "capacity": 3, "pressure": "", "queued": 1, "waiting": 2, "rev": 1 },
        "signals": { "sources": [], "subscriptions": [], "signals": [], "signalsMore": 0, "rev": 1 },
        "boards": serde_json::to_value(wire::app::Boards::default()).unwrap_or_default(),
        "assistant": assistant(),
        "assistantFeed": feed(),
    })
}
