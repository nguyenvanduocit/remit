//! Command remit serves the Remit console: the orchestrator, the link every channel and the AI runtime come in on, the
//! agents' and clients' MCP endpoints, the event signals' door and the built web app, from one process. The same binary is
//! the AI runtime (`remit runtime`), the jail's helper, `remit emit`, `remit export`, `remit update` (puts a release
//! live) and `remit doctor` (checks a running server).

mod agent_home;
mod agentmcp;
mod channels;
mod clock;
mod config;
mod dash_flags;
mod datadog_ports;
mod doctor;
mod emit;
mod errors;
mod export;
mod flags;
mod google_signin;
mod health;
mod host;
mod identity;
mod instance;
mod memory;
mod otlp;
mod ports;
mod probe;
mod rotate;
mod runtime;
mod serve;
mod update;
mod web;

use std::path::PathBuf;
use std::process::ExitCode;

use tokio_util::sync::CancellationToken;

fn main() -> ExitCode {
    // First: a jailed program, and on Linux an agent's session, starts through this binary again, and this is where that
    // run is the jail's helper.
    jail::init();
    // Every HTTPS client and server of this binary (emit, update, doctor, the server's own) uses one crypto, ring.
    rustls::crypto::ring::default_provider().install_default().ok();
    // Read before any thread starts, the only time it can be.
    let offset = clock::local_offset();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let (name, result) = match args.first().map(String::as_str) {
        Some("emit") => (
            "remit emit",
            block_on(emit::run(args.get(1..).unwrap_or_default()))
                .and_then(|r| r)
                .map(|answer| println!("{answer}")),
        ),
        Some("export") => (
            "remit export",
            export::run(args.get(1..).unwrap_or_default(), &home).map(|said| println!("{said}")),
        ),
        Some("update") => (
            "remit update",
            block_on(update::run(args.get(1..).unwrap_or_default(), &home)).and_then(|r| r),
        ),
        Some("doctor") => (
            "remit doctor",
            block_on(doctor::run(args.get(1..).unwrap_or_default(), &home)).and_then(|r| r),
        ),
        Some(runtime::COMMAND) if args.len() == 1 => {
            logging("runtime", None, None);
            ("remit runtime", block_on(runtime::run(offset)).and_then(|r| r))
        }
        _ => ("remit", server(&args, &home, offset)),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{name}: {err}");
            ExitCode::FAILURE
        }
    }
}

/// The server: the command line read, then served until the first SIGINT or SIGTERM, which stops it in order (serve.rs);
/// a second one ends it at once.
fn server(args: &[String], home: &std::path::Path, offset: time::UtcOffset) -> Result<(), String> {
    let defs = flags::defs(home, &std::env::var("CLAUDE_CONFIG_DIR").unwrap_or_default());
    let parsed = match dash_flags::parse(&defs, args) {
        Ok(p) => p,
        Err(dash_flags::Error::Help) => {
            eprintln!("Usage of remit:\n{}", dash_flags::usage(&defs));
            return Ok(());
        }
        Err(dash_flags::Error::Bad(why)) => {
            eprintln!("{why}\nUsage of remit:\n{}", dash_flags::usage(&defs));
            return Err(why);
        }
    };
    let exe =
        std::env::current_exe().map_err(|e| format!("the path of this binary, which sessions cannot change: {e}"))?;
    let f = flags::Flags::of(&parsed).with_paths(&exe, home, &std::env::var("PM2_HOME").unwrap_or_default())?;
    let datadog = datadog_of(&f);
    let (issues, issued) = errors::layer();
    logging("core", Some(&datadog), Some(issues));
    warn_of_secrets_in_environment();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    rt.block_on(async {
        let (sc, cfg) = config::configure(f, home)?;
        let stop = CancellationToken::new();
        tokio::spawn(until_signalled(stop.clone()));
        serve::run(sc, cfg, offset, datadog, issued, stop).await
    })
}

/// Cancels stop at the first SIGINT or SIGTERM; a second one ends the process at once.
async fn until_signalled(stop: CancellationToken) {
    use tokio::signal::unix::{SignalKind, signal};
    let (Ok(mut int), Ok(mut term)) = (signal(SignalKind::interrupt()), signal(SignalKind::terminate())) else {
        tracing::error!("the stop signals cannot be caught: stop the server with SIGKILL");
        return;
    };
    for n in 0..2 {
        tokio::select! {
            _ = int.recv() => {}
            _ = term.recv() => {}
        }
        if n == 0 {
            stop.cancel();
        } else {
            std::process::exit(1);
        }
    }
}

fn block_on<T>(f: impl std::future::Future<Output = T>) -> Result<T, String> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    Ok(rt.block_on(f))
}

/// What this server sends to Datadog (ADR-0119), its records tagged -datadog-env (this host's name unless given) and the
/// release it runs (its build's revision when it runs from elsewhere); sent to the Datadog source's site once its key is
/// entered, and nothing before.
fn datadog_of(f: &flags::Flags) -> datadog::Datadog {
    let env = match f.datadog_env.trim() {
        "" => sysinfo::System::host_name().unwrap_or_else(|_| "remit".to_owned()),
        env => env.to_owned(),
    };
    let tag = instance::release_of(std::path::Path::new(&f.executable), &f.releases_dir());
    let version = if tag.is_empty() {
        health::this_build().revision
    } else {
        tag
    };
    datadog::Datadog::new(&env, &version)
}

/// The process's log: JSON lines on stderr, at info unless RUST_LOG says otherwise, each saying which process wrote it.
/// A line written inside a chain's step or a session's turn carries their spans (`remit.step`, `remit.turn`) and the
/// correlation attributes they set (`remit.chain_id`, `remit.card_id`, `remit.session_id`, `remit.turn`), so one step
/// reads end to end across the crates; each span's close is a line of its own, with how long it was busy. With datadog the
/// spans and lines are also sent there (ADR-0119); with issues the server's errors become `fact:remit.error`.
fn logging(process: &'static str, datadog: Option<&datadog::Datadog>, issues: Option<errors::Issues>) {
    use tracing_subscriber::layer::SubscriberExt as _;
    use tracing_subscriber::util::SubscriberInitExt as _;
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let lines = tracing_subscriber::fmt::layer()
        .json()
        .with_writer(std::io::stderr)
        .with_current_span(true)
        .with_span_list(true)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE);
    let done = tracing_subscriber::registry()
        .with(filter)
        .with(telemetry::Correlate)
        .with(lines)
        .with(datadog.map(datadog::Datadog::layer))
        .with(issues)
        .try_init();
    if done.is_ok() {
        tracing::info!(process, "log");
        if !tracing::dispatcher::get_default(telemetry::correlates) {
            tracing::error!(
                process,
                "the log keeps no correlation attributes: children are told no chain"
            );
        }
    }
}

/// Tells the owner which names in the server's own environment look like credentials. On macOS a session reads the
/// environment of any process of the user (ADR-0021), so a credential the server was started with is one an agent can
/// read; credentials belong in -source-env or the vault's env files. Linux keeps a session in a pid namespace that cannot
/// see the server, so there it says nothing.
fn warn_of_secrets_in_environment() {
    if !cfg!(target_os = "macos") {
        return;
    }
    let env: Vec<String> = std::env::vars().map(|(k, v)| format!("{k}={v}")).collect();
    let names = childenv::secret_names(&env);
    if !names.is_empty() {
        tracing::warn!(
            ?names,
            "the server's environment holds credentials an agent's session can read on macOS: move them to -source-env or the vault's env files and start the server without them"
        );
    }
}
