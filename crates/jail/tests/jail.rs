//! The jail's main paths, run for real: this binary is the helper too, so `main` calls `jail::init` first, as Remit's.
//! It has no test harness (the helper must run before any thread), so clippy's allowance of unwrap in tests, which
//! holds for `#[test]` functions, is given here for the same reason.
#![expect(
    clippy::unwrap_used,
    reason = "a test without a harness, where unwrap is the assertion"
)]

use std::path::{Path, PathBuf};
use std::time::Duration;

fn main() {
    jail::init();
    type Test = fn(&tokio::runtime::Runtime);
    let tests: Vec<(&str, Test)> = vec![(
        "a jailed program reads an allowed path and not a hidden one",
        hides_in_a_jail,
    )];
    // libtest's arguments and output, so `cargo test` filters these by name and nextest lists and runs them.
    let trials = tests
        .into_iter()
        .map(|(name, test)| {
            libtest_mimic::Trial::test(name, move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                test(&rt);
                Ok(())
            })
        })
        .collect();
    libtest_mimic::run(&libtest_mimic::Arguments::from_args(), trials).exit();
}

/// A fresh directory, under a real path, with an allowed file and, in a directory of their own, a hidden directory
/// holding a secret and a locked directory holding a kept file: a test hides the one and makes the other read-only.
fn fixture(name: &str) -> (PathBuf, PathBuf, PathBuf) {
    let root = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("jail-test-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    let hidden = root.join("data/hidden");
    std::fs::create_dir_all(&hidden).unwrap();
    std::fs::create_dir_all(root.join("data/locked")).unwrap();
    std::fs::write(root.join("allowed.txt"), "allowed-content").unwrap();
    std::fs::write(hidden.join("secret.txt"), "secret-content").unwrap();
    std::fs::write(root.join("data/locked/kept.txt"), "kept-content").unwrap();
    (root.clone(), root.join("allowed.txt"), hidden)
}

/// Reads the hidden secret directly and again after moving the directory that holds it, which would carry the secret
/// past a rule written on its path.
const READ_SECRET: &str = "cat data/hidden/secret.txt; mv data moved; cat moved/hidden/secret.txt";

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn spec(dir: &Path, script: &str) -> jail::Spec {
    jail::Spec {
        argv: vec!["sh".into(), "-c".into(), script.into()],
        dir: s(dir),
        env: vec!["PATH=/usr/bin:/bin".into()],
        read_only: jail::system_paths(),
        timeout: Duration::from_secs(20),
        max_output: 1 << 16,
        memory_bytes: 1 << 30,
        cpu: Duration::from_secs(10),
        ..Default::default()
    }
}

fn hides_in_a_jail(rt: &tokio::runtime::Runtime) {
    let (dir, allowed, hidden) = fixture("run");
    let mut sp = spec(&dir, &format!("cat {}; {READ_SECRET}", s(&allowed)));
    sp.hide = vec![s(&hidden)];
    let res = rt.block_on(jail::run(sp)).unwrap();
    assert!(
        res.stdout.contains("allowed-content"),
        "the allowed file was not read: {res:?}"
    );
    assert!(
        !res.stdout.contains("secret-content"),
        "the hidden file was read: {res:?}"
    );
    assert!(
        !dir.join("moved").exists(),
        "the hidden path's directory moved: {res:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
