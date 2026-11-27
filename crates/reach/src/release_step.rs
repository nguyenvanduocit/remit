//! The way out of a step of a repository's release (ADR-0116, form T1): the connections its `.remit/release/connections`
//! names, each through the grant its repository holds, served by one injecting proxy made for the step and closed when
//! it ends. The credential is in the proxy, in this process: the step's environment holds none (shipping builds it with
//! the release's variables alone), and what the runner adds is the way to the proxy and the certificate of its authorities, which has no key. Its output is scrubbed of every
//! credential the proxy holds.
//!
//! Each connection is answered by the grant crate's one rule (`grant::decide`), the repository the holder: a read it
//! does not allow refuses the step, and writes pass only where it allows an `irreversible` effect, as a publish is not
//! undone and the proxy cannot tell one write from another. A lower grant reaches it as a script does, by reads.

use std::path::Path;

use egress::{InjectConfig, Injector, Scrub, Upstream};
use procgroup::{Limits, Program};
use grant::{Action, Decision, Effect, decide};
use model::{GrantHolder, HolderKind};
use source::{BoxError, Output};

use crate::Reach;
use crate::grants::grants_of;
use crate::program::{GATE_MAX_TUNNELS, ProgramRunner, ProxyEnd, run_behind};

/// The file the proxy's authorities are written to, in the step's scratch directory.
const STEP_CA_FILE: &str = "remit-release-ca.pem";
/// Bytes of bodies one step carries, both directions together: a wasm package or a site is uploaded whole.
const STEP_MAX_BYTES: u64 = 2 << 30;
/// Bytes of one request body, and of one answer.
const STEP_BODY: u64 = 512 << 20;
const STEP_ANSWER: u64 = 64 << 20;
/// Requests one step may make, refused ones included.
const STEP_REQUESTS: u64 = 5_000;

/// The variables through which a client is told which authority to trust, as a script's are (script.rs).
const AUTHORITY_VARS: [&str; 4] = [
    "SSL_CERT_FILE",
    "REQUESTS_CA_BUNDLE",
    "CURL_CA_BUNDLE",
    "NODE_EXTRA_CA_CERTS",
];

/// The way a step reaches its connections: one upstream for each, and what removes every credential from its output.
pub struct StepWay {
    pub upstreams: Vec<Upstream>,
    pub scrubs: Vec<Scrub>,
}

impl StepWay {
    /// text with every connection's credential removed.
    pub fn scrubbed(&self, text: &str) -> String {
        self.scrubs.iter().fold(text.to_owned(), |t, s| s(&t))
    }
}

impl Reach {
    /// The way a step of repository repo's release reaches each of connections (source, connection): refused, saying
    /// which and what to do, for a connection the repository may not read and for one with no credential yet. Writes
    /// pass where the repository's grants allow an irreversible effect.
    pub async fn release_way(&self, repo: &str, connections: &[(String, String)]) -> Result<StepWay, String> {
        let holder = GrantHolder {
            kind: HolderKind::Repository,
            id: repo.to_owned(),
        };
        let rows = self
            .store
            .read(move |r| r.grants(&holder))
            .await
            .map_err(|e| format!("the grants of repository {repo}: {e}"))?;
        let held = grants_of(&rows);
        let mut way = StepWay {
            upstreams: Vec::new(),
            scrubs: Vec::new(),
        };
        for (source, connection) in connections {
            let action = |effect| Action {
                source: source.clone(),
                connection: connection.clone(),
                effect,
            };
            if let Decision::Deny { reason, .. } | Decision::Ask { reason, .. } = decide(&held, &action(Effect::Read)) {
                return Err(format!(
                    "repository {repo}: {reason}: the chain whose change names {source}/{connection} in \
                     .remit/release/connections asks with request_grant naming the repository"
                ));
            }
            let writes = decide(&held, &action(Effect::Irreversible)) == Decision::Allow;
            let spec = self
                .sources
                .injection(source, connection)
                .await
                .map_err(|e| format!("{source}/{connection}: {e}"))?;
            let scrub = spec.scrubber();
            way.upstreams.push(Upstream {
                base: spec.base.clone(),
                headers: spec.headers.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                sign: spec.sign.clone(),
                read_posts: spec.read_posts.clone(),
                writes,
                scrub: scrub.clone(),
                max_request_body: STEP_BODY,
                max_answer: STEP_ANSWER,
            });
            way.scrubs.push(scrub);
        }
        Ok(way)
    }
}

impl ProgramRunner {
    /// Runs program, a step of a release, under limits, with way the proxy it reaches its connections through: one
    /// injecting proxy for every connection, its authorities written to the step's scratch directory (`home`) and named
    /// in the variables clients read. What the step printed comes back scrubbed of every credential.
    pub async fn run_release_step(
        &self,
        program: Program,
        home: &Path,
        way: &StepWay,
        limits: Limits,
    ) -> Result<Output, BoxError> {
        let upstreams = way.upstreams.clone();
        let authority = home.join(STEP_CA_FILE);
        let mut out = run_behind(program, limits, |mut program| async move {
            let proxy = Injector::new(
                InjectConfig {
                    upstreams,
                    deadline: limits.timeout,
                    max_bytes: STEP_MAX_BYTES,
                    max_requests: STEP_REQUESTS,
                    max_tunnels: GATE_MAX_TUNNELS,
                    roots: self.upstream_roots.clone(),
                    resolver: None,
                    now: self.now.clone(),
                },
            )
            .await?;
            if let Err(e) = std::fs::write(&authority, proxy.certificate_pem()) {
                proxy.close().await; // the write error is the one worth returning
                return Err(e.into());
            }
            let file = authority.to_string_lossy().into_owned();
            let mut env = crate::program::without(&program.env, &AUTHORITY_VARS);
            env.extend(AUTHORITY_VARS.iter().map(|name| format!("{name}={file}")));
            env.push("NODE_USE_ENV_PROXY=1".to_owned());
            program.env = env;
            let end: Box<dyn ProxyEnd> = Box::new(proxy);
            Ok((end, program))
        })
        .await?;
        out.stdout = way.scrubbed(&out.stdout);
        out.stderr = way.scrubbed(&out.stderr);
        Ok(out)
    }
}
