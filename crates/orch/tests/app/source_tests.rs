//! The sources, grants and credentials seen from an agent that holds a chain's tools and from the owner's inbox: an
//! orchestrator whose sources are a real `source::Set` bound to it through `OrchPorts`, as the server builds it.

use std::sync::Arc;

use model::{InboxItem, InboxKind, SourceConnection, by_assistant};
use serde_json::{Map, Value, json};
use source::{Env, Options};
use store::SourceValue;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::tests::{NOW, Nothing, Told, owner};
use orch::orch::Orchestrator;
use orch::{Config, Deps};
use orch::{CreateChain, Request};
use reach::sources::OrchPorts;

/// An orchestrator whose sources are real and reach a Grafana.
struct SourceRig {
    o: Arc<Orchestrator>,
    _dir: tempfile::TempDir,
}

/// A Grafana that answers every request with an empty list.
async fn grafana() -> String {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(async move {
        loop {
            let Ok((mut s, _)) = l.accept().await else { return };
            tokio::spawn(async move {
                let mut buf = [0u8; 8192];
                let _request = s.read(&mut buf).await;
                s
                    .write_all(b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 2\r\nconnection: close\r\n\r\n[]")
                    .await
                    .ok();
            });
        }
    });
    url
}

async fn source_rig() -> SourceRig {
    source_rig_with(Arc::new(Recording::default())).await
}

/// Keeps the jobs it is given and answers each with a harmless output: what reaches the runner.
#[derive(Default)]
struct Recording(std::sync::Mutex<Vec<source::Job>>);

impl source::Runner for Recording {
    fn run<'a>(
        &'a self,
        j: &'a source::Job,
    ) -> source::BoxFuture<'a, std::result::Result<source::Output, source::BoxError>> {
        self.0.lock().unwrap().push(j.clone());
        Box::pin(async {
            Ok(source::Output {
                stdout: r#"{"Account": "123456789012"}"#.into(),
                ..source::Output::default()
            })
        })
    }
}

async fn source_rig_with(runner: Arc<dyn source::Runner>) -> SourceRig {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(store::storetest::open(&dir.path().join("remit.db")).unwrap());
    let ports = OrchPorts::new();
    let shared = source::Shared {
        now: source::Clock::system(),
        http: source::Http::new(),
    };
    let set = source::Set::open(
        ports.store(),
        Env::new(|_| String::new()),
        shared.clone(),
        Options::default()
            .with_policy(ports.policy())
            .with_recorder(ports.recorder())
            .with_plugins(ports.plugins())
            .with_pins(ports.pins())
            .with_runner(runner)
            .with_script_interpreters(|_| true),
    );
    let cfg = Config {
        core: orch::chains::Config {
            work_root: dir.path().join("work"),
            secret_key_file: dir.path().join("secret.key"),
            key_backup_dir: dir.path().join("keys"),
            ext_dir: dir.path().join("ext"),
            claude_config_dir: dir.path().join("claude"),
            ..orch::chains::Config::default()
        },
        ..Config::default()
    };
    let deps = Deps {
        store,
        notify: Arc::new(Told::default()),
        launcher: Arc::new(Nothing),
        sources: Arc::new(set),
        plugins: Arc::new(reach::plugins::PluginProcesses::default()),
        bundled: Arc::new(reach::plugins::BundledPlugins::new(&ports.forge(), &shared)),
        clock: Arc::new(|| NOW),
    };
    let o = Orchestrator::new(cfg, deps);
    ports.bind(&o.reach);
    o.ensure_assistant().await.unwrap();
    o.reach.load_extensions().await; // the plugins that ship with Remit are sources too (github is one)
    let url = grafana().await;
    o.store
        .update(move |tx| -> store::Result<()> {
            for name in ["prod", "staging"] {
                tx.put_source_connection(&SourceConnection {
                    source: "grafana".into(),
                    name: name.into(),
                    description: format!("{name} Grafana"),
                })?;
                tx.put_source_value("grafana", name, "GRAFANA_URL", &SourceValue::Plain(url.clone()))?;
                tx.put_source_value(
                    "grafana",
                    name,
                    "GRAFANA_TOKEN",
                    &SourceValue::Plain(format!("tok-{name}")),
                )?;
            }
            Ok(())
        })
        .await
        .unwrap();
    SourceRig { o, _dir: dir }
}

