//! The store's main paths, driven as orch drives them: open, migrate, write through `update`, read through `read`.
#![expect(
    clippy::disallowed_methods,
    reason = "the test sets its fixture up and drives the program as a person would, outside the code under test"
)]

use std::path::{Path, PathBuf};

use model::Time;
use rusqlite::Connection;
use store::{Error, Store};

fn db(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("remit.db")
}

/// Reads one value of the database at path on a connection of its own, read-only.
fn peek(path: &Path, query: &str) -> rusqlite::Result<i64> {
    let c = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    c.query_row(query, [], |r| r.get(0))
}

#[tokio::test(flavor = "multi_thread")]
async fn an_empty_database_migrates_to_the_last_and_opens_again_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let path = db(&dir);
    Store::open(&path).unwrap().close().unwrap();
    assert_eq!(peek(&path, "PRAGMA user_version").unwrap(), 131);
    assert_eq!(peek(&path, "SELECT count(*) FROM schema_migrations").unwrap(), 131);
    let objects = peek(
        &path,
        "SELECT count(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'",
    );
    // A second open finds nothing to run and takes no snapshot: the database has nothing pending.
    let s = Store::open(&path).unwrap();
    assert!(
        matches!(Store::open(&path), Err(Error::InUse(_))),
        "a second process may not open a held database"
    );
    s.close().unwrap();
    assert_eq!(
        peek(
            &path,
            "SELECT count(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'"
        ),
        objects
    );
    assert!(store::snapshots(&path).unwrap().is_empty());
}

/// Rewinds the database at path to version (108: before the migrations of ADR-0107 step 3a) and writes rows into it by
/// raw SQL, as an earlier build left them: the next open runs every later migration over them.
fn as_an_earlier_build_left_it(path: &Path, version: i64, rows: &str) -> Result<(), Box<dyn std::error::Error>> {
    Store::open(path)?.close()?;
    Connection::open(path)?.execute_batch(&format!(
        "DELETE FROM schema_migrations WHERE version > {version}; PRAGMA user_version = {version};
        ALTER TABLE attachments DROP COLUMN name; ALTER TABLE attachments RENAME TO images;
        DROP INDEX attachments_home; CREATE INDEX images_home ON images(home);
        {rows}"
    ))?;
    Ok(())
}

