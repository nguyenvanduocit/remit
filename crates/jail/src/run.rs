//! Running one spec in a jail to its end.

use std::process::{ExitStatus, Stdio};

use tokio::io::{AsyncRead, AsyncReadExt};

use crate::{Error, RunResult, Spec, os_plan, wrap};

/// The variables that send a program's HTTP clients to the relay of its socket. The jail owns them, as it alone knows
/// the relay's address. NO_PROXY is set empty, so that no address is exempt from the proxy.
const PROXY_VARS: [&str; 6] = [
    "HTTPS_PROXY",
    "HTTP_PROXY",
    "https_proxy",
    "http_proxy",
    "NO_PROXY",
    "no_proxy",
];

/// Runs spec in a jail and returns what the program wrote and how it ended. The error is for a jail that could not be
/// built; a program that fails, or that the jail ended for its deadline, is a `RunResult`. Dropping the future ends the
/// program and everything it started.
pub async fn run(spec: Spec) -> Result<RunResult, Error> {
    spec.validate()?;
    let prepared = os_plan::prepare(&spec).await?;
    let mut cmd = wrap::launch(&prepared.plan)?;
    cmd.env_clear().current_dir(&spec.dir);
    for e in &prepared.env {
        if let Some((k, v)) = e.split_once('=') {
            cmd.env(k, v);
        }
    }
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn()?;
    let (out, err) = (child.stdout.take(), child.stderr.take());
    let max = spec.max_output;
    let reads = async { tokio::join!(capped(out, max), capped(err, max)) };
    tokio::pin!(reads);

    let timed_out = tokio::select! {
        status = child.wait() => { status.map_err(|e| Error::new(format!("wait: {e}")))?; false }
        () = tokio::time::sleep(spec.timeout) => true,
    };
    if timed_out {
        os_plan::stop(&mut child).await;
    }
    let status = child.wait().await.map_err(|e| Error::new(format!("wait: {e}")))?;
    let (stdout, stderr) = tokio::time::timeout(procgroup::WAIT_DELAY, &mut reads)
        .await
        .unwrap_or_default();
    drop(prepared);
    Ok(RunResult {
        stdout,
        stderr,
        exit_code: if timed_out { -1 } else { exit_code(status) },
        timed_out,
    })
}

/// How the process ended, as a shell says it: its status, or 128 plus the signal that ended it.
pub(crate) fn exit_code(s: ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    s.code()
        .or_else(|| s.signal().and_then(|sig| sig.checked_add(128)))
        .unwrap_or(-1)
}

/// `env` with every proxy variable set for a relay at 127.0.0.1:port, and none of env's own: the first value of a
/// variable is the one a C library reads, so a copy the spec held would win over the jail's.
pub(crate) fn with_proxy(env: &[String], port: u16) -> Vec<String> {
    let url = format!("http://127.0.0.1:{port}");
    let mut out: Vec<String> = env
        .iter()
        .filter(|e| !PROXY_VARS.contains(&e.split_once('=').map_or(e.as_str(), |(k, _)| k)))
        .cloned()
        .collect();
    for name in PROXY_VARS {
        let value = if name.eq_ignore_ascii_case("NO_PROXY") {
            ""
        } else {
            &url
        };
        out.push(format!("{name}={value}"));
    }
    out
}

/// Reads the pipe to its end, keeping the first `max` bytes and dropping the rest, so that the program writing to it is
/// never blocked by its own output.
async fn capped(pipe: Option<impl AsyncRead + Unpin>, max: usize) -> String {
    let mut kept = Vec::new();
    let mut cut = false;
    if let Some(mut p) = pipe {
        let mut buf = [0u8; 8192];
        while let Ok(n @ 1..) = p.read(&mut buf).await {
            let room = max.saturating_sub(kept.len());
            cut |= n > room;
            kept.extend_from_slice(buf.get(..n.min(room)).unwrap_or_default());
        }
    }
    let mut s = String::from_utf8_lossy(&kept).into_owned();
    if cut {
        s.push_str(&format!("\n[output cut at {max} bytes]\n"));
    }
    s
}
