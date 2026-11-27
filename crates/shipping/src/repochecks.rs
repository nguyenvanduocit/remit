//! A repository declares the checks that grade a change to it (ADR-0025): executables in `.remit/checks`, run in name
//! order. Whatever the work is (a contract, a price list, a campaign plan, code in a language Remit has no check for), the
//! repository carries the program that says whether a change to it is acceptable. Chains cannot change this directory
//! (land.rs `forbidden_path`): a chain that could change the check that grades it would grade itself.

use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use procgroup::Program;
use telemetry::Child;
use tracing::Instrument as _;

use crate::repo::go_checks;
use chains::chain_core::new_id;
use chains::git::{Check, CheckRunner, path_str, tail};

/// Where a repository declares its checks.
pub(crate) const CHECKS_DIR: &str = ".remit/checks";

/// Wall time of one check.
const DECLARED_CHECK_TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// Bytes kept of each of a check's stdout and stderr.
const DECLARED_CHECK_OUTPUT: usize = 64 << 10;

/// The `CheckRunner` of a Remit that runs: the checks the language implies (`go_checks`), and then, only if they pass,
/// the checks the repository declares. A repository that is not Go has none of the first and all of the second.
pub(crate) fn run_checks() -> CheckRunner {
    Arc::new(move |dir: String, changed: Vec<String>| {
        Box::pin(async move {
            let mut out = go_checks(&dir, &changed).await;
            if out.iter().all(|c| c.ok) {
                out.extend(declared_checks(&dir, &changed).await);
            }
            out
        })
    })
}

/// A directory of its own for one run of checks, removed when it is dropped (a directory that cannot be removed is the
/// temp directory's to clean).
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> std::io::Result<Self> {
        let p = std::env::temp_dir().join(new_id("remit-check-"));
        std::fs::DirBuilder::new().mode(0o700).create(&p)?;
        Ok(Self(p))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

/// Runs the checks the repository at dir declares in `CHECKS_DIR`, one after another in name order, each a program of its
/// own (`procgroup::Program`), and stops at the first that fails. A check finds the repository at REMIT_REPO and the paths
/// the change touched in the file REMIT_CHANGED names, one per line; its work directory is a scratch directory. It passes
/// when it exits 0. Nothing is skipped without a word: a file that cannot run, and a program that cannot be started, fail
/// the check, so a submit never passes for want of a place to run its checks.
pub(crate) async fn declared_checks(dir: &str, changed: &[String]) -> Vec<Check> {
    let failed = |output: String| {
        vec![Check {
            command: CHECKS_DIR.to_owned(),
            ok: false,
            output,
        }]
    };
    let entries = match std::fs::read_dir(Path::new(dir).join(CHECKS_DIR)) {
        Ok(e) => e,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(err) => return failed(format!("cannot be read: {err}")),
    };
    let mut runnable: Vec<std::fs::DirEntry> = entries
        .flatten()
        // a README or a helper directory is not a check
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.') && !e.file_type().is_ok_and(|t| t.is_dir()))
        .collect();
    if runnable.is_empty() {
        return Vec::new();
    }
    runnable.sort_by_key(std::fs::DirEntry::file_name);
    let scratch = match Scratch::new() {
        Ok(s) => s,
        Err(err) => return failed(format!("no scratch directory: {err}")),
    };
    let changed_file = scratch.0.join("changed");
    let mut listed = changed.join("\n");
    listed.push('\n');
    if let Err(err) = write_private(&changed_file, &listed) {
        return failed(format!("cannot list the changed paths: {err}"));
    }
    let mut out = Vec::new();
    for e in &runnable {
        let ck = run_declared(dir, &scratch.0, &changed_file, e).await;
        let failed = !ck.ok;
        out.push(ck);
        if failed {
            break;
        }
    }
    out
}

fn write_private(p: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(p)?;
    f.write_all(text.as_bytes())
}

/// Runs one declared check, the entry e of `CHECKS_DIR`.
async fn run_declared(dir: &str, scratch: &Path, changed_file: &Path, e: &std::fs::DirEntry) -> Check {
    let name = e.file_name().to_string_lossy().into_owned();
    let mut ck = Check {
        command: format!("{CHECKS_DIR}/{name}"),
        ..Check::default()
    };
    match e.metadata() {
        Err(err) => {
            ck.output = format!("cannot be read: {err}");
            return ck;
        }
        Ok(m) if !m.file_type().is_file() => {
            "is not a regular file".clone_into(&mut ck.output);
            return ck;
        }
        Ok(m) if m.permissions().mode() & 0o111 == 0 => {
            ck.output = format!(
                "is not executable (mode {:o}): a check that cannot run is not skipped",
                m.permissions().mode() & 0o777
            );
            return ck;
        }
        Ok(_) => {}
    }
    let env = vec![
        format!("REMIT_REPO={dir}"),
        format!("REMIT_CHANGED={}", path_str(changed_file)),
    ];
    let argv = vec![path_str(&Path::new(dir).join(CHECKS_DIR).join(&name))];
    run_check(ck, argv, scratch, scratch, env).await
}

/// Runs line, the command check of a key result (ADR-0068), with sh in the worktree dir, as a declared check runs, with
/// its own scratch directory for HOME and TMPDIR. The check's line is the Chair's, whose sessions write the worktree
/// already. It passes when it exits 0.
pub(crate) async fn command_check(dir: &str, line: &str) -> Check {
    let ck = Check {
        command: line.to_owned(),
        ..Check::default()
    };
    let scratch = match Scratch::new() {
        Ok(s) => s,
        Err(err) => {
            return Check {
                output: format!("no scratch directory: {err}"),
                ..ck
            };
        }
    };
    let argv = vec!["/bin/sh".to_owned(), "-c".to_owned(), line.to_owned()];
    run_check(ck, argv, Path::new(dir), &scratch.0, vec![format!("REMIT_REPO={dir}")]).await
}

/// Runs argv as check ck in work_dir, with scratch as its HOME and TMPDIR, env added to a fixed PATH, and the time and
/// output limits of a check. It runs in a span of its own under the work that runs it (`remit.child`), which it is told of
/// (ADR-0119), and which records how it ended.
async fn run_check(mut ck: Check, argv: Vec<String>, work_dir: &Path, scratch: &Path, env: Vec<String>) -> Check {
    let child = Child::open(&ck.command, "");
    let scratch = path_str(scratch);
    let mut full = vec![
        "PATH=/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin".to_owned(),
        format!("HOME={scratch}"),
        format!("TMPDIR={scratch}"),
    ];
    full.extend(env);
    let program = Program {
        argv,
        dir: work_dir.to_path_buf(),
        env: child.tell(full),
        proxy: None,
    };
    match program
        .run(procgroup::Limits {
            timeout: DECLARED_CHECK_TIMEOUT,
            max_output: DECLARED_CHECK_OUTPUT,
        })
        .instrument(child.span().clone())
        .await
    {
        Err(err) => {
            child.failed();
            ck.output = format!("could not be started: {err}");
        }
        Ok(res) => {
            child.ended(res.exit_code, res.timed_out);
            let words = tail(&format!("{}\n{}", res.stderr, res.stdout), 1500);
            if res.timed_out {
                ck.output = format!("did not end within {DECLARED_CHECK_TIMEOUT:?}\n{words}");
            } else {
                ck.ok = res.exit_code == 0;
                ck.output = words;
            }
        }
    }
    ck
}