// Documents an earlier build wrote (Go's or this one's) read after migrating, each in its table: an enum key holding the
// empty text, the zero time where a time may be unset, a list or a map stored as null, checks with no state, Go's HTML
// escapes and a time in a local offset.
#[tokio::test(flavor = "multi_thread")]
async fn documents_an_earlier_build_wrote_read_after_migrating() {
    let dir = tempfile::tempdir().unwrap();
    let path = db(&dir);
    as_an_earlier_build_left_it(
        &path,
        108,
        r#"
INSERT INTO chains(id, created_at, doc) VALUES('c_old', 0, '{"id":"c_old","title":"Fix <the> login & signup","status":"done","origin":"","unjudged":"","repos":[{"repo":"r","branch":"b","mr":{"iid":1,"url":"u1","state":"opened","priority":"","updatedAt":"0001-01-01T00:00:00Z","decidedAt":"0001-01-01T00:00:00Z","checks":{"state":""}},"earlierMrs":[{"iid":0,"state":"closed","checks":{"state":""}},{"iid":2,"state":"merged","updatedAt":"2026-11-09T10:00:00Z","decidedAt":"2026-11-09T12:00:00Z","checks":{"state":"failed","failed":"lint"}}]}],"createdAt":"2026-11-10T17:00:00.12+07:00","updatedAt":"2026-11-10T17:00:00.12+07:00","rev":1}');
INSERT INTO cards(id, home, created_at, doc) VALUES('k_old', 'c_old', 0, '{"id":"k_old","home":"c_old","agent":"worker","signs":"","origin":"hired","status":"idle","plan":null,"loadout":{"instructions":"i","skills":null},"createdAt":"2026-11-10T17:00:00+07:00","rev":1}');
INSERT INTO cards(id, home, created_at, doc) VALUES('k_intake', 'c_old', 1, '{"id":"k_intake","home":"c_old","agent":"intake","signs":"","origin":"judge","status":"stopped","createdAt":"2026-11-10T17:00:00+07:00","rev":1}');
INSERT INTO cards(id, home, created_at, doc) VALUES('k_goal', 'c_old', 2, '{"id":"k_goal","home":"c_old","agent":"judge","signs":"goal","origin":"judge","status":"idle","createdAt":"2026-11-10T17:00:00+07:00","rev":1}');
INSERT INTO cards(id, home, created_at, doc) VALUES('k_check', 'c_old', 3, '{"id":"k_check","home":"c_old","agent":"judge","signs":"check","origin":"judge","status":"idle","createdAt":"2026-11-10T17:00:00+07:00","rev":1}');
INSERT INTO cards(id, home, created_at, doc) VALUES('k_duel', 'c_old', 4, '{"id":"k_duel","home":"c_old","agent":"duel_judge","signs":"","origin":"judge","status":"stopped","createdAt":"2026-11-10T17:00:00+07:00","rev":1}');
INSERT INTO cards(id, home, created_at, doc) VALUES('k_second', 'c_old', 5, '{"id":"k_second","home":"c_old","agent":"second_opinion","signs":"","origin":"judge","status":"stopped","createdAt":"2026-11-10T17:00:00+07:00","rev":1}');
INSERT INTO cards(id, home, created_at, doc) VALUES('k_hired_judge', 'c_old', 6, '{"id":"k_hired_judge","home":"c_old","agent":"judge","signs":"","origin":"hired","status":"idle","loadout":{"instructions":"i"},"createdAt":"2026-11-10T17:00:00+07:00","rev":1}');
INSERT INTO approvals(id, chain_id, at, doc) VALUES('a_old', 'c_old', 0, '{"id":"a_old","chainId":"c_old","role":"judge","verdict":"approve","findings":[{"item":"x","status":"met","blocker":"","code":""}],"at":"2026-11-10T17:00:00+07:00"}');
INSERT INTO chains(id, created_at, doc) VALUES('c_fin', 1, '{"id":"c_fin","status":"running","repos":[{"repo":"r","mr":{"iid":3,"url":"u3","state":"opened","change":"p1","gates":["reviewer","qc","security"]}}],"finishing":{"summary":"done","judge":"k_judge","at":"2026-11-10T17:00:00+07:00","verdict":"a_fin","recheck":false},"createdAt":"2026-11-10T17:00:00+07:00","rev":1}');
INSERT INTO chains(id, created_at, doc) VALUES('c_judging', 2, '{"id":"c_judging","status":"running","finishing":{"summary":"done","judge":"k_judge2","at":"2026-11-10T17:00:00+07:00","verdict":"","recheck":false},"createdAt":"2026-11-10T17:00:00+07:00","rev":1}');
INSERT INTO approvals(id, chain_id, at, doc) VALUES('a_rev', 'c_fin', 1, '{"id":"a_rev","chainId":"c_fin","repo":"r","change":"p1","role":"reviewer","verdict":"approve","findings":[],"at":"2026-11-10T17:00:00+07:00"}');
INSERT INTO approvals(id, chain_id, at, doc) VALUES('a_qc', 'c_fin', 2, '{"id":"a_qc","chainId":"c_fin","repo":"r","change":"p1","role":"qc","verdict":"request_changes","findings":[],"at":"2026-11-10T17:00:00+07:00"}');
INSERT INTO approvals(id, chain_id, at, doc) VALUES('a_sec', 'c_fin', 3, '{"id":"a_sec","chainId":"c_fin","repo":"r","change":"p1","role":"security","verdict":"approve","findings":[],"at":"2026-11-10T17:00:00+07:00"}');
INSERT INTO approvals(id, chain_id, at, doc) VALUES('a_chk', 'c_fin', 4, '{"id":"a_chk","chainId":"c_fin","role":"check_review","verdict":"approve","findings":[],"at":"2026-11-10T17:00:00+07:00"}');
INSERT INTO approvals(id, chain_id, at, doc) VALUES('a_fin', 'c_fin', 5, '{"id":"a_fin","chainId":"c_fin","role":"judge","verdict":"request_changes","findings":[],"at":"2026-11-10T17:00:00+07:00"}');
INSERT INTO inbox(id, home, status, created_at, doc) VALUES('i_old', 'c_old', 'open', 0, '{"id":"i_old","home":"c_old","kind":"permission","status":"open","options":null,"changeKind":"","permission":{"outcome":""},"change":{"kind":"agent","name":"worker","record":"{}","fields":null},"replay":{"by":{"kind":"remit"},"at":"2026-11-10T17:00:00+07:00","briefs":null},"createdAt":"2026-11-10T17:00:00+07:00"}');
INSERT INTO cards(id, home, created_at, doc) VALUES('k_asks_chair', 'c_asks', 0, '{"id":"k_asks_chair","home":"c_asks","origin":"hired","status":"permission","activity":"waiting for the Chair: Run kubectl rollout restart","createdAt":"2026-11-10T17:00:00+07:00","rev":1}');
INSERT INTO cards(id, home, created_at, doc) VALUES('k_asks_owner', 'c_asks', 1, '{"id":"k_asks_owner","home":"c_asks","origin":"chair","status":"permission","activity":"waiting for permission: Run rm -rf build","createdAt":"2026-11-10T17:00:00+07:00","rev":1}');
INSERT INTO cards(id, home, created_at, doc) VALUES('k_asked_once', 'c_asks', 2, '{"id":"k_asked_once","home":"c_asks","origin":"hired","status":"idle","activity":"waiting for permission: Run ls","createdAt":"2026-11-10T17:00:00+07:00","rev":1}');
INSERT INTO agents(id, doc) VALUES('ag_old','{"id":"ag_old","name":"worker","signs":"","title":"","tools":null,"skills":null}');
INSERT INTO agents(id, doc) VALUES('ag_flat', '{"id":"ag_flat","name":"builder","instructions":"Work <carefully>","tools":["Read","Bash"],"skills":["rust"],"rev":2}');
INSERT INTO agents(id, doc) VALUES('ag_none', '{"id":"ag_none","name":"reader","instructions":"Read","tools":[]}');
INSERT INTO inbox(id, home, status, created_at, doc) VALUES('i_agent', 'c_old', 'open', 1, '{"id":"i_agent","home":"c_old","kind":"change","status":"open","change":{"kind":"agent","name":"builder","record":"{\"id\":\"builder\",\"instructions\":\"Build\",\"tools\":[\"Read\"],\"skills\":[\"go\"]}","fields":[]},"createdAt":"2026-11-10T17:00:00+07:00"}');
INSERT INTO employees(id, created_at, doc) VALUES('e_old', 0, '{"id":"e_old","loadout":{"instructions":"i","skills":null},"createdAt":"2026-11-10T17:00:00+07:00"}');
INSERT INTO objectives(id, created_at, doc) VALUES('o_old', 0, '{"id":"o_old","rev":1,"status":"active","keyResults":[{"id":"kr1","check":{"kind":"","expiry":"0001-01-01T00:00:00Z"}},{"id":"kr2","check":{"how":"","reason":""}},{"id":"kr3","check":{"kind":"submit","how":""}}],"surrogates":[{"kpi":"k","expires":"0001-01-01T00:00:00Z"}],"horizon":{"text":null,"until":"0001-01-01T00:00:00Z"},"createdAt":"2026-11-10T17:00:00+07:00"}');
INSERT INTO objectives(id, created_at, doc) VALUES('o_none', 1, '{"id":"o_none","rev":1,"status":"active","keyResults":null,"createdAt":"2026-11-10T17:00:00+07:00"}');
INSERT INTO subscriptions(id, owner_kind, owner_id, match, seen_seq, doc) VALUES('s_old', 'chain', 'c_old', 'fact.x', 0, '{"id":"s_old","owner":{"kind":"chain","id":"c_old"},"match":"fact.x","flow":{"policy":""},"control":"","until":"0001-01-01T00:00:00Z","dueAt":"0001-01-01T00:00:00Z","lastFired":"0001-01-01T00:00:00Z","action":{"kind":"wake","why":"w"},"start":{"from":"now"},"createdAt":"2026-11-10T17:00:00+07:00"}');
INSERT INTO subscriptions(id, owner_kind, owner_id, match, seen_seq, doc) VALUES('s_cron', 'chain', 'c_old', '', 0, '{"id":"s_cron","owner":{"kind":"chain","id":"c_old"},"match":"","schedule":{"cron":"@every 1h","at":"0001-01-01T00:00:00Z","last":"0001-01-01T00:00:00Z"},"action":{"kind":"wake","why":"w"},"createdAt":"2026-11-10T17:00:00+07:00"}');
INSERT INTO firings(subscription_id, key, rev, at, status, doc) VALUES('s_old', 'fact.x', 1, 0, 'pending', '{"subscriptionId":"s_old","key":"fact.x","action":"wake","rev":1,"at":"2026-11-10T17:00:00+07:00","cause":"reading","status":"pending","nextAttempt":"0001-01-01T00:00:00Z"}');
INSERT INTO playbooks(id, doc) VALUES('p_old', '{"id":"p_old","name":"Free","chairBrief":"how it goes well","chair":"e_old","capabilities":["ship"],"seeds":null}');
INSERT INTO chains(id, created_at, doc) VALUES('c_k_okr-checkin', 11, '{"id":"c_k_okr-checkin","status":"done","playbookId":"okr-checkin","ownerWaits":true,"createdAt":"2026-11-10T17:00:00+07:00","rev":1}');
INSERT INTO chains(id, created_at, doc) VALUES('c_k_playbook-author', 12, '{"id":"c_k_playbook-author","status":"done","playbookId":"playbook-author","createdAt":"2026-11-10T17:00:00+07:00","rev":1}');
INSERT INTO chains(id, created_at, doc) VALUES('c_k_seeded-fault', 13, '{"id":"c_k_seeded-fault","status":"done","playbookId":"seeded-fault","createdAt":"2026-11-10T17:00:00+07:00","rev":1}');
INSERT INTO chains(id, created_at, doc) VALUES('c_k_feature', 14, '{"id":"c_k_feature","status":"done","playbookId":"feature","capabilities":["ship","shell"],"gates":"qc","createdAt":"2026-11-10T17:00:00+07:00","rev":1}');
CREATE TABLE IF NOT EXISTS review_threads (id TEXT PRIMARY KEY, chain_id TEXT NOT NULL, n INTEGER NOT NULL, doc TEXT NOT NULL);
INSERT INTO review_threads(id, chain_id, n, doc) VALUES('t_old', 'c_old', 1, '{"id":"t_old","chainId":"c_old","messages":null,"createdAt":"2026-11-10T17:00:00+07:00","updatedAt":"2026-11-10T17:00:00+07:00"}');
INSERT INTO decisions(id, chain_id, created_at, doc) VALUES('d_mr', 'c_old', 0, '{"id":"d_mr","chainId":"c_old","kind":"mr","text":"Open MR !1","createdAt":"2026-11-10T17:00:00+07:00"}');
INSERT INTO decisions(id, chain_id, created_at, doc) VALUES('d_ship', 'c_old', 1, '{"id":"d_ship","chainId":"c_old","kind":"ship","text":"Merged !2","createdAt":"2026-11-10T17:00:00+07:00"}');
INSERT INTO drafts(id, home, at, doc) VALUES('dr_comment', 'c_old', 0, '{"id":"dr_comment","home":"c_old","kind":"comment","text":"why?","repo":"r","path":"a.go","head":"abc","updatedAt":"2026-11-10T17:00:00+07:00","rev":1}');
INSERT INTO drafts(id, home, at, doc) VALUES('dr_msg', 'c_old', 1, '{"id":"dr_msg","home":"c_old","kind":"message","text":"go on","cardId":"k_old","updatedAt":"2026-11-10T17:00:00+07:00","rev":1}');
INSERT INTO subscriptions(id, owner_kind, owner_id, match, seen_seq, doc) VALUES('s_mr', 'chain', 'c_fin', 'chain:c_old.mr.*', 0, '{"id":"s_mr","owner":{"kind":"chain","id":"c_fin"},"match":"chain:c_old.mr.*","action":{"kind":"wake","why":"w"},"createdAt":"2026-11-10T17:00:00+07:00"}');
INSERT INTO session_inits(card_id, home, doc) VALUES('k_old', 'c_old', '{"cardId":"k_old","home":"c_old","sessionId":"s_old","model":"m","tools":null,"mcpServers":null,"skills":null,"plugins":null,"pluginErrors":null,"agents":null,"slashCommands":null,"at":"2026-11-10T17:00:00+07:00"}');
INSERT INTO session_contexts(card_id, home, doc) VALUES('k_old', 'c_old', '{"cardId":"k_old","home":"c_old","sessionId":"s_old","total":12,"parts":null,"memoryFiles":null,"skills":null,"agents":null,"mcpTools":null,"toolCalls":null,"at":"2026-11-10T17:00:00+07:00"}');
INSERT INTO changes(id, kind, name, at, doc) VALUES('ch_old', 'agent', 'worker', 0, '{"id":"ch_old","kind":"agent","name":"worker","fields":null,"at":"2026-11-10T17:00:00+07:00"}');
INSERT INTO client_keys(id, name, sealed, doc) VALUES('ck_old', 'ck', x'00', '{"id":"ck_old","name":"ck","playbooks":null,"lastUsedAt":"0001-01-01T00:00:00Z","createdAt":"2026-11-10T17:00:00+07:00","updatedAt":"2026-11-10T17:00:00+07:00"}');
INSERT INTO worker_keys(id, mentor, sealed, doc) VALUES('wk_old', 'm1', x'00', '{"id":"wk_old","mentor":"m1","lastUsedAt":"0001-01-01T00:00:00Z","createdAt":"2026-11-10T17:00:00+07:00"}');
INSERT INTO skills(name, doc) VALUES('sk_old', '{"name":"sk_old","files":null}');
INSERT INTO retrospectives(id, chain_id, created_at, doc) VALUES('r_old', 'c_old', 0, '{"id":"r_old","chainId":"c_old","issues":null,"memories":null,"createdAt":"2026-11-10T17:00:00+07:00"}');
INSERT INTO workflows(id, home, created_at, doc) VALUES('w_old', 'c_old', 0, '{"id":"w_old","home":"c_old","card":"k_old","engine":"remit","name":"n","script":"s","status":"done","steps":null,"createdAt":"2026-11-10T17:00:00+07:00","endedAt":"0001-01-01T00:00:00Z","rev":1}');
INSERT INTO workflow_steps(workflow_id, seq, home, doc) VALUES('w_old', 1, 'c_old', '{"workflow":"w_old","seq":1,"verb":"agent","hash":"h","status":"done","at":"2026-11-10T17:00:00+07:00","endedAt":"0001-01-01T00:00:00Z"}');
INSERT INTO helps(id, chain, card, mentor, created_at, doc) VALUES('h_old', 'c_old', 'k_old', 'm1', 0, '{"id":"h_old","chain":"c_old","card":"k_old","mentor":"m1","state":"offered","offeredAt":"2026-11-10T17:00:00+07:00","deadline":"2026-11-10T18:00:00+07:00","answeredAt":"0001-01-01T00:00:00Z","endedAt":"0001-01-01T00:00:00Z","rev":1}');
INSERT INTO events(home, card_id, kind, status, text, created_at, updated_at, by_kind) VALUES('c_old', 'k_old', 'prompt', 'delivered', 'hello', 0, 0, '');
"#,
    )
    .unwrap();
    let s = Store::open(&path).unwrap();
    let (chain, card, approvals, item, agent, objective, sub, events) = s
        .read(|r| {
            Ok::<_, Error>((
                r.chain("c_old")?,
                r.card("k_old")?,
                r.approvals("c_old")?,
                r.inbox_item("i_old")?,
                r.agent("ag_old")?,
                r.objective("o_old")?,
                r.subscription("s_old")?,
                r.card_events("k_old", 10)?,
            ))
        })
        .await
        .unwrap();
    assert_eq!(chain.title, "Fix <the> login & signup", "Go's escapes read as the text");
    assert_eq!(
        chain.created_at.0,
        time::macros::datetime!(2026-11-10 10:00:00.12 UTC),
        "a local offset reads as the same instant"
    );
    // A chain's merge requests are its landings (ADR-0114): the merged one landed, the open one (which has no time of its
    // own, so it was made when its chain was) the version waiting for its gates, the closed one dropped, each numbered in
    // the chain's versions on its repository, oldest first; the repository keeps where it works.
    let landings = s.read(|r| r.chain_landings("c_old")).await.unwrap();
    let states: Vec<(model::LandingState, i64)> =
        landings.iter().map(|l| (l.state.clone(), l.version.number)).collect();
    assert_eq!(
        states,
        [(model::LandingState::Landed, 1), (model::LandingState::Judging, 2)]
    );
    assert_eq!(
        (landings[1].priority, &landings[1].checks, landings[1].landed_at),
        (None, &None, None),
        "an enum key holding the empty text reads as none"
    );
    assert_eq!(
        landings[0].landed_at.map(|t| t.0),
        Some(time::macros::datetime!(2026-11-09 12:00 UTC)),
        "a merged MR landed when it was merged"
    );
    assert_eq!(chain.repos[0].branch, "b", "the repository keeps where the chain works");
    assert_eq!(
        chain.origin,
        model::ChainOrigin::Owner,
        "a chain with no origin is the owner's"
    );
    assert_eq!(chain.unjudged, None);
    assert_eq!(sub.flow.policy, model::FlowPolicy::Each);
    assert_eq!((sub.until, sub.due_at, sub.last_fired), (None, None, None));
    assert_eq!(item.change_kind, None);
    assert!(item.options.is_empty(), "a null list reads empty");
    assert!(item.change.unwrap().fields.is_empty());
    assert!(item.replay.unwrap().briefs.is_empty());
    assert_eq!(agent.title, None);
    assert!(agent.loadout.skills.is_empty());

    // An agent an earlier build wrote with its loadout's keys among its own (ADR-0107 step 4d), and the agent a change
    // item would write, read with them under its loadout.
    let (flat, none, asked) = s
        .read(|r| Ok::<_, Error>((r.agent("ag_flat")?, r.agent("ag_none")?, r.inbox_item("i_agent")?)))
        .await
        .unwrap();
    assert_eq!(
        flat.loadout,
        model::Loadout {
            instructions: "Work <carefully>".into(),
            tools: Some(vec!["Read".into(), "Bash".into()]),
            skills: vec!["rust".into()],
        }
    );
    assert_eq!((flat.name.as_str(), flat.rev), ("builder", 2));
    assert_eq!(
        none.loadout.tools,
        Some(vec![]),
        "an agent stored with no tools holds none"
    );
    assert_eq!(
        agent.loadout.tools, None,
        "an agent stored with null tools holds every tool"
    );
    let record: model::Agent = serde_json::from_str(&asked.change.unwrap().record).unwrap();
    assert_eq!(
        record.loadout,
        model::Loadout {
            instructions: "Build".into(),
            tools: Some(vec!["Read".into()]),
            skills: vec!["go".into()],
        }
    );
    let checks: Vec<(&str, Option<model::CheckKind>)> = objective
        .key_results
        .iter()
        .map(|kr| (kr.id.as_str(), kr.check.as_ref().map(|c| c.kind)))
        .collect();
    assert_eq!(
        checks,
        [("kr1", None), ("kr2", None), ("kr3", Some(model::CheckKind::Submit))],
        "a check with no kind, or an empty one, is no check; one that names its kind is kept"
    );
    // ADR-0115: the key results left the objective's document for rows of their own, with no parent, in their order
    assert!(objective.key_results.iter().all(|kr| kr.parent.is_none()));
    assert_eq!(
        peek(
            &path,
            "SELECT count(*) FROM objectives WHERE json_type(doc, '$.keyResults') IS NOT NULL"
        )
        .unwrap(),
        0,
        "no objective's document holds its key results"
    );
    assert_eq!(
        peek(&path, "SELECT count(*) FROM key_results WHERE objective='o_old'").unwrap(),
        3
    );
    assert_eq!(objective.surrogates[0].expires, None);
    assert_eq!((objective.horizon.text.as_str(), objective.horizon.until), ("", None));
    assert_eq!(
        objective.project, None,
        "an objective stored before projects belongs to none"
    );
    assert_eq!(approvals[0].findings[0].blocker, None);
    assert_eq!(approvals[0].findings[0].item, "x");
    assert_eq!(card.home, "c_old");
    assert!(card.plan.is_empty());
    assert!(card.loadout.unwrap().skills.is_empty());
    assert_eq!(sub.id, "s_old");
    assert_eq!(events.len(), 1);
    assert!(events[0].by.is_none(), "an event with no actor reads with none");

    let (cron, firing, employee, none, playbook, decisions, drafts, init, context) = s
        .read(|r| {
            Ok::<_, Error>((
                r.subscription("s_cron")?,
                r.firing(&model::FiringKey {
                    subscription: "s_old".into(),
                    key: model::SignalKey("fact.x".into()),
                    rev: 1,
                })?,
                r.employee("e_old")?,
                r.objective("o_none")?,
                r.playbook("p_old")?,
                r.decisions("c_old")?,
                r.drafts("c_old")?,
                r.session_init("k_old")?,
                r.session_context("k_old")?,
            ))
        })
        .await
        .unwrap();
    let schedule = cron.schedule.unwrap();
    assert_eq!((schedule.at, schedule.last), (None, None));
    assert_eq!(firing.next_attempt, None);
    assert!(employee.loadout.skills.is_empty());
    assert!(none.key_results.is_empty());
    assert!(playbook.seeds.is_empty());
    assert_eq!(
        playbook.brief, "how it goes well",
        "a playbook's Chair brief is its brief"
    );
    let kinds: Vec<&model::DecisionKind> = decisions.iter().map(|d| &d.kind).collect();
    assert_eq!(
        kinds,
        [&model::DecisionKind::Landing, &model::DecisionKind::Landing],
        "a submit and a merged stage are a landing's decisions"
    );
    let unsent: Vec<&str> = drafts.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(
        unsent,
        ["dr_msg"],
        "a comment on an MR's diff goes with the MR; a message stays"
    );
    let heard = Time(time::macros::datetime!(2026-11-10 10:00 UTC));
    assert_eq!((init.at, context.at), (heard, heard));
    let (init, context) = (init.init, context.context);
    assert_eq!((init.session_id.as_str(), init.model.as_str()), ("s_old", "m"));
    assert_eq!((context.session_id.as_str(), context.total), ("s_old", 12));
    assert!(init.tools.is_empty() && init.mcp_servers.is_empty() && init.plugin_errors.is_empty());
    assert!(
        init.slash_commands.is_empty() && init.skills.is_empty() && init.plugins.is_empty() && init.agents.is_empty()
    );
    assert!(context.parts.is_empty() && context.memory_files.is_empty() && context.skills.is_empty());
    assert!(context.agents.is_empty() && context.mcp_tools.is_empty() && context.tool_calls.is_empty());

    let (change, client, worker, skill, retros, run, help) = s
        .read(|r| {
            Ok::<_, Error>((
                r.change("ch_old")?,
                r.client_key("ck_old")?,
                r.worker_key("wk_old")?,
                r.skill("sk_old")?,
                r.retrospectives_of("c_old")?,
                r.workflow("w_old")?,
                r.help("h_old")?,
            ))
        })
        .await
        .unwrap();
    assert!(change.fields.is_empty());
    assert_eq!(
        client.chains_per_day, 0,
        "a key whose playbooks let it start nothing starts none"
    );
    assert_eq!(client.last_used_at, None);
    assert_eq!(worker.last_used_at, None);
    assert!(skill.files.is_empty());
    assert!(retros[0].issues.is_empty() && retros[0].memories.is_empty());
    assert_eq!(run.ended_at, None);
    assert_eq!((run.steps.len(), run.steps[0].ended_at), (1, None));
    assert_eq!((help.answered_at, help.ended_at), (None, None));

    // a chain takes the kind its playbook stood for, and keeps the capabilities it held (ADR-0113)
    for (id, kind) in [
        ("okr-checkin", model::ChainKind::Objective),
        ("playbook-author", model::ChainKind::Author),
        ("seeded-fault", model::ChainKind::Trial),
        ("feature", model::ChainKind::Work),
    ] {
        let key = format!("c_k_{id}");
        let ch = s.read(move |r| r.chain(&key)).await.unwrap();
        assert_eq!(ch.kind, kind, "{id}");
    }
    let feature = s.read(|r| r.chain("c_k_feature")).await.unwrap();
    assert_eq!(feature.capabilities, ["shell"], "ship goes: no agent merges");

    let (finishing, judging, gated) = s
        .read(|r| Ok::<_, Error>((r.chain("c_fin")?, r.chain("c_judging")?, r.approvals("c_fin")?)))
        .await
        .unwrap();
    let finishing_of = |c: &model::Chain| c.finishing.clone().unwrap();
    assert_eq!(
        (
            finishing_of(&finishing).approval_id.as_deref(),
            finishing_of(&finishing).judge.as_str()
        ),
        (Some("a_fin"), "k_judge"),
        "the approval a finish waits on keeps its id"
    );
    assert_eq!(
        finishing_of(&judging).approval_id,
        None,
        "a finish whose judge has given no verdict waits on none"
    );
    let fin = s.read(|r| r.chain_landings("c_fin")).await.unwrap();
    assert_eq!(
        (
            fin[0].state.clone(),
            fin[0].version.change.as_str(),
            fin[0].version.gates.as_slice()
        ),
        (
            model::LandingState::Judging,
            "p1",
            [model::Gate::Reviewer, model::Gate::Qc, model::Gate::Security].as_slice()
        ),
        "an open MR is the version its gates judge, with the gates it needs"
    );
    let mut carried: Vec<(String, i64)> = s
        .read(|r| r.carried_mrs())
        .await
        .unwrap()
        .into_iter()
        .map(|m| (m.url, m.iid))
        .collect();
    carried.sort();
    assert_eq!(
        carried,
        [("u1".to_owned(), 1), ("u3".to_owned(), 3)],
        "each open draft MR is carried over to be closed once on its forge"
    );
    let watch = s.read(|r| r.subscription("s_mr")).await.unwrap();
    assert_eq!(
        watch.r#match.0, "chain:c_old.landing.*",
        "a watch on a chain's MRs watches its landings"
    );
    let signed: Vec<(&str, model::VerdictRole, model::Verdict)> =
        gated.iter().map(|a| (a.id.as_str(), a.role, a.verdict)).collect();
    assert_eq!(
        signed,
        [
            ("a_rev", model::VerdictRole::Reviewer, model::Verdict::Approve),
            ("a_qc", model::VerdictRole::Qc, model::Verdict::RequestChanges),
            ("a_sec", model::VerdictRole::Security, model::Verdict::Approve),
            ("a_fin", model::VerdictRole::Judge, model::Verdict::RequestChanges),
        ]
    );
    let cards = s.read(|r| r.cards("c_old")).await.unwrap();
    let roles: Vec<(&str, model::CardRole)> = cards.iter().map(|c| (c.id.as_str(), model::CardRole::of(c))).collect();
    assert_eq!(
        roles,
        [
            ("k_old", model::CardRole::Worker),
            ("k_goal", model::CardRole::Judge(model::JudgeKind::Goal)),
            ("k_duel", model::CardRole::Judge(model::JudgeKind::Duel)),
            ("k_second", model::CardRole::Judge(model::JudgeKind::Second)),
            ("k_hired_judge", model::CardRole::Worker),
        ],
        "a judge Remit started reads as its kind, the intake and the check judge are gone with the check judge's verdict, and a card the Chair hired from the judge agent is a worker"
    );
    let asks = s.read(|r| r.cards("c_asks")).await.unwrap();
    let waits: Vec<(&str, Option<&model::PermissionWait>)> =
        asks.iter().map(|c| (c.id.as_str(), c.permission_wait())).collect();
    assert_eq!(
        waits,
        [
            ("k_asks_chair", Some(&model::PermissionWait::Chair)),
            ("k_asks_owner", Some(&model::PermissionWait::Owner)),
            ("k_asked_once", None),
        ],
        "a card left asking waits on whom its activity named; a card no longer asking waits on none"
    );
    assert_eq!(s.unreadable(), 0, "every document reads");
    s.close().unwrap();
    assert_eq!(
        peek(
            &path,
            "SELECT count(*) FROM chains WHERE json_type(doc, '$.finishing.verdict') IS NOT NULL"
        )
        .unwrap(),
        0,
        "no finish keeps the old key"
    );
    assert_eq!(
        peek(
            &path,
            "SELECT count(*) FROM cards WHERE json_extract(doc, '$.origin') = 'judge' AND json_type(doc, '$.judge') IS NULL"
        )
        .unwrap(),
        0,
        "every judge names its kind"
    );
    assert_eq!(
        peek(
            &path,
            "SELECT count(*) FROM sqlite_master WHERE name = 'review_threads'"
        )
        .unwrap(),
        0,
        "the review threads go with the MR"
    );
    assert_eq!(
        peek(
            &path,
            "SELECT count(*) FROM chains, json_each(chains.doc, '$.repos') r WHERE json_type(r.value, '$.mr') IS NOT NULL OR json_type(r.value, '$.earlierMrs') IS NOT NULL"
        )
        .unwrap(),
        0,
        "no repository keeps a merge request"
    );
}

