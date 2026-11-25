//! Stamps the binary with the commit it is built from: `REMIT_REVISION` (the commit, "unknown"
//! outside a repository) and `REMIT_MODIFIED` ("true" when the tree had uncommitted changes). A build that sets
//! `REMIT_REVISION` itself (the release workflow) is taken at its word.

use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    #[expect(
        clippy::disallowed_methods,
        reason = "a build script reads the commit it builds from git, as cargo runs it"
    )]
    let out = Command::new("git").args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn main() {
    println!("cargo:rerun-if-env-changed=REMIT_REVISION");
    println!("cargo:rerun-if-env-changed=REMIT_MODIFIED");
    if let Some(dir) = git(&["rev-parse", "--git-dir"]) {
        println!("cargo:rerun-if-changed={dir}/HEAD");
        println!("cargo:rerun-if-changed={dir}/index");
    }
    let (revision, modified) = match std::env::var("REMIT_REVISION") {
        Ok(r) if !r.is_empty() => (r, std::env::var("REMIT_MODIFIED").unwrap_or_default() == "true"),
        _ => match git(&["rev-parse", "HEAD"]) {
            Some(r) => (r, git(&["status", "--porcelain"]).is_some_and(|s| !s.is_empty())),
            None => ("unknown".to_owned(), false),
        },
    };
    println!("cargo:rustc-env=REMIT_REVISION={revision}");
    println!("cargo:rustc-env=REMIT_MODIFIED={modified}");
}
