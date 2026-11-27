//! The server: it opens the store, builds the orchestrator with its ports, brings back what it serves from, and serves
//! HTTP on TCP and the link on UDP at -addr, with the AI runtime as its child and the scheduler, until it is told to
//! stop; then it stops in the order that keeps every write whole.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::http::header::{CONTENT_TYPE, HOST};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use orch::Orchestrator;
use store::Store;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use crate::channels::Channels;
use crate::config::{ServerConfig, link_url};
use crate::health::{Build, TICK_EVERY, this_build, verdict};
use crate::ports::{LinkAccounts, LinkTranscripts, RuntimeLink};
use crate::runtime::{Settings, starter};

/// How long each step of stopping may take. Together they stay under the 15 s the supervisor gives the process before it
/// kills it (pm2's kill_timeout).
const DRAIN_FOR: Duration = Duration::from_secs(3); // actions the browsers asked for, in progress
const SHUTDOWN_FOR: Duration = Duration::from_secs(8); // agents' turns and background work
const HTTP_FOR: Duration = Duration::from_secs(3); // HTTP calls in progress

/// Serves until `stop` is cancelled, then returns once everything it started has stopped; the database closes last.
pub async fn run(
    sc: ServerConfig,
    mut cfg: orch::Config,
    offset: time::UtcOffset,
    datadog: datadog::Datadog,
    issued: tokio::sync::mpsc::Receiver<orch::shipping::remit_facts::RemitError>,
    stop: CancellationToken,
) -> Result<(), String> {
    // Checked first: with no way to sign in nothing should start.
    let (firebase, firebase_web) = sc.google.map(|g| (g.firebase, g.web)).unzip();
    let owner = Arc::new(
        auth::OwnerGate::new(open_owner_key(&sc.owner_key_file)?, firebase).map_err(|e| format!("owner key: {e}"))?,
    );
    make_workplaces(&cfg)?;
    let store =
        Arc::new(Store::open(&cfg.core.database).map_err(|e| format!("open {}: {e}", cfg.core.database.display()))?);
    let telemetry_db = sc.data_dir.join("telemetry.db");
    let telemetry =
        store::Telemetry::open(&telemetry_db).map_err(|e| format!("open {}: {e}", telemetry_db.display()))?;
    cfg.core.telemetry = Some(telemetry.clone());
    let clock = crate::clock::model_clock(offset);
    crate::memory::start(
        &store,
        &sc.data_dir,
        &sc.old_memory_dir,
        &cfg.core.database,
        (clock)().0,
    )
    .await?;
    let broadcast = rpc::Broadcast::new();
    // Every session runs in the AI runtime, this binary's child, which dials back on the link with the key minted for this
    // start (ADR-0058); the accounts' logins and the transcripts live there with them.
    let key = runtimelink::Key::new().map_err(|e| format!("mint the AI runtime's key: {e}"))?;
    let launcher = runtimelink::Launcher::<RuntimeLink>::new(runtimelink::Options {
        key: key.clone(),
        now: Arc::clone(&clock),
        kill_wait: None,
    });
    let claude_login = cfg.core.claude_config_dir.display().to_string();
    cfg.core.accounts = Some(Arc::new(LinkAccounts(runtimelink::Accounts { l: launcher.clone() })));
    cfg.core.transcripts = Some(Arc::new(LinkTranscripts(runtimelink::Transcripts {
        l: launcher.clone(),
    })));
    cfg.instance = Some(Arc::new(crate::instance::ThisServer {
        store: Arc::clone(&store),
        launcher: launcher.clone(),
        clock: Arc::clone(&clock),
        build: this_build(),
        tag: crate::instance::release_of(&sc.executable, &cfg.core.releases_dir),
        started_at: (clock)(),
    }));
    cfg.datadog = Some(Arc::new(crate::datadog_ports::Exporter(datadog.clone())));
    cfg.deployments = Some(Arc::new(crate::datadog_ports::Deployments(datadog.clone())));
    let (o, sources) = orchestrator(cfg, &store, &broadcast, &launcher, sc.source_env, &clock).await?;
    restore(&o).await?;
    let exporter = tokio::spawn(
        datadog
            .clone()
            .run(crate::datadog_ports::Account(sources), stop.clone()),
    );
    let issues = tokio::spawn(record_issues(Arc::clone(&o), issued, stop.clone()));

    let node = crate::identity::node(&sc.node_key_file)?;
    let cert = crate::identity::certificate((clock)().0)?;
    let listener = link_quic::Listener::bind(sc.link, cert.chain, cert.key, link::Config::default(), node)
        .map_err(|e| format!("listen on the link at udp {}: {e}", sc.link))?;
    let gate: Arc<dyn rpc::Verifier> = Arc::<auth::OwnerGate>::clone(&owner);
    let browsers = CancellationToken::new();
    let channels = Channels {
        app: rpc::Server::new(Arc::clone(&o), Arc::clone(&broadcast), Some(gate), crate::web::bundle()),
        runtime: launcher.clone(),
        browsers: browsers.clone(),
    };
    let http = TcpListener::bind(sc.addr)
        .await
        .map_err(|e| format!("listen at tcp {}: {e}", sc.addr))?;
    tokio::spawn(crate::otlp::forget_old(telemetry.clone(), stop.clone()));
    let routes = router(
        &o,
        &store,
        &launcher,
        sc.link.port(),
        (hex::encode(&cert.hash), cert.not_after),
        &clock,
    )
    .merge(crate::otlp::router(Arc::clone(&o), telemetry, datadog))
    .merge(crate::google_signin::router(Arc::clone(&owner), firebase_web)?);
    if let Some(firebase) = owner.firebase() {
        tracing::info!(project = firebase.project(), "the owner may also sign in with Google");
    }
    tracing::info!(url = %format!("http://{}/", sc.addr), data = %sc.data_dir.display(), "remit listening");
    announce(&sc.addr, owner.key(), &sc.owner_key_file);

    let settings = Settings {
        core_url: link_url(SocketAddr::new(sc.addr.ip(), sc.link.port())),
        certificate_hash: hex::encode(&cert.hash),
        key: key.as_str().to_owned(),
        agents: sc.agents,
        claude_login,
        agents_log: sc.data_dir.join("agents.log").display().to_string(),
    };
    let served = serve(Serving {
        o: Arc::clone(&o),
        launcher,
        channels,
        listener,
        browsers,
        http,
        routes,
        start_runtime: starter(sc.executable, settings),
        stop,
    })
    .await;
    // the issues end at stop; the exporter sends what waits for a few seconds more (`datadog::Datadog::run`)
    for (task, ended) in [("fact:remit.error", issues), ("datadog", exporter)] {
        if let Err(err) = ended.await {
            tracing::warn!(task, %err, "a task did not end cleanly");
        }
    }
    drop(o);
    // The orchestrator's tasks hold the store; what holds it last closes it when dropped.
    match Arc::try_unwrap(store) {
        Ok(s) => s.close().map_err(|e| format!("close the database: {e}"))?,
        Err(_) => tracing::debug!("the database closes when the last task holding it ends"),
    }
    served
}