impl SourceRig {
    /// A work chain holding caps, each given by the owner.
    async fn chain(&self, caps: &[&str]) -> model::Chain {
        let input = CreateChain {
            intent: format!("look at {}", caps.join(" and ")),
            ..CreateChain::default()
        };
        let ch = self
            .o
            .create_chain(owner(&chains::chain_core::new_id("rq_")), input)
            .await
            .unwrap();
        for cap in caps {
            let cmd = chains::step::ChainCmd::Capability(chains::step::CapabilityCmd {
                capability: (*cap).to_owned(),
                why: "the test's chain".to_owned(),
                card_id: String::new(),
                by: model::by_owner("owner", "web"),
            });
            self.o
                .apply(&ch.id, cmd, chains::apply::Records::default())
                .await
                .unwrap();
        }
        self.o.chain(&ch.id).await.unwrap()
    }

    /// Calls tool name as card, the way its MCP session does: the error is what the agent reads.
    async fn call(&self, card: &str, name: &str, args: Value) -> (String, bool) {
        let tools = self.o.tools_for(card).await.unwrap();
        let args: Map<String, Value> = args.as_object().cloned().unwrap_or_default();
        match tools.call(name, Some(args)).await {
            Ok(Ok(out)) => (out, false),
            Ok(Err(e)) => (e.to_string(), true),
            Err(e) => (e.message.to_string(), true),
        }
    }

    async fn read(&self, ch: &model::Chain, connection: &str) -> (String, bool) {
        self.call(&ch.chair_id, "grafana_datasources", json!({"connection": connection}))
            .await
    }

    async fn open_items(&self, kind: InboxKind) -> Vec<InboxItem> {
        let items = self.o.store.read(|r| r.open_inbox()).await.unwrap();
        items.into_iter().filter(|i| i.kind == kind).collect()
    }
}

/// Only the owner types a credential, into his inbox item; then a tool reads through the connection and the secret shows
/// in no answer and no source_calls row.
#[tokio::test]
async fn only_the_owner_enters_a_credential_and_nothing_shows_it() {
    let r = source_rig().await;
    let ch = r.chain(&["observe", "sources"]).await;
    let (out, failed) = r
        .call(
            &ch.chair_id,
            "request_source_credentials",
            json!({"source": "grafana", "connection": "lab", "description": "the lab Grafana", "variables": ["GRAFANA_URL", "GRAFANA_TOKEN"]}),
        )
        .await;
    assert!(!failed, "{out}");
    let items = r.open_items(InboxKind::Credential).await;
    assert!(!items.is_empty(), "no credential item was raised");
    let url = grafana().await;
    for it in &items {
        let value = if it.variable == "GRAFANA_URL" {
            url.clone()
        } else {
            "tok-lab-SECRET-123".to_owned()
        };
        let by_the_assistant = Request {
            id: chains::chain_core::new_id("rq_"),
            by: by_assistant("as"),
        };
        let refused =
            r.o.answer(by_the_assistant, it.id.clone(), "answer".into(), value.clone())
                .await;
        assert!(refused.is_err(), "the Assistant typed a credential");
        r.o.answer(
            owner(&chains::chain_core::new_id("rq_")),
            it.id.clone(),
            "answer".into(),
            value,
        )
        .await
        .unwrap();
    }
    let (out, failed) = r.read(&ch, "lab").await;
    assert!(!failed, "{out}");
    assert!(!out.contains("SECRET"), "the answer shows the secret: {out}");
    let id = ch.id.clone();
    let calls = r.o.store.read(move |r| r.source_calls(&id, 100)).await.unwrap();
    assert!(!calls.is_empty());
    assert!(
        !format!("{calls:?}").contains("SECRET"),
        "a source_calls row shows the secret"
    );
    let items = r.o.store.read(|r| r.open_inbox()).await.unwrap();
    assert!(
        !format!("{items:?}").contains("SECRET"),
        "an inbox item shows the secret"
    );
}

impl SourceRig {}
