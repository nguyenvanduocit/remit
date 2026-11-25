//! The script tool's executor (ADR-0018, form T1): the source crate decides what a script may say (the rules of its
//! injection, carried in `Job::inject`) and builds the job; this runs it in a jail whose only way out is the injecting proxy of
//! crates/egress, made for the call and closed when it ends.
//!
//! The credential is in the proxy, which is in this process, and nowhere the script can look: the job's environment holds
//! none (the source crate builds it with PATH and HOME alone), the jail's files are the scratch directory and the system's
//! read-only paths, and the one file the runner adds to the scratch directory is the certificate of the proxy's authority,
//! which has no key. What the runner adds to the environment is the way to the proxy and the authority to trust, for the
//! clients that read them (python's urllib and requests, curl, node), and the connection's address, which is public:
//! list_sources shows it.

use std::path::Path;

use egress::{InjectConfig, Injector, Upstream};
use source::{BoxError, Job, Output, PROGRAM_PATH};

use crate::program::{GATE_MAX_TUNNELS, JailRunner, ProxyEnd, run_behind, under_system_paths, without};

const SCRIPT_CA_FILE: &str = "ca.pem";
/// Bytes of bodies one run carries, both directions together.
const SCRIPT_MAX_BYTES: u64 = 64 << 20;

/// The variables through which a client is told which authority to trust, all pointing at the one file; the runner owns them
/// as the jail owns the proxy variables. NODE_USE_ENV_PROXY makes node's own clients read HTTPS_PROXY.
const AUTHORITY_VARS: [&str; 4] = [
    "SSL_CERT_FILE",
    "REQUESTS_CA_BUNDLE",
    "CURL_CA_BUNDLE",
    "NODE_EXTRA_CA_CERTS",
];

impl JailRunner {
    /// Runs spec with the injecting proxy for j.inject. The jail shows no certificate authorities of the host: the script
    /// trusts the proxy's alone, which is all it ever talks to.
    pub(crate) async fn run_through_injector(&self, j: &Job, spec: jail::Spec) -> Result<Output, BoxError> {
        let Some(inject) = &j.inject else {
            return Err("the job names no connection to reach".into());
        };
        run_behind(spec, |socket, mut spec| async move {
            let proxy = Injector::new(
                &socket,
                InjectConfig {
                    upstreams: vec![Upstream {
                        base: inject.base.clone(),
                        headers: inject.headers.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                        sign: inject.sign.clone(),
                        read_posts: inject.read_posts.clone(),
                        // A script reads its source; what it may say is what the source's rules admit.
                        writes: false,
                        scrub: inject.scrub.clone(),
                        max_request_body: inject.max_body,
                        max_answer: inject.max_answer,
                    }],
                    deadline: j.timeout,
                    max_bytes: SCRIPT_MAX_BYTES,
                    max_requests: inject.max_requests,
                    max_tunnels: GATE_MAX_TUNNELS,
                    roots: self.upstream_roots.clone(),
                    resolver: None,
                    now: self.now.clone(),
                },
            )
            .await?;
            let authority = j.dir.join(SCRIPT_CA_FILE);
            if let Err(e) = write_private(&authority, &proxy.certificate_pem()) {
                proxy.close().await; // the write error is the one worth returning
                return Err(e.into());
            }
            spec.env = with_injection(&j.env, &authority.to_string_lossy(), &inject.base);
            let end: Box<dyn ProxyEnd> = Box::new(proxy);
            Ok((end, spec))
        })
        .await
    }
}

/// Writes bytes to a file only the server's user reads.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// env with the authority file and the connection's address, and none of the job's own of those names; the jail adds the
/// proxy settings.
fn with_injection(env: &[String], authority: &str, base: &str) -> Vec<String> {
    let mut names: Vec<&str> = AUTHORITY_VARS.to_vec();
    names.extend(["NODE_USE_ENV_PROXY", "API_BASE"]);
    let mut out = without(env, &names);
    out.extend(AUTHORITY_VARS.iter().map(|name| format!("{name}={authority}")));
    out.push("NODE_USE_ENV_PROXY=1".to_owned());
    out.push(format!("API_BASE={}", base.trim_end_matches('/')));
    out
}

/// Whether a jail shows the program: it is on the PATH a script runs with, under a path the jail mounts, and runnable. The
/// jail shows the system's paths and nothing of the user's, so a node under a home directory is not there.
pub fn jail_has(binary: &str) -> bool {
    std::env::split_paths(PROGRAM_PATH).any(|dir| {
        let path = dir.join(binary);
        if !under_system_paths(&path) {
            return false;
        }
        std::fs::metadata(&path).is_ok_and(|m| m.is_file() && runnable(&m))
    })
}

#[cfg(unix)]
fn runnable(m: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    m.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn runnable(_m: &std::fs::Metadata) -> bool {
    true
}
