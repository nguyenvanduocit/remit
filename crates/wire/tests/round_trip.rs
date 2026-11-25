//! A representative call and push of each contract cross a link in memory and arrive as the types they left as.

use link::Payload;
use serde_json::json;
use wire::app;

// ADR-0091: a block event as a server pushes it reads back as the types it left as, each part kind, and a block stored
// before a choice said `many` or the owner picked reads as a single choice still the agent's.
#[test]
fn a_stored_block_of_each_part_kind_reads_across_versions() {
    let pushed = json!({
        "event": {
            "id": 7, "home": "c1", "cardId": "k1", "kind": "system", "status": "block", "toolCallId": "",
            "toolKind": "", "toolName": "", "title": "Cut the migration", "text": "",
            "createdAt": "2026-11-13T09:00:00+07:00", "updatedAt": "2026-11-13T09:05:00+07:00", "rev": 2
        },
        "activity": {"type": "block", "block": {
            "spec": {"title": "Cut the migration", "parts": [
                {"type": "choice", "id": "how", "label": "Approach", "many": false, "picked": ["two"], "options": [
                    {"id": "one", "label": "One step", "detail": "fast, locks the table"},
                    {"id": "two", "label": "Expand, then contract"}
                ]},
                {"type": "chart", "id": "rows", "label": "Rows a day", "kind": "line", "x": ["Mon", "Tue"],
                 "series": [{"name": "rows", "values": [3.5, 5.0]}], "unit": "k"}
            ]},
            "pickedBy": {"kind": "owner", "who": "alexp", "via": "web"},
            "pickedAt": "2026-11-13T09:05:00+07:00"
        }}
    });
    let event: app::EventRead = serde_json::from_value(pushed.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(&event).unwrap(),
        pushed,
        "a block reads back as stored"
    );
    let crossed: app::EventRead = Payload::encode(&event).unwrap().decode().unwrap();
    assert_eq!(crossed, event, "a block crosses the link unchanged");

    let older: app::Block = serde_json::from_value(json!({"spec": {"title": "t", "parts": [
        {"type": "choice", "id": "p", "label": "l", "picked": ["a"], "options": [{"id": "a", "label": "A"}, {"id": "b", "label": "B"}]},
        {"type": "chart", "id": "c", "label": "l", "kind": "bar", "x": ["x"], "series": [{"name": "s", "values": [1.0]}]}
    ]}}))
    .unwrap();
    assert!(
        matches!(
            older.spec.parts.first(),
            Some(app::BlockPart::Choice { many: false, .. })
        ) && older.picked_by.is_none(),
        "an older block reads as a single choice still the agent's: {older:?}"
    );
}

// ADR-0114, ADR-0116: a landing as a chain's stream is pushed it, with the release that carried it, reads back as stored
// and crosses the link as the types it left as, each tagged state too; a landing pushed before a release carried it, and
// one stored before its later fields, read with none of them.
#[test]
fn a_landing_with_its_release_reads_across_versions() {
    let owner = json!({"kind": "owner", "who": "alexp", "via": "web"});
    let pushed = json!({
        "landing": {
            "id": "l_1", "chainId": "c1", "repo": "pixfx-rs", "host": "github.com", "project": "acme/pixfx", "base": "main",
            "version": {"number": 2, "from": "a1b2", "forked": "f0", "change": "p9", "gates": ["reviewer", "qc"],
                "verdicts": ["v1", "v2"], "by": ["e1"], "title": "feat: blur", "description": "Adds a blur.",
                "paths": ["src/blur.rs"], "judged": ["j1"]},
            "priority": "high", "priorityWhy": "the customer waits on it",
            "state": {"kind": "reverted", "revert": "l_2", "cause": "guardrail", "by": owner, "why": "the release was taken back"},
            "batch": "b1", "commit": "c0ffee", "checks": {"state": "passed", "failed": ""}, "release": "r1",
            "createdAt": "2026-11-19T09:00:00+07:00", "readyAt": "2026-11-19T09:10:00+07:00",
            "landedAt": "2026-11-19T09:20:00+07:00", "revertedAt": "2026-11-19T10:00:00+07:00",
            "updatedAt": "2026-11-19T10:00:00+07:00", "rev": 4
        },
        "release": {
            "id": "r1", "repo": "pixfx-rs", "host": "github.com", "project": "acme/pixfx", "base": "main",
            "commit": "c0ffee", "previous": "beef", "top": "l_1", "tries": 1, "waits": "", "steps": [], "back": [],
            "guards": [],
            "state": {"kind": "taken_back", "by": owner, "why": "error rate breached"},
            "version": "1.4.0", "links": ["https://crates.io/crates/pixfx/1.4.0"],
            "createdAt": "2026-11-19T09:30:00+07:00", "bakeUntil": "2026-11-19T11:30:00+07:00",
            "endedAt": "2026-11-19T10:00:00+07:00", "rev": 3
        }
    });
    let read: app::LandingRead = serde_json::from_value(pushed.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(&read).unwrap(),
        pushed,
        "a landing reads back as pushed"
    );
    let crossed: app::LandingRead = Payload::encode(&read).unwrap().decode().unwrap();
    assert_eq!(crossed, read, "a landing crosses the link unchanged");

    let older: app::LandingRead = serde_json::from_value(json!({"landing": {
        "id": "l_3", "chainId": "c1", "repo": "pixfx-rs", "base": "main", "state": {"kind": "ready"}
    }}))
    .unwrap();
    assert!(
        older.release.is_none() && older.landing.release.is_none() && older.landing.state == model::LandingState::Ready,
        "an older landing reads with no release: {older:?}"
    );
}

