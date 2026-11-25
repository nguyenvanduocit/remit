//! The binary as a supervisor runs it: started on a fresh data directory, it serves `/healthz` (healthy once its AI runtime
//! connected and the scheduler ticked), tells a page where the link is, serves the web app, and on SIGTERM stops in order
//! and exits 0. The agent command is a stand-in that is signed in and runs no session.
#![expect(
    clippy::unwrap_used,
    reason = "a test, where a failed unwrap is the failed assertion"
)]
#![expect(
    clippy::disallowed_methods,
    reason = "the test sets its fixture up and drives the program as a person would, outside the code under test"
)]

use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpStream;

/// A Claude Code that says it is signed in and runs nothing.
fn fake_claude(dir: &Path) -> String {
    let path = dir.join("claude");
    std::fs::write(&path, "#!/bin/sh\n[ \"$1\" = auth ] && exit 0\nexit 1\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path.display().to_string()
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// GET path on the server: its status and body, or none while it does not answer.
async fn get(port: u16, path: &str) -> Option<(u16, String)> {
    let mut s = TcpStream::connect(("127.0.0.1", port)).await.ok()?;
    let req = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    s.write_all(req.as_bytes()).await.ok()?;
    let mut out = String::new();
    s.read_to_string(&mut out).await.ok()?;
    let status = out.split_whitespace().nth(1)?.parse().ok()?;
    let body = out
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_owned())
        .unwrap_or_default();
    Some((status, body))
}

/// Says hello on a new session at url: welcomed, or why it was refused.
async fn hello(
    client: &link_quic::Client,
    url: &link_quic::Url,
    credential: &str,
    capability: &str,
    bundle: Option<&str>,
) -> Result<(), link::Refusal> {
    let hello = link::Hello {
        node: link::Node {
            id: "test".to_owned(),
            public_key: String::new(),
        },
        credential: credential.to_owned(),
        capabilities: vec![capability.to_owned()],
        offers: Vec::new(),
        bundle: bundle.map(str::to_owned),
    };
    let mut peer = link_quic::dial(client, url.clone(), link::Config::default(), hello)
        .await
        .unwrap();
    let event = tokio::time::timeout(Duration::from_secs(10), peer.events.recv())
        .await
        .unwrap();
    match event {
        Some(link::Event::Welcomed(_)) => Ok(()),
        Some(link::Event::Refused(r) | link::Event::Closed(link::Closing::Refused(r))) => Err(r),
        other => Err(link::Refusal {
            code: link::RefusalCode::BadCredential,
            reason: format!("neither welcomed nor refused: {other:?}"),
        }),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn it_starts_on_a_new_data_directory_answers_healthz_and_stops_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    std::fs::create_dir_all(home.join(".claude")).unwrap();
    // On Linux the runtime links the agents' login from this file instead of asking `claude auth status`
    // (agent_home::prepare); with none it never connects.
    std::fs::write(home.join(".claude/.credentials.json"), "{}").unwrap();
    let data = dir.path().join("data");
    let port = free_port();
    let mut server = tokio::process::Command::new(env!("CARGO_BIN_EXE_remit"))
        .args([
            "-addr",
            &format!("127.0.0.1:{port}"),
            "-data",
            &data.display().to_string(),
        ])
        .args(["-agent", &fake_claude(dir.path())])
        .env("HOME", &home)
        .env("CLAUDE_CONFIG_DIR", home.join(".claude"))
        .env_remove("PM2_HOME")
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    // The server logs to stderr: read it as it comes, or a full pipe blocks its every log line and the server with it.
    let mut stderr = server.stderr.take().unwrap();
    let log = tokio::spawn(async move {
        let mut out = String::new();
        stderr.read_to_string(&mut out).await.ok();
        out
    });
    let tail = |log: String| {
        log.chars()
            .rev()
            .take(4000)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<String>()
    };

    let started = Instant::now();
    let health = loop {
        if let Some((200, body)) = get(port, "/healthz").await {
            break body;
        }
        if let Ok(Some(exit)) = server.try_wait() {
            panic!("the server exited at start ({exit}): {}", tail(log.await.unwrap()));
        }
        if started.elapsed() >= Duration::from_secs(90) {
            let health = get(port, "/healthz").await;
            server.start_kill().ok();
            panic!("not healthy in 90s: {health:?}\n{}", tail(log.await.unwrap()));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    };
    let h: serde_json::Value = serde_json::from_str(&health).unwrap();
    assert_eq!(h["ok"], true, "{h}");
    assert_eq!(h["runtime"]["connected"], true, "{h}");
    assert!(h["schedulerTickAt"].is_string(), "{h}");
    assert!(data.join("remit.db").exists() && data.join("owner.key").exists() && data.join("node.key").exists());

    let (status, link) = get(port, "/link.json").await.unwrap();
    assert_eq!(status, 200);
    let link: serde_json::Value = serde_json::from_str(&link).unwrap();
    assert_eq!(link["url"], format!("https://127.0.0.1:{port}/link"));
    assert_eq!(link["certificateHashes"][0].as_str().unwrap().len(), 64);

    let hash = hex::decode(link["certificateHashes"][0].as_str().unwrap()).unwrap();
    let url = link_quic::Url::parse(link["url"].as_str().unwrap()).unwrap();
    let client = link_quic::client(link_quic::Trust::CertificateHashes(vec![hash])).unwrap();
    let owner_key = std::fs::read_to_string(data.join("owner.key")).unwrap();
    let app = wire::app::CAPABILITY;
    assert_eq!(
        hello(&client, &url, owner_key.trim(), app, None).await,
        Ok(()),
        "the owner comes in"
    );
    let wrong = hello(&client, &url, "a forged key", app, None).await.unwrap_err();
    assert_eq!(wrong.reason, rpc::BAD_TOKEN);
    let code = hello(&client, &url, "wk_no-such-code", app, None).await.unwrap_err();
    assert_eq!(
        code.reason,
        rpc::BAD_TOKEN,
        "a mentor's code that opens nothing is turned away"
    );

    let (status, page) = get(port, "/board").await.unwrap();
    assert_eq!(status, 200, "an app route is the app's page");
    assert!(page.contains("<div id=\"app\""), "{page}");

    let (status, _) = get(port, "/mcp").await.unwrap();
    assert_eq!(status, 401, "the agents' door lets no one in without a run's token");

    let pid = rustix::process::Pid::from_raw(i32::try_from(server.id().unwrap()).unwrap()).unwrap();
    rustix::process::kill_process(pid, rustix::process::Signal::TERM).unwrap();
    let exit = tokio::time::timeout(Duration::from_secs(20), server.wait())
        .await
        .expect("stopped within 20 s")
        .unwrap();
    assert!(exit.success(), "{exit}");
    assert!(get(port, "/healthz").await.is_none(), "nothing listens after the stop");
}