// Remit lands changes itself (ADR-0114, ADR-0116): the merge requests a chain's repository work holds read as its
// landings after migration 0119, and a grant a chain held reads the same, held by the chain, after chain_grants became
// grants. Run again, the migration carries over nothing twice.
#[tokio::test(flavor = "multi_thread")]
async fn stored_merge_requests_read_as_landings_and_a_grant_reads_the_same_after_migrating() {
    let dir = tempfile::tempdir().unwrap();
    let path = db(&dir);
    as_an_earlier_build_left_it(
        &path,
        118,
        r#"
CREATE TABLE chain_grants (chain_id TEXT NOT NULL, source TEXT NOT NULL, connection TEXT NOT NULL, effect TEXT NOT NULL CHECK (effect IN ('read', 'reversible', 'compensable', 'irreversible')), granted_by TEXT NOT NULL, granted_at INTEGER NOT NULL, PRIMARY KEY(chain_id, source, connection));
INSERT INTO chain_grants VALUES('c_mr', 'mongo', 'prod', 'irreversible', '{"kind":"owner","who":"o","via":"web"}', 1794100000000);
INSERT INTO chains(id, created_at, doc) VALUES('c_mr', 0, '{"id":"c_mr","status":"done","kind":"work","repos":[
 {"repo":"r","base":"main","pushedSha":"s3","pushedFrom":"f3","mr":{"iid":3,"url":"u3","state":"opened","head":"s3","change":"p3","gates":["reviewer","qc"],"by":["e1"],"priority":"high","priorityWhy":"w","updatedAt":"2026-11-03T10:00:00+07:00","version":1,"checks":{"state":"passed"}},
  "earlierMrs":[{"iid":1,"url":"u1","state":"merged","head":"h1","updatedAt":"2026-11-01T10:00:00Z","decidedAt":"2026-11-01T12:00:00Z"},{"iid":2,"url":"u2","state":"closed","head":"h2","updatedAt":"2026-11-02T10:00:00Z"}]},
 {"repo":"s","base":"master","pushedFrom":"f4","mr":{"iid":4,"url":"u4","state":"opened","head":"h4","change":"p4","updatedAt":"2026-11-04T10:00:00Z"}}],
 "createdAt":"2026-11-01T00:00:00Z","rev":1}');
INSERT INTO approvals(id, chain_id, at, doc) VALUES('a_rev', 'c_mr', 1, '{"id":"a_rev","chainId":"c_mr","repo":"r","change":"p3","role":"reviewer","verdict":"approve","at":"2026-11-03T04:00:00Z"}');
INSERT INTO approvals(id, chain_id, at, doc) VALUES('a_qc1', 'c_mr', 2, '{"id":"a_qc1","chainId":"c_mr","repo":"r","change":"p3","role":"qc","verdict":"request_changes","at":"2026-11-03T05:00:00Z"}');
INSERT INTO approvals(id, chain_id, at, doc) VALUES('a_qc2', 'c_mr', 3, '{"id":"a_qc2","chainId":"c_mr","repo":"r","change":"p3","role":"qc","verdict":"approve","at":"2026-11-03T06:00:00Z"}');
INSERT INTO approvals(id, chain_id, at, doc) VALUES('a_rev4', 'c_mr', 4, '{"id":"a_rev4","chainId":"c_mr","repo":"s","change":"p4","role":"reviewer","verdict":"approve","at":"2026-11-04T11:00:00Z"}');
INSERT INTO approvals(id, chain_id, at, doc) VALUES('a_qc4', 'c_mr', 5, '{"id":"a_qc4","chainId":"c_mr","repo":"s","change":"p0","role":"qc","verdict":"approve","at":"2026-11-04T12:00:00Z"}');
"#,
    )
    .unwrap();
    let s = Store::open(&path).unwrap();
    let (landings, grants) = s
        .read(|r| Ok::<_, Error>((r.chain_landings("c_mr")?, r.grants(&model::GrantHolder::chain("c_mr"))?)))
        .await
        .unwrap();
    let at = |t: time::OffsetDateTime| Some(Time(t));
    let read: Vec<_> = landings
        .iter()
        .map(|l| {
            (
                l.repo.as_str(),
                l.base.as_str(),
                l.state.clone(),
                l.reference.as_deref(),
                l.commit.as_deref(),
                l.version.from.as_str(),
                l.version.verdicts.clone(),
                l.ready_at,
                l.landed_at,
            )
        })
        .collect();
    assert_eq!(
        read,
        [
            (
                "r",
                "main",
                model::LandingState::Landed,
                Some("u1"),
                Some("h1"),
                "h1",
                vec![],
                None,
                at(time::macros::datetime!(2026-11-01 12:00 UTC))
            ),
            (
                "r",
                "main",
                model::LandingState::Ready,
                Some("u3"),
                None,
                "f3",
                vec!["a_rev".to_owned(), "a_qc2".to_owned()],
                at(time::macros::datetime!(2026-11-03 06:00 UTC)),
                None
            ),
            (
                "s",
                "master",
                model::LandingState::Judging,
                Some("u4"),
                None,
                "f4",
                vec![],
                None,
                None
            ),
        ],
        "a merged MR landed, an open one its gates approve is ready when the last verdict came, one whose QC approved an \
         earlier version is judged, and a closed one is none"
    );
    let ready = &landings[1];
    assert_eq!(
        (
            ready.version.change.as_str(),
            ready.version.gates.as_slice(),
            ready.version.by.as_slice(),
            ready.priority,
            ready.priority_why.as_str()
        ),
        (
            "p3",
            [model::Gate::Reviewer, model::Gate::Qc].as_slice(),
            ["e1".to_owned()].as_slice(),
            Some(model::Priority::High),
            "w"
        )
    );
    assert_eq!(
        landings[2].version.gates,
        [model::Gate::Reviewer, model::Gate::Qc],
        "a version pushed before its gates were recorded needs review and QC"
    );
    assert!(
        landings
            .iter()
            .all(|l| l.id.starts_with("ln_") && l.chain_id == "c_mr" && l.rev == 1)
    );
    assert_eq!(
        grants,
        [model::Grant {
            holder: model::GrantHolder::chain("c_mr"),
            source: "mongo".into(),
            connection: "prod".into(),
            effect: "irreversible".into(),
            granted_by: model::by_owner("o", "web"),
            granted_at: Time(time::macros::datetime!(2026-11-08 01:06:40 UTC)),
        }]
    );
    assert_eq!(s.unreadable(), 0, "every landing reads");
    s.close().unwrap();

    Connection::open(&path)
        .unwrap()
        .execute_batch(include_str!("../migrations/0119.sql"))
        .unwrap();
    assert_eq!(peek(&path, "SELECT count(*) FROM landings").unwrap(), 3);
    assert_eq!(peek(&path, "SELECT count(*) FROM grants").unwrap(), 1);
    assert_eq!(
        peek(&path, "SELECT count(*) FROM sqlite_master WHERE name = 'chain_grants'").unwrap(),
        0
    );
}