/// The owner key, made at the first start, which every browser comes in with.
fn open_owner_key(path: &Path) -> Result<auth::LocalKey, String> {
    if let Some(dir) = path.parent() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
            .map_err(|e| format!("owner key: {e}"))?;
    }
    auth::LocalKey::open(path).map_err(|e| format!("owner key: {e}"))
}

use std::os::unix::fs::DirBuilderExt as _;

/// The directories that must be there before any session is: the chains' work, the server's OS user's alone, the
/// extensions' directory and the releases directory, since a path that is not there when a session starts is protected
/// from nothing (on Linux a hidden path that does not exist yet is not hidden).
fn make_workplaces(cfg: &orch::Config) -> Result<(), String> {
    let mk = |p: &Path, mode| {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(mode)
            .create(p)
            .map_err(|e| format!("{}: {e}", p.display()))
    };
    mk(&cfg.core.work_root, 0o700)?;
    mk(&cfg.core.ext_dir, 0o750)?;
    mk(&cfg.core.releases_dir, 0o700)
}

/// The orchestrator over the store, the channels' notifier, the AI runtime's launcher, the sources (whose ports are the
/// orchestrator's own, bound once it is made) and the plugins.
async fn orchestrator(
    cfg: orch::Config,
    store: &Arc<Store>,
    broadcast: &Arc<rpc::Broadcast>,
    launcher: &runtimelink::Launcher<RuntimeLink>,
    source_env: source::Env,
    clock: &orch::Clock,
) -> Result<(Arc<Orchestrator>, source::Set), String> {
    let ports = orch::OrchPorts::new();
    // One clock and one pool of HTTP connections for every source and every plugin that ships with Remit.
    let shared = source::Shared {
        now: source::Clock::system(),
        http: source::Http::new(),
    };
    let sources = source::Set::open(
        ports.store(),
        source_env,
        shared.clone(),
        source::Options::default()
            .with_policy(ports.policy())
            .with_recorder(ports.recorder())
            .with_runner(Arc::new(orch::ProgramRunner::new()))
            .with_plugins(ports.plugins())
            .with_pins(ports.pins())
            .with_script_interpreters(orch::on_program_path),
    );
    // The channels' notifier: the web app's broadcast, which reaches the owner's and the mentors' sessions.
    let notify: Arc<dyn orch::Notifier> = Arc::<rpc::Broadcast>::clone(broadcast);
    let deps = orch::Deps {
        store: Arc::clone(store),
        notify,
        launcher: Arc::new(launcher.clone()),
        sources: Arc::new(sources.clone()),
        plugins: Arc::new(orch::PluginProcesses::default()),
        bundled: Arc::new(orch::BundledPlugins::new(&ports.forge(), &shared)),
        clock: Arc::clone(clock),
    };
    let o = Orchestrator::new(cfg, deps);
    ports.bind(&o.reach);
    o.reach.load_extensions().await;
    let statuses = o.reach.source_status().await.map_err(|e| format!("sources: {e}"))?;
    for st in statuses {
        tracing::info!(name = %st.source, connection = %st.connection, capability = %st.capability, configured = st.missing.is_empty(), missing = ?st.missing, "source");
    }
    o.reach
        .check_secret_key()
        .await
        .map_err(|e| format!("secret key: {e}"))?;
    Ok((o, sources))
}