// ADR-0107 step 4d: the library's and the crew's records cross as `model`'s own, filled: an agent with its loadout (once
// flattened among its own keys, which the link's encoding cannot carry), a change with the record before and after (which
// the hand-written copy dropped), and an employee as a channel reads it.
#[test]
fn an_agent_a_change_and_an_employee_cross_the_link_unchanged() {
    let at = model::Time::try_from("2026-11-17T08:00:00Z".to_owned()).unwrap();
    let owner = model::Actor {
        kind: model::ActorKind::Owner,
        who: "alexp".into(),
        via: "web".into(),
    };
    let agent = model::Agent {
        id: "builder".into(),
        name: "Builder".into(),
        description: "Builds".into(),
        group: "engineering".into(),
        model: "opus".into(),
        effort: "high".into(),
        loadout: model::Loadout {
            instructions: "Work carefully".into(),
            tools: Some(vec!["Read".into(), "Bash".into()]),
            skills: vec!["rust".into()],
        },
        signs: model::Signs::Review,
        title: Some(model::Title::Worker),
        source: Some(model::Source {
            kind: "claude-code".into(),
            name: "builder".into(),
            hash: "h1".into(),
            at,
        }),
        builtin: true,
        edited: true,
        rev: 3,
    };
    let change = model::Change {
        id: "ch_1".into(),
        kind: model::ChangeKind::Agent,
        name: "builder".into(),
        action: model::ChangeAction::Changed,
        before: "{}".into(),
        after: "{\"id\":\"builder\"}".into(),
        fields: vec![model::ChangedField {
            name: "tools".into(),
            before: String::new(),
            after: "[\"Bash\"]".into(),
        }],
        by: owner.clone(),
        approved_by: Some(owner),
        at,
    };
    let employee = app::EmployeeRead {
        employee: model::Employee {
            id: "e_1".into(),
            number: 4,
            label: "Login".into(),
            template: "builder".into(),
            template_rev: 3,
            loadout: agent.loadout.clone(),
            scope: "the login page".into(),
            reports_to: model::Accountable::owner(),
            hired_by: model::Accountable::owner(),
            rev: 2,
            created_at: at,
            ..model::Employee::default()
        },
        shown: "Login · Builder #4".into(),
        tag: "Builder #4".into(),
        face: app::Face {
            shape: app::FaceShape::Hexagon,
            hue: 210,
            accessory: app::FaceAccessory::Visor,
        },
    };

    let crossed: model::Agent = Payload::encode(&agent).unwrap().decode().unwrap();
    assert_eq!(crossed, agent, "an agent crosses the link with its loadout");
    let crossed: model::Change = Payload::encode(&change).unwrap().decode().unwrap();
    assert_eq!(
        crossed, change,
        "a change crosses the link with its record before and after"
    );
    let crossed: app::EmployeeRead = Payload::encode(&employee).unwrap().decode().unwrap();
    assert_eq!(crossed, employee, "an employee crosses the link as the server words it");
}