// ADR-0115: the owner's key results are rows written with the objective. Writing it again drops what it leaves out,
// keeps the order it gives, and keeps a key result's lift, which is rail state and no part of the commitment; a key
// result that refines another is written on its own and outlives the objective written again.
#[tokio::test(flavor = "multi_thread")]
async fn writing_an_objective_again_drops_what_it_leaves_out_and_keeps_lifts_and_refinements() {
    let dir = tempfile::tempdir().unwrap();
    let s = Store::open(db(&dir)).unwrap();
    let kr = |id: &str| model::KeyResult {
        id: id.to_owned(),
        text: id.to_owned(),
        ..model::KeyResult::default()
    };
    let draft = model::Objective {
        id: "ob_1".to_owned(),
        key_results: vec![kr("a"), kr("b"), kr("c")],
        ..model::Objective::default()
    };
    let lift = model::Lift {
        why: "a new approach".to_owned(),
        ..model::Lift::default()
    };
    let lifted = model::KeyResult {
        lifted: Some(lift.clone()),
        ..kr("c")
    };
    let child = model::KeyResult {
        parent: Some("c".to_owned()),
        ..kr("c1")
    };
    let again = vec![kr("c"), kr("a")];
    let stored = s
        .update(move |tx| {
            let first = tx.put_objective(draft)?;
            tx.put_key_result("ob_1", lifted)?;
            tx.put_key_result("ob_1", child)?;
            tx.put_objective(model::Objective {
                key_results: again,
                ..first
            })
        })
        .await
        .unwrap();
    let ids = |krs: &[model::KeyResult]| krs.iter().map(|k| k.id.clone()).collect::<Vec<_>>();
    assert_eq!(
        ids(&stored.key_results),
        ["c", "a"],
        "b is dropped, and the owner's order holds"
    );
    let (objective, tree) = s
        .read(|r| Ok::<_, Error>((r.objective("ob_1")?, r.key_results("ob_1")?)))
        .await
        .unwrap();
    assert_eq!(ids(&objective.key_results), ["c", "a"]);
    assert_eq!(
        ids(&tree),
        ["c", "a", "c1"],
        "what refines a key result follows the owner's"
    );
    assert_eq!(
        tree[0].lifted,
        Some(lift),
        "the lift outlives the commitment written again"
    );
    assert_eq!(tree[2].parent.as_deref(), Some("c"));
    s.close().unwrap();
}

