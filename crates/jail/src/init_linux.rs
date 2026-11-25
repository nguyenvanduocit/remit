//! The Linux helper's two stages. Without `pre_exec` a child cannot be cloned straight into its namespaces, so a
//! first stage, this binary as `remit-jail-init`, unshares them, maps the user and starts the second,
//! `remit-jail-namespaced`, inside them: the pid namespace's init, which mounts the program's view, brings up loopback
//! and runs the program.

use crate::status::Report;
use crate::{HELPER_FAILED, HELPER_NAME, namespaces_linux, plan_linux::Plan, supervise_linux};

/// argv\[0\] of the second stage.
pub(crate) const NAMESPACED_NAME: &str = "remit-jail-namespaced";

pub(crate) fn init() {
    let args: Vec<String> = std::env::args().collect();
    match args.as_slice() {
        [name, plan, status] if name == HELPER_NAME => std::process::exit(namespaces_linux::first_stage(plan, status)),
        [name, plan, status] if name == NAMESPACED_NAME => std::process::exit(helper(plan, status)),
        _ => {}
    }
}

/// The second stage: it reports on the status socket and exits with the program's status.
fn helper(body: &str, status: &str) -> i32 {
    let report = match Report::connect(status) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("remit jail: status socket: {e}");
            return HELPER_FAILED;
        }
    };
    let prepared = serde_json::from_str::<Plan>(body)
        .map_err(|e| format!("read plan: {e}"))
        .and_then(|p| set_up(&p).map(|relay| (p, relay)));
    match prepared {
        Ok((plan, relay)) => supervise_linux::supervise(&plan, report, relay),
        Err(why) => {
            report.failed(&why);
            HELPER_FAILED
        }
    }
}

/// Everything privileged, then the giving up of every privilege, then the relay's listener (made before the program
/// starts, so its first connection finds it).
fn set_up(p: &Plan) -> Result<Option<std::net::TcpListener>, String> {
    if p.argv.is_empty() {
        return Err("read plan: no program".into());
    }
    let start = std::env::current_dir().map_err(|e| format!("where the helper started: {e}"))?;
    let dir = p.dir.clone().unwrap_or_else(|| start.to_string_lossy().into_owned());
    crate::mounts_linux::set_up_filesystem(p, &dir)?;
    if p.jail.as_ref().is_some_and(|j| j.loopback) {
        crate::mounts_linux::loopback_up()?;
    }
    // Last of the privileged work: after this the helper holds no capability and the program can get none. It comes
    // before any thread starts, as capabilities belong to a thread.
    supervise_linux::give_up_privileges()?;
    match p.jail.as_ref().and_then(|j| j.socket.as_ref()) {
        Some(_) => std::net::TcpListener::bind(("127.0.0.1", crate::plan_linux::RELAY_PORT))
            .map(Some)
            .map_err(|e| format!("relay: listen on 127.0.0.1:{}: {e}", crate::plan_linux::RELAY_PORT)),
        None => Ok(None),
    }
}