/// Records each new issue among the server's errors as `fact:remit.error` (ADR-0119), until the layer that hands them is
/// gone. A failure to record one is a warning: as an error it would be an issue of its own, handed back here.
async fn record_issues(
    o: Arc<Orchestrator>,
    mut issued: tokio::sync::mpsc::Receiver<orch::shipping::remit_facts::RemitError>,
    stop: CancellationToken,
) {
    loop {
        let issue = tokio::select! {
            () = stop.cancelled() => return,
            issue = issued.recv() => issue,
        };
        let Some(issue) = issue else { return };
        if let Err(err) = o.shipping.record_remit_error(issue).await {
            tracing::warn!(%err, "fact:remit.error not recorded");
        }
    }
}

/// Brings back what the orchestrator serves from: the Assistant, the crew, the library, the chains at work and the
/// playbooks.
async fn restore(o: &Arc<Orchestrator>) -> Result<(), String> {
    o.ensure_assistant().await.map_err(|e| format!("assistant: {e}"))?;
    // Before recover: a card it restarts runs the agent its role names with the library's skills, and both must be there.
    o.capabilities.seed_agents().await.map_err(|e| format!("agents: {e}"))?;
    o.capabilities
        .seed_library()
        .await
        .map_err(|e| format!("library: {e}"))?;
    // Before recover: the facts it records are answered by them.
    o.pipeline
        .seed_subscriptions()
        .await
        .map_err(|e| format!("subscriptions: {e}"))?;
    o.recover().await.map_err(|e| format!("recover: {e}"))?;
    o.capabilities
        .seed_playbooks()
        .await
        .map_err(|e| format!("playbooks: {e}"))?;
    o.pipeline
        .seed_event_signals()
        .await
        .map_err(|e| format!("event signals: {e}"))?;
    Ok(())
}