// Memories as migration 120 wrote them read after migration 122: a memory's use leaves its document for memory_use and
// reads back with it, its words are found again, a teaching is found by its holder and by its card. Writing the memory
// again keeps its use, a count lands on active memories only, and a vector made of a passage the memory no longer has is
// not stored.
#[tokio::test(flavor = "multi_thread")]
async fn memories_an_earlier_build_wrote_keep_their_use_and_words_after_migrating() {
    let dir = tempfile::tempdir().unwrap();
    let path = db(&dir);
    as_an_earlier_build_left_it(
        &path,
        121,
        r#"
INSERT INTO memories(id, holder, template, project, scope, status, created_at, updated_at, doc) VALUES('mem_old', 'e1', 't1', '', 'own', 'active', 0, 0, '{"id":"mem_old","holder":"e1","template":"t1","project":"","kind":"pitfall","scope":"own","trust":"auto","status":"active","title":"record_verdict requires a clean git worktree","body":"Commit first.","evidence":"e","source":"card:k1 accepted","teacher":"","about":["tool:record_verdict"],"links":[],"supersededBy":"","retaught":[],"recalled":3,"lastRecalledAt":"2026-11-19T18:01:43.42467+07:00","helped":1,"misled":0,"createdAt":"2026-11-19T17:00:00+07:00","updatedAt":"2026-11-19T17:00:00+07:00","endedAt":null,"endedWhy":""}');
INSERT INTO memories(id, holder, template, project, scope, status, created_at, updated_at, doc) VALUES('mem_gone', 'e1', 't1', '', 'own', 'superseded', 1, 1, '{"id":"mem_gone","holder":"e1","kind":"lesson","scope":"own","status":"superseded","title":"old","body":"b","supersededBy":"mem_old","createdAt":"2026-11-19T17:00:00+07:00","updatedAt":"2026-11-19T17:00:00+07:00","endedAt":"2026-11-19T17:30:00+07:00","endedWhy":"superseded by mem_old"}');
INSERT INTO teachings(id, state, created_at, doc) VALUES('tch_1', 'settled', 0, '{"id":"tch_1","holder":"e1","card":"k1","role":"accepted","state":"settled","written":["mem_old"],"createdAt":"2026-11-19T17:00:00+07:00"}');
INSERT INTO cards(id, home, created_at, doc) VALUES('k1', 'ch1', 0, '{"id":"k1","home":"ch1","name":"Engineer · sword slash"}');
INSERT INTO chains(id, created_at, doc) VALUES('ch1', 0, '{"id":"ch1","title":"Implement sword slash"}');
"#,
    )
    .unwrap();
    let s = Store::open(&path).unwrap();
    s.close().unwrap();
    assert_eq!(
        peek(
            &path,
            "SELECT count(*) FROM memories WHERE json_type(doc, '$.recalled') IS NOT NULL"
        )
        .unwrap(),
        0,
        "the use left the document"
    );
    let s = Store::open(&path).unwrap();
    let reader = store::MemoryReader {
        holder: "e1".to_owned(),
        ..store::MemoryReader::default()
    };
    let (m, gone, found, of_holder, of_card) = s
        .read(move |r| {
            Ok::<_, Error>((
                r.memory("mem_old")?,
                r.memory("mem_gone")?,
                r.memories_matching(&reader, "how do I record the verdict", 10)?,
                r.teachings_of("e1", 10)?,
                r.teachings_of_card("k1")?,
            ))
        })
        .await
        .unwrap();
    assert_eq!((m.recalled, m.helped, m.misled), (3, 1, 0));
    assert_eq!(
        m.last_recalled_at.map(|t| t.0),
        Some(time::macros::datetime!(2026-11-19 11:01:43.425 UTC))
    );
    assert_eq!(gone.end.superseded_by(), "mem_old");
    assert_eq!(
        (
            m.source_label.as_str(),
            m.chain.as_str(),
            m.card.as_str(),
            m.work.as_str()
        ),
        (
            "an accepted card: Engineer · sword slash",
            "ch1",
            "k1",
            "Implement sword slash"
        ),
        "where it was learned is read from its card"
    );
    assert_eq!(found.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), ["mem_old"]);
    assert_eq!((of_holder.len(), of_card.len()), (1, 1));

    let hash = store::passage_hash(&m);
    let stored = s
        .update(move |tx| {
            tx.count_recall(&["mem_old".to_owned(), "mem_gone".to_owned()], Time::default())?;
            tx.count_use("mem_old", model::MemoryUse::Misled)?;
            tx.put_memory(&model::Memory {
                recalled: 0,
                body: "Commit or stash first.".to_owned(),
                links: vec![model::MemoryLink {
                    kind: model::MemoryLinkKind::Supersedes,
                    to: "mem_gone".to_owned(),
                }],
                ..m
            })?;
            tx.put_memory_vector("mem_old", "model", &hash, &[1.0, 0.0])
        })
        .await
        .unwrap();
    assert!(!stored, "a vector of the passage before the change is not stored");
    let (m, gone) = s
        .read(|r| Ok::<_, Error>((r.memory("mem_old")?, r.memory("mem_gone")?)))
        .await
        .unwrap();
    assert_eq!(
        (m.recalled, m.misled, m.body.as_str()),
        (4, 1, "Commit or stash first.")
    );
    assert_eq!(gone.recalled, 0, "an ended memory is not counted");

    // A forget takes the memory, its words and its use, and what the memory it superseded said of it.
    let (gone, left, words) = s
        .update(|tx| {
            tx.delete_memories(&["mem_old".to_owned()], Time::default())?;
            let reader = store::MemoryReader {
                holder: "e1".to_owned(),
                ..store::MemoryReader::default()
            };
            Ok::<_, Error>((
                tx.memory("mem_gone")?,
                tx.memories_named(&["mem_old".to_owned()])?,
                tx.memories_matching(&reader, "record_verdict", 10)?,
            ))
        })
        .await
        .unwrap();
    assert!(left.is_empty() && words.is_empty());
    assert_eq!(gone.end.superseded_by(), "");
    assert!(gone.end.why().contains("forgotten since"));
    assert_eq!(
        peek(&path, "SELECT count(*) FROM memory_use WHERE memory_id = 'mem_old'").unwrap(),
        0
    );
    s.close().unwrap();
}

/// A landing with every field the released record holds set, in state, released by release.
fn full_landing(id: &str, state: model::LandingState, release: Option<&str>) -> model::Landing {
    let at = |m: i64| Time(time::macros::datetime!(2026-11-10 10:00 UTC).saturating_add(time::Duration::minutes(m)));
    model::Landing {
        id: id.into(),
        chain_id: "c_full".into(),
        repo: "app".into(),
        host: "github.com".into(),
        project: "acme/app".into(),
        base: "main".into(),
        version: model::LandingVersion {
            number: 2,
            from: "abc".into(),
            forked: "def".into(),
            change: "p1".into(),
            gates: vec![model::Gate::Reviewer, model::Gate::Qc],
            verdicts: vec!["ap_1".into(), "ap_2".into()],
            by: vec!["e_1".into()],
            title: "fix(login): sign in again".into(),
            description: "the session cookie is <Secure> again".into(),
            paths: vec!["auth/session.go".into()],
            judged: vec!["ju_1".into()],
            effort: Some("max".into()),
            plan: None,
        },
        priority: Some(model::Priority::High),
        priority_why: "nobody can sign in".into(),
        state,
        batch: Some("lb_1".into()),
        onto: Some("def".into()),
        commit: Some("0a1".into()),
        push: Some("0a2".into()),
        release: release.map(str::to_owned),
        reference: Some("https://github.com/acme/app/pull/7".into()),
        created_at: at(0),
        ready_at: Some(at(1)),
        landed_at: Some(at(2)),
        updated_at: at(3),
        ..model::Landing::default()
    }
}

