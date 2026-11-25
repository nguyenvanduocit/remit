//! The macOS plan: what the helper needs, and the parent's side of a jail.

use serde::{Deserialize, Serialize};

use crate::sandbox_darwin::{Sandbox, hidden_profile, real_path, real_paths, within};
use crate::{Error, HELPER_NAME, Spec};

/// Applies a Seatbelt profile to the program it runs, and to everything that program starts; nothing started under it
/// can lift the profile or apply another (ADR-0021).
const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";

/// The host paths that hold the system a program runs on: its shell, its libraries and the common tools, the OpenSSL
/// configurations its programs will not start without, the Command Line Tools that /usr/bin's developer tools run, and
/// the programs Homebrew installs, without the rest of Homebrew's etc and its var, which hold configuration, keys and data.
pub fn system_paths() -> Vec<String> {
    [
        "/usr",
        "/bin",
        "/System",
        "/etc/ssl/openssl.cnf",
        "/Library/Developer/CommandLineTools",
        "/opt/homebrew/bin",
        "/opt/homebrew/opt",
        "/opt/homebrew/Cellar",
        "/opt/homebrew/lib",
        "/opt/homebrew/share",
        "/opt/homebrew/Frameworks",
        "/opt/homebrew/etc/openssl@3/openssl.cnf",
        "/opt/homebrew/etc/openssl@4/openssl.cnf",
    ]
    .map(String::from)
    .to_vec()
}

/// The host paths that hold the certificate authorities a program checks TLS against: the system's bundle and
/// Homebrew's. Programs that ask the system's trust service reach it without a path.
pub fn ca_paths() -> Vec<String> {
    ["/etc/ssl/cert.pem", "/opt/homebrew/etc/ca-certificates"]
        .map(String::from)
        .to_vec()
}

/// Everything the helper needs besides the program's environment and directory, which are its own. The parent starts
/// the helper under sandbox-exec with the plan's profile; the helper applies the limits, finds the program, starts it,
/// and ends whatever of the sandbox is left once the program has ended or the parent asks.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Plan {
    /// The program to run; empty means look `argv[0]` up among the paths the profile shows.
    pub path: String,
    pub argv: Vec<String>,
    /// The Seatbelt profile the helper and the program run under: the parent hands it to sandbox-exec.
    #[serde(skip)]
    pub profile: String,
    /// The real paths the profile lets the program read and refuses it, where a bare `argv[0]` is looked for.
    pub readable: Vec<String>,
    pub hidden: Vec<String>,
    /// Set for a jail's program; a session (a Wrap) runs with none.
    pub limits: Option<Limits>,
}

/// The rlimits of a jail's program.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Limits {
    pub cpu_seconds: u64,
}

/// The path the helper is started from: this binary.
pub(crate) fn self_exe() -> Result<String, Error> {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| Error::new(format!("the path of this binary, which starts the helper: {e}")))
}

/// The plan of a Wrap: everything the user may do, but reading or changing a hidden path, changing a read-only one and
/// signalling a process outside the sandbox.
pub(crate) fn wrap_plan(path: String, argv: Vec<String>, hide: &[String], read_only: &[String]) -> Plan {
    Plan {
        path,
        argv,
        profile: hidden_profile(&real_paths(hide), &real_paths(read_only)),
        readable: Vec::new(),
        hidden: Vec::new(),
        limits: None,
    }
}

/// sandbox-exec, which applies the plan's profile and runs this binary as the helper in the sandbox, so that the helper
/// is a member of the sandbox it has to end. The profile also lets the helper reach its status socket.
pub(crate) fn helper_command(plan: &Plan, body: &str, status: &str) -> Result<procgroup::Command, Error> {
    let exe = self_exe()?;
    let profile = format!(
        "{}(allow network-outbound (remote unix-socket (path-literal {})))\n",
        plan.profile,
        crate::sandbox_darwin::quote(&real_path(status))
    );
    Ok(procgroup::command(
        SANDBOX_EXEC,
        ["-p", &profile, &exe, HELPER_NAME, body, status],
    ))
}

/// What the parent does for a spec, and undoes once the program has ended: the relay of the socket, which listens on the
/// host's loopback at a port of its own, as macOS gives a program no loopback of its own.
pub(crate) struct Prepared {
    pub plan: Plan,
    pub env: Vec<String>,
    pub release: Option<tokio::task::JoinHandle<()>>,
}

impl Drop for Prepared {
    fn drop(&mut self) {
        // Aborting the relay's task drops its listener and every connection it carries.
        if let Some(relay) = self.release.take() {
            relay.abort();
        }
    }
}

pub(crate) async fn prepare(spec: &Spec) -> Result<Prepared, Error> {
    let exe = self_exe()?;
    let mut writable = vec![spec.dir.clone()];
    writable.extend(spec.writable.iter().cloned());
    let mut readable = spec.read_only.clone();
    readable.extend(writable.iter().cloned());
    let writable = real_paths(&writable);
    let kept = real_paths(&spec.read_only)
        .into_iter()
        .filter(|p| writable.iter().any(|w| within(p, w)))
        .collect();
    let mut sandbox = Sandbox {
        helper: real_path(&exe),
        readable: real_paths(&readable),
        writable,
        kept,
        hidden: real_paths(&spec.hide),
        socket: None,
    };
    let (mut env, mut release) = (spec.env.clone(), None);
    if let Some(socket) = &spec.socket {
        use std::os::unix::fs::FileTypeExt;
        let info = std::fs::metadata(socket).map_err(|e| Error::new(format!("socket {socket}: {e}")))?;
        if !info.file_type().is_socket() {
            return Err(Error::new(format!("socket {socket}: not a unix socket")));
        }
        let l = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| Error::new(format!("relay: {e}")))?;
        let port = l.local_addr().map_err(|e| Error::new(format!("relay: {e}")))?.port();
        release = Some(tokio::spawn(crate::relay::serve_relay(l, socket.clone())));
        sandbox.socket = Some((real_path(socket), port));
        env = crate::run::with_proxy(&env, port);
    }
    let plan = Plan {
        path: String::new(),
        argv: spec.argv.clone(),
        profile: sandbox.profile(),
        readable: sandbox.readable.clone(),
        hidden: sandbox.hidden.clone(),
        limits: Some(Limits {
            cpu_seconds: spec.cpu_seconds(),
        }),
    };
    Ok(Prepared { plan, env, release })
}

/// Ends a jail whose deadline passed: a SIGTERM to the helper, which ends every process of the sandbox and then itself;
/// the process group's SIGKILL would end the helper first. A helper that has not ended within the wait delay is killed
/// with its group.
pub(crate) async fn stop(child: &mut procgroup::Child) {
    if let Some(pid) = child
        .id()
        .and_then(|id| rustix::process::Pid::from_raw(id.cast_signed()))
    {
        // ESRCH: the helper already ended, which is what was wanted.
        rustix::process::kill_process(pid, rustix::process::Signal::TERM).ok();
    }
    if tokio::time::timeout(procgroup::WAIT_DELAY, child.wait()).await.is_err()
        && let Some(pid) = child
            .id()
            .and_then(|id| rustix::process::Pid::from_raw(id.cast_signed()))
    {
        rustix::process::kill_process_group(pid, rustix::process::Signal::KILL).ok();
    }
}