/// What `/healthz` and `/link.json` read.
#[derive(Clone)]
struct Probe {
    o: Arc<Orchestrator>,
    store: Arc<Store>,
    launcher: runtimelink::Launcher<RuntimeLink>,
    clock: orch::Clock,
    build: Build,
    port: u16,
    certificate_hash: String,
    /// When the certificate expires, RFC 3339: it is made at each start, so a server up longer than it is valid shuts
    /// browsers out until it restarts (`remit doctor` reads it).
    certificate_expires: String,
}

/// Every HTTP route: the agents' MCP and OTLP doors, the client door, the event signals' door, the health check, where the link is,
/// and the web app.
fn router(
    o: &Arc<Orchestrator>,
    store: &Arc<Store>,
    launcher: &runtimelink::Launcher<RuntimeLink>,
    link_port: u16,
    (certificate_hash, expires): (String, time::OffsetDateTime),
    clock: &orch::Clock,
) -> Router {
    let probe = Probe {
        o: Arc::clone(o),
        store: Arc::clone(store),
        launcher: launcher.clone(),
        clock: Arc::clone(clock),
        build: this_build(),
        port: link_port,
        certificate_hash,
        certificate_expires: expires
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default(),
    };
    Router::new()
        .route("/healthz", get(healthz))
        .route("/link.json", get(where_the_link_is))
        .with_state(probe)
        .merge(crate::agentmcp::router(Arc::clone(o)))
        .merge(clientmcp::router(Arc::new(orch::ClientKeys(Arc::clone(o)))))
        .merge(door::router(Arc::clone(o), Arc::clone(o), Arc::clone(clock)))
        .fallback(crate::web::serve)
}

async fn healthz(State(p): State<Probe>) -> Response {
    let answers = crate::instance::db_answers(&p.store).await;
    let body = verdict(answers, p.o.last_tick(), &p.launcher.state(), (p.clock)(), &p.build);
    let status = if body.ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    json(status, &body)
}

/// Where a page opens its session on the link, and the hash of the certificate it is to trust there (ADR-0072 decision 3):
/// the host it reached this server by, at the link's port.
async fn where_the_link_is(State(p): State<Probe>, headers: HeaderMap) -> Response {
    let host = headers
        .get(HOST)
        .and_then(|h| h.to_str().ok())
        .map(host_of)
        .filter(|h| !h.is_empty())
        .unwrap_or("127.0.0.1");
    let host = if host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    let body = serde_json::json!({
        "url": format!("https://{host}:{}{}", p.port, link_quic::PATH),
        "certificateHashes": [p.certificate_hash],
        "certificateExpires": p.certificate_expires,
    });
    json(StatusCode::OK, &body)
}

/// The host of a Host header, without its port and brackets.
fn host_of(h: &str) -> &str {
    if let Some(rest) = h.strip_prefix('[') {
        return rest.split(']').next().unwrap_or_default();
    }
    h.split(':').next().unwrap_or_default()
}