/// Every row of landings with every column, released and generated, as text: what a rebuild of the table must keep.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper outside a #[test] fn, where a failed unwrap is the failed assertion"
)]
fn landings_rows(path: &Path) -> Vec<String> {
    let c = Connection::open(path).unwrap();
    let mut st = c
        .prepare(
            "SELECT id, chain_id, repo, created_at, doc, release, host, project, base, state FROM landings ORDER BY id",
        )
        .unwrap();
    st.query_map([], |r| {
        let cols: Vec<String> = (0..10)
            .map(|i| r.get::<_, rusqlite::types::Value>(i).map(|v| format!("{v:?}")))
            .collect::<rusqlite::Result<_>>()?;
        Ok(cols.join("|"))
    })
    .unwrap()
    .collect::<rusqlite::Result<_>>()
    .unwrap()
}

// Migration 0123 makes landings again with the queue's columns: run over a table that holds landings (once by the open
// after a rewind, once more by hand) it keeps every row, every released column and every document as it was, and the
// landings read back field for field.
#[tokio::test(flavor = "multi_thread")]
async fn the_landings_rebuild_keeps_every_row_and_runs_twice_to_the_same_state() {
    let dir = tempfile::tempdir().unwrap();
    let path = db(&dir);
    let s = Store::open(&path).unwrap();
    let put = vec![
        full_landing("ln_landed", model::LandingState::Landed, Some("rl_1")),
        full_landing("ln_open", model::LandingState::Ready, None),
    ];
    let stored: Vec<model::Landing> = s
        .update(move |tx| {
            put.into_iter()
                .map(|l| tx.put_landing(l))
                .collect::<store::Result<Vec<_>>>()
        })
        .await
        .unwrap();
    s.close().unwrap();
    let before = landings_rows(&path);
    assert_eq!(before.len(), 2);

    as_an_earlier_build_left_it(&path, 122, "").unwrap();
    let s = Store::open(&path).unwrap();
    let read: Vec<model::Landing> = s
        .read(|r| Ok::<_, Error>(vec![r.landing("ln_landed")?, r.landing("ln_open")?]))
        .await
        .unwrap();
    assert_eq!(read, stored, "each landing reads back field for field");
    let queue = model::QueueKey {
        host: "github.com".into(),
        project: "acme/app".into(),
        base: "main".into(),
    };
    let (in_queue, carried) = s
        .read(move |r| Ok::<_, Error>((r.queue_landings(&queue)?.len(), r.release_landings("rl_1")?.len())))
        .await
        .unwrap();
    assert_eq!((in_queue, carried), (2, 1), "the generated columns read as before");
    s.close().unwrap();
    assert_eq!(landings_rows(&path), before, "the rebuild keeps every row and column");

    Connection::open(&path)
        .unwrap()
        .execute_batch(include_str!("../migrations/0123.sql"))
        .unwrap();
    assert_eq!(landings_rows(&path), before, "run again, 0123 ends in the same state");
    assert_eq!(
        peek(
            &path,
            "SELECT count(*) FROM sqlite_master WHERE tbl_name = 'landings' AND type = 'index' AND sql IS NOT NULL"
        )
        .unwrap(),
        4,
        "its indexes are made again, once each"
    );
}

/// Every row of table with every column, as text, in the order stored: what a migration run again must leave as it was.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper outside a #[test] fn, where a failed unwrap is the failed assertion"
)]
fn table_rows(path: &Path, table: &str) -> Vec<String> {
    let c = Connection::open(path).unwrap();
    let mut st = c.prepare(&format!("SELECT * FROM {table} ORDER BY rowid")).unwrap();
    let n = st.column_count();
    st.query_map([], |r| {
        let cols: Vec<String> = (0..n)
            .map(|i| r.get::<_, rusqlite::types::Value>(i).map(|v| format!("{v:?}")))
            .collect::<rusqlite::Result<_>>()?;
        Ok(cols.join("|"))
    })
    .unwrap()
    .collect::<rusqlite::Result<_>>()
    .unwrap()
}

/// The tables migration 0124 writes, each as table_rows reads it.
fn release_seat_tables(path: &Path) -> Vec<Vec<String>> {
    [
        "chains",
        "subscriptions",
        "firings",
        "near_misses",
        "inbox",
        "sensors",
        "releases",
        "release_pauses",
    ]
    .into_iter()
    .map(|t| table_rows(path, t))
    .collect()
}

