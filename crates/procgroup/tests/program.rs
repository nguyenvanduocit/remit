//! The runner's rails, run for real: a program gets the environment it is given and nothing of the server's, its HTTP
//! clients reach the proxy it names, its output is capped, and its deadline ends everything it started.

use std::path::PathBuf;
use std::time::Duration;

use procgroup::{Limits, Program};

fn sh(script: &str, env: &[&str]) -> Program {
    Program {
        argv: vec!["sh".into(), "-c".into(), script.into()],
        dir: std::env::temp_dir(),
        env: env.iter().map(|e| (*e).to_owned()).collect(),
        proxy: None,
    }
}

#[tokio::test]
async fn a_program_gets_its_own_environment_and_the_proxy_alone() {
    // The test process has a HOME; the program must not see it.
    assert!(std::env::var_os("HOME").is_some(), "the test runs with a HOME to leak");
    let mut p = sh("env | sort", &["PATH=/usr/bin:/bin", "KEPT=1", "HTTPS_PROXY=http://elsewhere:1"]);
    p.proxy = Some("127.0.0.1:4999".parse().unwrap());
    let ran = p.run(Limits { timeout: Duration::from_secs(10), max_output: 1 << 16 }).await.unwrap();
    assert_eq!(ran.exit_code, 0, "{ran:?}");
    let mut names: Vec<&str> = ran.stdout.lines().filter_map(|l| l.split_once('=').map(|(k, _)| k)).collect();
    names.retain(|n| !["PWD", "SHLVL", "_", "OLDPWD"].contains(n));
    assert_eq!(
        names,
        ["HTTPS_PROXY", "HTTP_PROXY", "KEPT", "NO_PROXY", "PATH", "http_proxy", "https_proxy", "no_proxy"],
        "{}",
        ran.stdout
    );
    assert!(ran.stdout.contains("HTTPS_PROXY=http://127.0.0.1:4999\n"), "{}", ran.stdout);
    assert!(!ran.stdout.contains("elsewhere"), "the env's own proxy won: {}", ran.stdout);
}

#[tokio::test]
async fn a_bare_name_is_found_along_the_programs_own_path() {
    let p = Program {
        argv: vec!["echo".into(), "found".into()],
        dir: std::env::temp_dir(),
        env: vec!["PATH=/bin:/usr/bin".into()],
        proxy: None,
    };
    let ran = p.run(Limits { timeout: Duration::from_secs(10), max_output: 1 << 16 }).await.unwrap();
    assert_eq!((ran.exit_code, ran.stdout.as_str()), (0, "found\n"));
    let none = Program {
        env: vec!["PATH=/nonexistent".into()],
        ..p
    };
    assert!(none.run(Limits { timeout: Duration::from_secs(10), max_output: 1 << 16 }).await.is_err(), "echo ran off the PATH it was given");
}

#[tokio::test]
async fn output_past_the_cap_is_cut_and_said() {
    let ran = sh("head -c 100000 /dev/zero | tr '\\0' x", &["PATH=/usr/bin:/bin"])
        .run(Limits { timeout: Duration::from_secs(10), max_output: 1000 })
        .await
        .unwrap();
    assert_eq!(ran.exit_code, 0);
    assert!(ran.stdout.starts_with(&"x".repeat(1000)), "{:.40}", ran.stdout);
    assert!(ran.stdout.ends_with("[output cut at 1000 bytes]\n"), "{:.1100}", ran.stdout);
}

#[tokio::test]
async fn the_deadline_ends_the_program_and_everything_it_started() {
    let pidfile: PathBuf = std::env::temp_dir().join(format!("procgroup-deadline-{}", std::process::id()));
    let script = format!("sleep 60 & echo $! > {}; wait", pidfile.display());
    let ran = sh(&script, &["PATH=/usr/bin:/bin"])
        .run(Limits { timeout: Duration::from_millis(500), max_output: 1 << 16 })
        .await
        .unwrap();
    assert!(ran.timed_out && ran.exit_code == -1, "{ran:?}");
    let pid: i32 = std::fs::read_to_string(&pidfile).unwrap().trim().parse().unwrap();
    std::fs::remove_file(&pidfile).unwrap();
    let pid = rustix::process::Pid::from_raw(pid).unwrap();
    assert!(
        rustix::process::test_kill_process(pid).is_err(),
        "the program's background child outlived its deadline"
    );
}

#[tokio::test]
async fn no_argv_is_refused() {
    let r = Program::default().run(Limits { timeout: Duration::from_secs(1), max_output: 1 }).await;
    assert!(matches!(r, Err(procgroup::Error::NoProgram)), "{r:?}");
}