pub(crate) fn json(status: StatusCode, body: &impl serde::Serialize) -> Response {
    match serde_json::to_vec(body) {
        Ok(mut b) => {
            b.push(b'\n');
            (status, [(CONTENT_TYPE, "application/json")], b).into_response()
        }
        Err(err) => {
            tracing::warn!(%err, "an answer not written");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Tells the owner how to come in: on a terminal, which only the person who started the server sees, the link itself;
/// anywhere else (a supervisor's log, which agents of the same user may read) where the key is, never the key.
fn announce(addr: &SocketAddr, key: &auth::LocalKey, key_file: &Path) {
    use std::io::IsTerminal as _;
    if std::io::stderr().is_terminal() {
        eprintln!(
            "\nRemit is ready. Open this link to come in; it holds your owner key, so keep it to yourself:\n\n    http://{addr}/#key={}\n",
            key.key()
        );
    } else {
        eprintln!(
            "Remit is ready. Open the address with #key=<the key in {}> after it to come in.",
            key_file.display()
        );
    }
}

/// What `serve` runs.
struct Serving<F> {
    o: Arc<Orchestrator>,
    launcher: runtimelink::Launcher<RuntimeLink>,
    channels: Channels,
    listener: link_quic::Listener,
    browsers: CancellationToken,
    http: TcpListener,
    routes: Router,
    start_runtime: F,
    stop: CancellationToken,
}

/// Runs HTTP, the link, the AI runtime (supervised by the launcher every session runs through) and the scheduler, which
/// starts once the runtime is connected and the agents' health check has run, until `stop` (or one of them fails; a
/// runtime that exits before it connects fails the start), then stops in the order that keeps every write whole:
///  1. the browsers leave, and the actions they asked for are finished (new ones are refused);
///  2. the agents end their turns and the work in the background ends, while the MCP endpoint they may still call
///     answers; the next start resumes them and delivers what they had not read;
///  3. the AI runtime ends, closing what sessions are left;
///  4. the HTTP server stops accepting and finishes the calls in progress.
async fn serve<F>(s: Serving<F>) -> Result<(), String>
where
    F: Fn(CancellationToken) -> agent::BoxFuture<'static, Result<Arc<dyn runtimelink::Process>, runtimelink::BoxError>>
        + Send
        + Sync
        + 'static,
{
    let Serving {
        o,
        launcher,
        channels,
        listener,
        browsers,
        http,
        routes,
        start_runtime,
        stop,
    } = s;
    // One of the parts failing stops them all.
    let failed = stop.child_token();
    // The AI runtime outlives the signal to stop: it ends after the agents' turns have.
    let runtime_life = CancellationToken::new();
    let link_life = CancellationToken::new();
    let http_life = CancellationToken::new();
    let supervising = {
        let (l, life, failed) = (launcher.clone(), runtime_life.clone(), failed.clone());
        tokio::spawn(async move {
            let r = l.supervise(&life, start_runtime).await.map_err(|e| e.to_string());
            if r.is_err() {
                failed.cancel();
            }
            r
        })
    };
    tokio::spawn(channels.serve(listener, link_life.clone()));
    let serving_http = {
        let (life, failed) = (http_life.clone(), failed.clone());
        let app = routes.into_make_service_with_connect_info::<SocketAddr>();
        tokio::spawn(async move {
            let r = axum::serve(http, app)
                .with_graceful_shutdown(async move { life.cancelled().await })
                .await
                .map_err(|e| format!("http: {e}"));
            if r.is_err() {
                failed.cancel();
            }
            r
        })
    };
    // The scheduler starts queued chains and the routines once the AI runtime is connected and it knows which providers
    // answer: it starts nothing that cannot run.
    let scheduling = {
        let (o, l, failed) = (Arc::clone(&o), launcher.clone(), failed.clone());
        tokio::spawn(async move {
            tokio::select! {
                () = failed.cancelled() => return, // the server stops before a runtime came
                () = l.ready() => {}
            }
            o.prepare_accounts().await;
            o.check_agents(&failed).await;
            o.run(TICK_EVERY, &failed).await;
        })
    };
    failed.cancelled().await;
    tracing::info!("remit stopping");
    browsers.cancel();
    let drain = deadline(DRAIN_FOR);
    if let Err(err) = o.drain(&drain).await {
        tracing::warn!(%err, "actions channels asked for not drained");
    }
    o.shutdown(&deadline(SHUTDOWN_FOR)).await;
    scheduling.await.ok();
    runtime_life.cancel();
    let runtime = supervising.await.map_err(|e| e.to_string()).and_then(|r| r);
    link_life.cancel();
    http_life.cancel();
    let http = match tokio::time::timeout(HTTP_FOR, serving_http).await {
        Ok(joined) => joined.map_err(|e| e.to_string()).and_then(|r| r),
        Err(_) => {
            tracing::warn!("HTTP calls not drained");
            Ok(())
        }
    };
    runtime.and(http)
}

/// A token cancelled after `d`.
fn deadline(d: Duration) -> CancellationToken {
    let t = CancellationToken::new();
    let at = t.clone();
    tokio::spawn(async move {
        tokio::time::sleep(d).await;
        at.cancel();
    });
    t
}