// Migration 0124 (ADR-0116) folds the Release seat into each repository's procedure. Over rows a build at 123 left (a
// Release seat at work, with watches it owns and waits it holds, their firings, near misses and held items, and its
// sensor; the routine that started it and another that starts one; a watch on fact:remit.released; a work chain beside
// it holding the capability release; a release) the seat reads finished as a finish in code leaves it, everything that
// would start, wake or feed it ends as a deleted subscription does, the work chain's own are untouched, the release
// reads back field for field and nothing is left unreadable. Run again, it changes nothing.
#[tokio::test(flavor = "multi_thread")]
async fn the_release_seat_is_finished_and_what_feeds_it_ends_and_running_it_again_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = db(&dir);
    as_an_earlier_build_left_it(
        &path,
        123,
        r#"
INSERT INTO chains(id, created_at, doc) VALUES('c_rel', 0, '{"id":"c_rel","title":"Release","kind":"release","origin":"routine","status":"running","capabilities":["history","propose","release","signals"],"summary":"","finishing":{"summary":"installed 20261110-c2","judge":"k_judge","at":"2026-11-10T10:00:00Z","verdict":"","recheck":false},"createdAt":"2026-11-10T09:00:00Z","rev":3}');
INSERT INTO chains(id, created_at, doc) VALUES('c_work', 1, '{"id":"c_work","title":"Fix login","kind":"work","status":"running","capabilities":["ship","release","shell"],"createdAt":"2026-11-10T09:00:00Z","rev":1}');
INSERT INTO subscriptions(id, owner_kind, owner_id, match, seen_seq, doc) VALUES('s_rel_watch', 'remit', 'c_rel', 'fact:chain.landed', 4, '{"id":"s_rel_watch","owner":{"kind":"remit","id":"c_rel"},"match":"fact:chain.landed","action":{"kind":"tell"},"createdAt":"2026-11-10T09:00:00Z"}');
INSERT INTO subscriptions(id, owner_kind, owner_id, match, seen_seq, doc) VALUES('s_rel_wait', 'chain', 'c_rel', 'fact:chain.landed', 4, '{"id":"s_rel_wait","owner":{"kind":"chain","id":"c_rel"},"match":"fact:chain.landed","action":{"kind":"wake","why":"the next landing to release"},"createdAt":"2026-11-10T09:00:00Z"}');
INSERT INTO subscriptions(id, owner_kind, owner_id, match, seen_seq, doc) VALUES('sub_start_release_seat', 'remit', '', '', 0, '{"id":"sub_start_release_seat","owner":{"kind":"remit","id":""},"match":"","schedule":{"cron":"@every 1h0m0s"},"action":{"kind":"start","start":{"title":"Release","goal":"release what landed","kind":"release","origin":"routine"}},"createdAt":"2026-11-10T09:00:00Z"}');
INSERT INTO subscriptions(id, owner_kind, owner_id, match, seen_seq, doc) VALUES('s_routine_release', 'remit', '', 'event:deployed', 0, '{"id":"s_routine_release","owner":{"kind":"remit","id":""},"match":"event:deployed","action":{"kind":"start","start":{"title":"Release after deploy","kind":"release","origin":"routine"}},"createdAt":"2026-11-10T09:00:00Z"}');
INSERT INTO subscriptions(id, owner_kind, owner_id, match, seen_seq, doc) VALUES('s_released', 'chain', 'c_work', 'fact:remit.released', 4, '{"id":"s_released","owner":{"kind":"chain","id":"c_work"},"match":"fact:remit.released","action":{"kind":"wake","why":"the release that carries it"},"createdAt":"2026-11-10T09:00:00Z"}');
INSERT INTO subscriptions(id, owner_kind, owner_id, match, seen_seq, doc) VALUES('s_work_wait', 'chain', 'c_work', 'fact:x', 4, '{"id":"s_work_wait","owner":{"kind":"chain","id":"c_work"},"match":"fact:x","action":{"kind":"wake","why":"x"},"createdAt":"2026-11-10T09:00:00Z"}');
INSERT INTO subscriptions(id, owner_kind, owner_id, match, seen_seq, doc) VALUES('s_work_watch', 'remit', 'c_work', 'fact:y', 4, '{"id":"s_work_watch","owner":{"kind":"remit","id":"c_work"},"match":"fact:y","action":{"kind":"tell"},"createdAt":"2026-11-10T09:00:00Z"}');
INSERT INTO firings(subscription_id, key, rev, at, owner_kind, owner_id, status, doc) VALUES('s_rel_watch', 'fact:chain.landed', 1, 0, 'remit', 'c_rel', 'pending', '{"subscriptionId":"s_rel_watch","owner":{"kind":"remit","id":"c_rel"},"key":"fact:chain.landed","action":"tell","rev":1,"seqs":[3],"at":"2026-11-10T10:00:00Z","cause":"reading","status":"pending"}');
INSERT INTO firings(subscription_id, key, rev, at, owner_kind, owner_id, status, doc) VALUES('s_rel_wait', 'fact:chain.landed', 1, 0, 'chain', 'c_rel', 'failed', '{"subscriptionId":"s_rel_wait","owner":{"kind":"chain","id":"c_rel"},"key":"fact:chain.landed","action":"wake","rev":1,"seqs":[3],"at":"2026-11-10T10:00:00Z","cause":"reading","status":"failed","attempts":1,"lastError":"busy"}');
INSERT INTO firings(subscription_id, key, rev, at, owner_kind, owner_id, status, doc) VALUES('sub_start_release_seat', 'schedule', 1, 0, 'remit', '', 'pending', '{"subscriptionId":"sub_start_release_seat","owner":{"kind":"remit","id":""},"key":"schedule","action":"start","rev":1,"at":"2026-11-10T10:00:00Z","cause":"scheduled","status":"pending","waiting":"one at work"}');
INSERT INTO firings(subscription_id, key, rev, at, owner_kind, owner_id, status, doc) VALUES('s_released', 'fact:remit.released', 1, 0, 'chain', 'c_work', 'pending', '{"subscriptionId":"s_released","owner":{"kind":"chain","id":"c_work"},"key":"fact:remit.released","action":"wake","rev":1,"seqs":[2],"at":"2026-11-10T10:00:00Z","cause":"reading","status":"pending"}');
INSERT INTO firings(subscription_id, key, rev, at, owner_kind, owner_id, status, doc) VALUES('s_work_wait', 'fact:x', 1, 0, 'chain', 'c_work', 'pending', '{"subscriptionId":"s_work_wait","owner":{"kind":"chain","id":"c_work"},"key":"fact:x","action":"wake","rev":1,"seqs":[1],"at":"2026-11-10T10:00:00Z","cause":"reading","status":"pending"}');
INSERT INTO near_misses(subscription_id, seq, reason, at, doc) VALUES('s_rel_watch', 2, 'condition_false', 0, '{"subscriptionId":"s_rel_watch","seq":2,"key":"fact:chain.landed","reason":"condition_false","at":"2026-11-10T10:00:00Z"}');
INSERT INTO near_misses(subscription_id, seq, reason, at, doc) VALUES('s_rel_wait', 2, 'condition_false', 0, '{"subscriptionId":"s_rel_wait","seq":2,"key":"fact:chain.landed","reason":"condition_false","at":"2026-11-10T10:00:00Z"}');
INSERT INTO near_misses(subscription_id, seq, reason, at, doc) VALUES('s_work_wait', 2, 'condition_false', 0, '{"subscriptionId":"s_work_wait","seq":2,"key":"fact:x","reason":"condition_false","at":"2026-11-10T10:00:00Z"}');
INSERT INTO inbox(id, home, status, created_at, doc) VALUES('i_rel', 'c_rel', 'open', 0, '{"id":"i_rel","home":"c_rel","kind":"held","status":"open","subscription":"s_rel_wait","createdAt":"2026-11-10T10:00:00Z","rev":1}');
INSERT INTO inbox(id, home, status, created_at, doc) VALUES('i_seat', '', 'open', 1, '{"id":"i_seat","home":"","kind":"held","status":"open","subscription":"sub_start_release_seat","createdAt":"2026-11-10T10:00:00Z","rev":1}');
INSERT INTO inbox(id, home, status, created_at, doc) VALUES('i_work', 'c_work', 'open', 2, '{"id":"i_work","home":"c_work","kind":"held","status":"open","subscription":"s_work_wait","createdAt":"2026-11-10T10:00:00Z","rev":1}');
INSERT INTO sensors(id, chain, name, signal, doc) VALUES('sn_rel', 'c_rel', 'health', 'chain:c_rel.sensor.health', '{"id":"sn_rel","kind":"script","name":"health","chain":"c_rel","signal":"chain:c_rel.sensor.health","everyMinutes":5,"createdAt":"2026-11-10T09:00:00Z","updatedAt":"2026-11-10T09:00:00Z","rev":1}');
INSERT INTO sensors(id, chain, name, signal, doc) VALUES('sn_work', 'c_work', 'probe', 'chain:c_work.sensor.probe', '{"id":"sn_work","kind":"script","name":"probe","chain":"c_work","signal":"chain:c_work.sensor.probe","everyMinutes":5,"createdAt":"2026-11-10T09:00:00Z","updatedAt":"2026-11-10T09:00:00Z","rev":1}');
INSERT INTO releases(id, repo, created_at, doc) VALUES('rl_1', 'app', 0, '{"id":"rl_1","repo":"app","commit":"c2","previous":"c1","state":{"kind":"live"},"version":"1.2.0","links":["https://app.example"],"createdAt":"2026-11-10T10:00:00Z","bakeUntil":"2026-11-10T11:00:00Z","endedAt":"2026-11-10T11:00:00Z","rev":2}');
"#,
    )
    .unwrap();

    let s = Store::open(&path).unwrap();
    let (seat, work, subs, sensors, releases, pause) = s
        .read(|r| {
            Ok::<_, Error>((
                r.chain("c_rel")?,
                r.chain("c_work")?,
                r.subscriptions()?,
                r.sensors()?,
                r.releases()?,
                r.release_pause("app")?,
            ))
        })
        .await
        .unwrap();
    assert_eq!(
        (*seat.status(), seat.kind, seat.finishing.is_none()),
        (model::ChainStatus::Done, model::ChainKind::Chore, true),
        "the seat is finished as a chore, its finish no longer under way"
    );
    assert_eq!(
        seat.summary,
        "Finished: releases are run by each repository's own .remit/release/, in code (ADR-0116)."
    );
    assert_eq!(seat.capabilities, ["history", "propose", "signals"]);
    assert_eq!(
        (*work.status(), work.capabilities.as_slice()),
        (
            model::ChainStatus::Running,
            ["ship".to_owned(), "shell".to_owned()].as_slice()
        ),
        "a chain beside it only loses the capability release"
    );
    let mut ids: Vec<&str> = subs.iter().map(|s| s.id.as_str()).collect();
    ids.sort_unstable();
    assert_eq!(
        ids,
        ["s_work_wait", "s_work_watch"],
        "the seat's watches and waits, what starts a seat and the watch on fact:remit.released end; the work chain's own \
         stay"
    );
    assert_eq!(sensors.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["sn_work"]);
    assert_eq!(
        releases,
        [model::Release {
            id: "rl_1".into(),
            repo: "app".into(),
            commit: "c2".into(),
            previous: Some("c1".into()),
            state: model::ReleaseState::Live,
            version: "1.2.0".into(),
            links: vec!["https://app.example".into()],
            created_at: Time(time::macros::datetime!(2026-11-10 10:00 UTC)),
            bake_until: Some(Time(time::macros::datetime!(2026-11-10 11:00 UTC))),
            ended_at: Some(Time(time::macros::datetime!(2026-11-10 11:00 UTC))),
            rev: 2,
            ..model::Release::default()
        }],
        "a release stored at 123 reads back field for field"
    );
    assert_eq!(pause, None, "no pause was ever pulled");

    let (firings, misses, items) = s
        .read(|r| {
            let firings = [
                "s_rel_watch",
                "s_rel_wait",
                "sub_start_release_seat",
                "s_released",
                "s_work_wait",
            ]
            .into_iter()
            .map(|id| {
                let f = store::FiringFilter {
                    subscription: id.into(),
                    ..store::FiringFilter::default()
                };
                r.firings(&f, 10)
                    .map(|fs| fs.into_iter().map(|f| f.status).collect::<Vec<_>>())
            })
            .collect::<store::Result<Vec<_>>>()?;
            let misses = ["s_rel_watch", "s_rel_wait", "s_work_wait"]
                .into_iter()
                .map(|id| r.near_misses(id, 10).map(|m| m.len()))
                .collect::<store::Result<Vec<_>>>()?;
            let items = ["i_rel", "i_seat", "i_work"]
                .into_iter()
                .map(|id| r.inbox_item(id).map(|i| (i.status, i.answer)))
                .collect::<store::Result<Vec<_>>>()?;
            Ok::<_, Error>((firings, misses, items))
        })
        .await
        .unwrap();
    let cancelled = vec![model::FiringStatus::Cancelled];
    assert_eq!(
        firings,
        [
            cancelled.clone(),
            cancelled.clone(),
            cancelled.clone(),
            cancelled,
            vec![model::FiringStatus::Pending]
        ],
        "a firing still to do of what ended is cancelled with it"
    );
    assert_eq!(misses, [0, 0, 1], "the near misses of what ended go");
    let gone = (model::InboxStatus::Cancelled, "the subscription is gone".to_owned());
    assert_eq!(
        items,
        [gone.clone(), gone, (model::InboxStatus::Open, String::new())],
        "an item held on what ended is answered by no one now"
    );
    assert_eq!(s.unreadable(), 0, "every row reads in the shape this build knows");
    s.close().unwrap();

    let before = release_seat_tables(&path);
    Connection::open(&path)
        .unwrap()
        .execute_batch(include_str!("../migrations/0124.sql"))
        .unwrap();
    assert_eq!(
        release_seat_tables(&path),
        before,
        "run again, 0124 ends in the same state"
    );
}

