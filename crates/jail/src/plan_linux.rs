//! The Linux plan: what the helper needs, and the parent's side of a jail.

use serde::{Deserialize, Serialize};

use crate::{Error, HELPER_NAME, Spec};

/// Where a jail's relay listens, on the jail's own loopback: each jail has a network namespace of its own, so the port
/// is the same in every jail and cannot clash with the host's.
pub(crate) const RELAY_PORT: u16 = 3128;

/// The host paths that hold the system a program runs on: its shell, its libraries and the common tools.
pub fn system_paths() -> Vec<String> {
    ["/usr", "/bin", "/lib", "/lib64"].map(String::from).to_vec()
}

/// The host paths that hold the certificate authorities a program checks TLS against. Only these, not /etc/ssl as a
/// whole: its private directory holds keys. A path the host lacks is skipped by the jail.
pub fn ca_paths() -> Vec<String> {
    ["/etc/ssl/certs", "/etc/pki/ca-trust", "/etc/pki/tls/certs"]
        .map(String::from)
        .to_vec()
}

/// Everything the helper needs besides the program's environment, which is its own.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Plan {
    /// The program to run; empty means look `argv[0]` up in the jail.
    pub path: String,
    pub argv: Vec<String>,
    /// Where the program starts; none is where the helper started. In a jail it is also bound writable.
    pub dir: Option<String>,
    /// The paths that read as empty.
    pub hide: Vec<String>,
    /// The paths that stay where they are and are remounted read-only; only a plan with no jail has any.
    pub remount: Vec<String>,
    /// Set when the program runs in a root of its own; none leaves it the host's filesystem and network (a Wrap).
    pub jail: Option<JailPlan>,
}

/// What only a jail adds to a plan.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct JailPlan {
    /// An empty directory made by the parent: the mount point of the new root.
    pub root: String,
    /// The paths bound writable besides the work directory.
    pub writable: Vec<String>,
    pub read_only: Vec<String>,
    pub socket: Option<String>,
    pub loopback: bool,
    pub memory_bytes: u64,
    pub cpu_seconds: u64,
}

pub(crate) fn wrap_plan(path: String, argv: Vec<String>, hide: &[String], read_only: &[String]) -> Plan {
    Plan {
        path,
        argv,
        dir: None,
        hide: hide.to_vec(),
        remount: read_only.to_vec(),
        jail: None,
    }
}

/// This binary, as the kernel knows it, whatever it was renamed or replaced by, started as the helper.
pub(crate) fn helper_command(_: &Plan, body: &str, status: &str) -> Result<procgroup::Command, Error> {
    let mut cmd = procgroup::command("/proc/self/exe", [body, status]);
    cmd.arg0(HELPER_NAME);
    Ok(cmd)
}

/// What the parent does for a spec, and undoes once the program has ended: the mount point of the jail's root, made
/// here, as the program's user, so that it is empty and its own.
pub(crate) struct Prepared {
    pub plan: Plan,
    pub env: Vec<String>,
    /// Held to its drop, which removes the mount point.
    pub _release: Root,
}

/// The jail root's mount point, removed when dropped.
pub(crate) struct Root(String);

impl Drop for Root {
    fn drop(&mut self) {
        // An empty directory left in the temporary directory is all a failure leaves.
        std::fs::remove_dir(&self.0).ok();
    }
}

pub(crate) async fn prepare(spec: &Spec) -> Result<Prepared, Error> {
    let root = loop {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("remit-jail-root-{}-{n}", std::process::id()));
        match std::fs::create_dir(&root) {
            Ok(()) => break root.to_string_lossy().into_owned(),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(Error::new(format!("root: {e}"))),
        }
    };
    let env = match &spec.socket {
        Some(_) => crate::run::with_proxy(&spec.env, RELAY_PORT),
        None => spec.env.clone(),
    };
    let plan = Plan {
        path: String::new(),
        argv: spec.argv.clone(),
        dir: Some(spec.dir.clone()),
        hide: spec.hide.clone(),
        remount: Vec::new(),
        jail: Some(JailPlan {
            root: root.clone(),
            writable: spec.writable.clone(),
            read_only: spec.read_only.clone(),
            socket: spec.socket.clone(),
            // A socket is reached through the relay on the jail's own loopback, so it needs that loopback up.
            loopback: spec.socket.is_some(),
            memory_bytes: spec.memory_bytes,
            cpu_seconds: spec.cpu_seconds(),
        }),
    };
    Ok(Prepared {
        plan,
        env,
        _release: Root(root),
    })
}

/// Ends a jail whose deadline passed: its process group, the helper among it, whose end ends the pid namespace.
pub(crate) async fn stop(child: &mut procgroup::Child) {
    if let Some(pid) = child
        .id()
        .and_then(|id| rustix::process::Pid::from_raw(id.cast_signed()))
    {
        // ESRCH: the group already ended, which is what was wanted.
        rustix::process::kill_process_group(pid, rustix::process::Signal::KILL).ok();
    }
}
