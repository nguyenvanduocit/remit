use std::ffi::OsStr;
use std::path::Path;

use crate::procgroup::{Error, command};

/// The intermediate shell: it checks the program and the log, starts the program in the background with its output
/// appended to the log and no input, prints the program's pid and exits. The program is then nobody's child but init's
/// (or a subreaper's), in the shell's process group, which the server's is not.
const SCRIPT: &str = r#"log=$1; shift
command -v "$1" >/dev/null || { echo "$1: not a program" >&2; exit 127; }
: >>"$log" || exit 1
"$@" </dev/null >>"$log" 2>&1 &
echo $!"#;

/// Starts name with args detached from the calling process, so that a supervisor restarting the server cannot end it,
/// and returns its pid. The program is forked twice: `/bin/sh` (a process group of its own, `command`) starts it in the
/// background and exits, so the program is reparented to init (or a subreaper) and is in no process group and no process
/// tree of the server's. A kill of the server's group (launchd without `AbandonProcessGroup`, procgroup's own drop) or of
/// its descendants by parent pid (pm2's `treekill`) does not reach it, and it carries no parent-death signal, which a fork
/// clears. A kill of the server's whole control group (systemd's `KillMode=control-group`) still does.
///
/// env is the program's whole environment, `NAME=value` entries as `childenv::of` gives them: nothing of the server's is
/// inherited besides. The working directory is the caller's. Standard input is `/dev/null`, standard output and error are
/// appended to log, created if missing. Dropping the future before it returns kills the shell's group, the program too
/// if it was started.
pub async fn detach<I, S, E>(name: impl AsRef<OsStr>, args: I, env: &[E], log: &Path) -> Result<u32, Error>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
    E: AsRef<str>,
{
    let mut sh = command(
        "/bin/sh",
        [
            OsStr::new("-c"),
            OsStr::new(SCRIPT),
            OsStr::new("sh"),
            log.as_os_str(),
            name.as_ref(),
        ],
    );
    sh.args(args)
        .env_clear()
        .envs(env.iter().filter_map(|kv| kv.as_ref().split_once('=')));
    let out = sh.output().await?;
    let pid = std::str::from_utf8(&out.stdout)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok());
    match pid {
        Some(pid) if out.status.success() => Ok(pid),
        _ => Err(Error::NotDetached {
            status: out.status,
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        }),
    }
}

/// Whether the program detach started as pid still runs. Once it ends, init (or the subreaper) reaps it and the pid names
/// nothing; a pid the OS gave again to another process reads as running, which errs on the side of waiting.
pub fn running(pid: u32) -> bool {
    let Some(pid) = i32::try_from(pid).ok().and_then(rustix::process::Pid::from_raw) else {
        return false;
    };
    match rustix::process::test_kill_process(pid) {
        Ok(()) => true,
        Err(err) => err != rustix::io::Errno::SRCH,
    }
}