fn judgment(id: &str, decider: &str, used: model::JudgmentUse, p: f64) -> model::Judgment {
    model::Judgment {
        id: id.to_owned(),
        asking: "ja_1".to_owned(),
        question: "memory.same_claim@1".to_owned(),
        decider: decider.to_owned(),
        answer: model::JudgmentAnswer::Noul { p },
        pick: model::JudgmentPick::Yes,
        p,
        cost_usd: 0.001,
        used,
        ..model::Judgment::default()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_outcome_scores_every_judgment_of_its_asking_and_a_shadow_stored_after_takes_it() {
    use model::{JudgmentOutcome, JudgmentUse, OutcomeSource, Proved};
    let dir = tempfile::tempdir().unwrap();
    let s = Store::open(db(&dir)).unwrap();
    let acting = judgment("ju_a", "liquid/d1.2-ultra", JudgmentUse::Acted, 0.95);
    let shadow = judgment("ju_s", "typesafe/jev-1.14", JudgmentUse::Shadow, 0.3);
    let late = judgment("ju_l", "perplexity/pplx-decider-v1.3:free", JudgmentUse::Shadow, 0.7);
    s.update(move |tx| tx.put_judgments(vec![acting, shadow]))
        .await
        .unwrap();
    let outcome = |proved| JudgmentOutcome {
        proved,
        source: OutcomeSource::Librarian,
        at: Time::default(),
    };
    let wrong = outcome(Proved::Wrong);
    s.update(move |tx| tx.set_judgment_outcome("ju_a", &wrong))
        .await
        .unwrap();
    // Recorded again, it replaces the one before.
    let right = outcome(Proved::Right);
    s.update(move |tx| tx.set_judgment_outcome("ju_a", &right))
        .await
        .unwrap();
    s.update(move |tx| tx.put_judgments(vec![late])).await.unwrap();
    let unknown = outcome(Proved::Unknown);
    assert!(matches!(
        s.update(move |tx| tx.set_judgment_outcome("ju_none", &unknown)).await,
        Err(Error::NotFound)
    ));

    let mut scores = s.read(|r| r.judgment_scores(&[])).await.unwrap();
    scores.sort_by(|a, b| a.decider.cmp(&b.decider));
    let seen: Vec<(&str, f64, bool)> = scores.iter().map(|x| (x.decider.as_str(), x.p, x.right)).collect();
    assert_eq!(
        seen,
        vec![
            ("liquid/d1.2-ultra", 0.95, true),
            ("perplexity/pplx-decider-v1.3:free", 0.7, true),
            ("typesafe/jev-1.14", 0.3, true),
        ]
    );
    let other = s
        .read(|r| r.judgment_scores(&["gate.risk@1".to_owned()]))
        .await
        .unwrap();
    assert!(other.is_empty(), "another question's scores are not read");
    let tallies = s.read(|r| r.judgment_tallies()).await.unwrap();
    let acted = tallies.iter().find(|t| t.used == JudgmentUse::Acted).unwrap();
    assert_eq!(
        (acted.judgments, acted.right, acted.wrong, acted.unknown),
        (1, 1, 0, 0),
        "the tally counts the outcome as it is now"
    );
    assert!((acted.cost_usd - 0.001).abs() < 1e-12, "and its cost");
}

// The permission wait and the duels' share, stored as bare numbers before migration 127, read as settings that say who set
// them (Remit, before settings recorded it): the number is kept, a wait of 0 (which read as the default) is dropped.
#[tokio::test(flavor = "multi_thread")]
async fn a_stored_permission_wait_and_duel_share_read_as_settings_after_migrating() {
    let dir = tempfile::tempdir().unwrap();
    let path = db(&dir);
    as_an_earlier_build_left_it(
        &path,
        126,
        r#"INSERT INTO settings(id, doc) VALUES('owner', '{"permissionWaitMinutes":45,"duelAgreementPercent":60,"rev":3}');"#,
    )
    .unwrap();
    let s = Store::open(&path).unwrap();
    let settings = s.read(|r| r.settings()).await.unwrap();
    assert_eq!(settings.permission_wait(), model::Duration::minutes(45));
    assert!((settings.duel_agreement() - 0.6).abs() < 1e-9);
    let wait = settings.permission_wait_minutes.unwrap();
    assert_eq!((wait.value, wait.by.kind), (45, model::ActorKind::Remit));
    s.close().unwrap();

    let other = tempfile::tempdir().unwrap();
    let again = db(&other);
    as_an_earlier_build_left_it(
        &again,
        126,
        r#"INSERT INTO settings(id, doc) VALUES('owner', '{"permissionWaitMinutes":0,"rev":1}');"#,
    )
    .unwrap();
    let s = Store::open(&again).unwrap();
    let settings = s.read(|r| r.settings()).await.unwrap();
    assert!(
        settings.permission_wait_minutes.is_none(),
        "0 read as the default, and still does"
    );
}

// The owner's messages and the background tasks a sleeping chain stopped read in one shape whichever build wrote them
// (ADR-0107 step 7e): his words alone, marked as his, and the task's cut in its data. A build before migration 130
// headed his words with the line that says who speaks and kept the cut in the task's words alone; a message Remit words
// for him heads itself in both shapes and is not his words.
#[tokio::test(flavor = "multi_thread")]
async fn an_owners_message_and_a_slept_task_read_in_the_new_shape_whichever_build_wrote_them() {
    let dir = tempfile::tempdir().unwrap();
    let path = db(&dir);
    as_an_earlier_build_left_it(
        &path,
        129,
        r#"
DROP TABLE event_shapes;
INSERT INTO events(home, card_id, kind, status, text, created_at, updated_at, by_kind, by_who) VALUES('c', 'k', 'prompt', 'delivered', '[message from the owner]' || char(10) || 'Fix the login', 0, 0, 'owner', 'me');
INSERT INTO events(home, card_id, kind, status, text, data, created_at, updated_at, by_kind, by_who) VALUES('c', 'k', 'prompt', 'queued', '[message from the owner, through his Assistant]' || char(10) || 'Fix the signup', '{"images":[]}', 0, 0, 'assistant', 'k_a');
INSERT INTO events(home, card_id, kind, status, text, created_at, updated_at, by_kind, by_who) VALUES('c', 'k', 'prompt', 'delivered', '[the owner answered your question "Which?"]' || char(10) || 'The first', 0, 0, 'owner', 'me');
INSERT INTO events(home, card_id, kind, status, title, text, data, created_at, updated_at, by_kind) VALUES('c', 'k', 'task', 'stopped', 'watch the deploy', 'The chain is waiting: its sessions closed before it finished.', '{"taskId":"t1"}', 0, 0, '');
INSERT INTO events(home, card_id, kind, status, title, text, created_at, updated_at, by_kind) VALUES('c', 'k', 'task', 'stopped', 'tail the log', 'R interrupted the turn before it finished.', 0, 0, '');
"#,
    )
    .unwrap();
    let s = Store::open(&path).unwrap();
    let owner = model::by_owner("me", "web");
    let (cut, words) = (model::Cut::Slept, model::with_owner_words(None));
    s.update(move |tx| {
        tx.add_event(model::Event {
            home: "c".into(),
            card_id: "k".into(),
            status: model::EventStatus::Prompt(model::PromptStatus::Delivered),
            by: Some(owner),
            text: "Fix the login".into(),
            data: Some(words),
            ..model::Event::default()
        })?;
        let mut task = model::Event {
            home: "c".into(),
            card_id: "k".into(),
            status: model::EventStatus::Task(model::TaskStatus::Running),
            title: "watch the deploy".into(),
            data: Some(model::ForeignJson(serde_json::json!({"taskId": "t2"}))),
            ..model::Event::default()
        };
        cut.stop_task(&mut task);
        tx.add_event(task)
    })
    .await
    .unwrap();
    let events = s.read(|r| r.card_events("k", 10)).await.unwrap();
    let read: Vec<(String, bool, String)> = events
        .iter()
        .filter(|e| e.kind() == model::EventKind::Prompt)
        .map(|e| {
            (
                e.text.clone(),
                model::holds_owner_words(e),
                model::text_as_read(e).into_owned(),
            )
        })
        .collect();
    assert_eq!(
        read,
        [
            (
                "Fix the login".into(),
                true,
                "[message from the owner]\nFix the login".into()
            ),
            (
                "Fix the signup".into(),
                true,
                "[message from the owner, through his Assistant]\nFix the signup".into()
            ),
            (
                "[the owner answered your question \"Which?\"]\nThe first".into(),
                false,
                "[the owner answered your question \"Which?\"]\nThe first".into()
            ),
            (
                "Fix the login".into(),
                true,
                "[message from the owner]\nFix the login".into()
            ),
        ]
    );
    let tasks: Vec<(Option<model::Cut>, Option<serde_json::Value>)> = events
        .iter()
        .filter(|e| e.kind() == model::EventKind::Task)
        .map(|e| {
            (
                model::Cut::of(e.data.as_ref()),
                e.data.as_ref().and_then(|d| d.field("taskId").cloned()),
            )
        })
        .collect();
    assert_eq!(
        tasks,
        [
            (Some(model::Cut::Slept), Some("t1".into())),
            (Some(model::Cut::Interrupted { who: "R".into() }), None),
            (Some(model::Cut::Slept), Some("t2".into())),
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_existing_image_remains_attached_to_its_draft_and_packed_message() {
    use std::io::Write;

    let dir = tempfile::tempdir().unwrap();
    let path = db(&dir);
    as_an_earlier_build_left_it(
        &path,
        130,
        r#"
INSERT INTO images(id, home, media_type, bytes, at) VALUES('im_old','c','image/png',x'89504e47',0);
INSERT INTO drafts(id, home, at, doc) VALUES('d','c',0,'{"id":"d","home":"c","cardId":"k","images":[{"id":"im_old","mediaType":"image/png"}]}');
"#,
    )
    .unwrap();
    let mut packed = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    packed
        .write_all(br#"{"ownerWords":true,"images":[{"id":"im_old","mediaType":"image/png"}]}"#)
        .unwrap();
    Connection::open(&path)
        .unwrap()
        .execute(
            "INSERT INTO events(home,card_id,kind,status,created_at,updated_at,data) VALUES('c','k','prompt','delivered',0,0,?)",
            [packed.finish().unwrap()],
        )
        .unwrap();

    let s = Store::open(&path).unwrap();
    let (draft, message, attachment) = s
        .read(|r| Ok::<_, Error>((r.draft("d")?, r.event(1)?, r.attachment("im_old")?)))
        .await
        .unwrap();
    assert_eq!(draft.attachments[0].id, "im_old");
    let data = message.data.unwrap().0;
    assert_eq!(data["attachments"][0]["id"], "im_old");
    assert_eq!(data["ownerWords"], true);
    assert!(data.get("images").is_none());
    assert_eq!(attachment, (String::new(), "image/png".into(), b"\x89PNG".to_vec()));
}
