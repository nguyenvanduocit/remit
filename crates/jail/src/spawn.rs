//! Starting one spec in a jail and leaving it running, its stdin and stdout the caller's: a server that answers on its stdio
//! for as long as its caller holds it (an MCP server, ADR-0077).

use std::process::Stdio;

use crate::{Error, Spec, os_plan, wrap};

/// A program running in a jail. Dropping it ends the program and everything it started, and the relay of its socket.
pub struct Jailed {
    /// The program's process group, its helper at the head; `stdin` and `stdout` are piped, stderr goes nowhere.
    pub child: procgroup::Child,
    _prepared: os_plan::Prepared,
}

/// Starts spec in a jail as `run` does, with its stdin and stdout piped, and returns once it runs. `spec.timeout` and
/// `spec.max_output` are not used: the caller ends the program by dropping it; `spec.cpu` still bounds its processor time.
pub async fn spawn(spec: Spec) -> Result<Jailed, Error> {
    spec.validate()?;
    let prepared = os_plan::prepare(&spec).await?;
    let mut cmd = wrap::launch(&prepared.plan)?;
    cmd.env_clear().current_dir(&spec.dir);
    for e in &prepared.env {
        if let Some((k, v)) = e.split_once('=') {
            cmd.env(k, v);
        }
    }
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    let child = cmd.spawn()?;
    Ok(Jailed {
        child,
        _prepared: prepared,
    })
}
