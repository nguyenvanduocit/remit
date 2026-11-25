/// The whole environment of a child: what it needs of parent's (`NAME=value` entries, as `os.Environ` gives them), then
/// extra. extra is the caller's own decision, `NAME=value`, and passes whole; it comes last, so its value of a name wins.
pub fn of<P: AsRef<str>, E: AsRef<str>>(parent: &[P], extra: &[E]) -> Vec<String> {
    parent
        .iter()
        .map(AsRef::as_ref)
        .filter(|kv| kv.split_once('=').is_some_and(|(name, _)| inherited(name)))
        .chain(extra.iter().map(AsRef::as_ref))
        .map(str::to_string)
        .collect()
}

/// Says whether a child takes the variable name from its parent. Each entry has a reason; a name is added when a child is
/// seen to fail without it, never because a prefix would be convenient (NODE_* would carry the supervisor's
/// NODE_CHANNEL_FD and break the process; CLAUDE_* the server's own session token; GO* would take
/// GOOGLE_WORKSPACE_CLI_CREDENTIALS_FILE).
///
/// The provider's login is not here: a session signs in from the files of its config dir (CLAUDE_CONFIG_DIR, which the
/// launcher names), and the only API key or token set is a plugin account's, which the launcher sets for that account's
/// sessions alone (ADR-0071). Its account's credential, that login file or those variables, is the one a session holds;
/// the process cannot run without reading it.
fn inherited(name: &str) -> bool {
    match name {
        // The shell and its tools: where programs are, who runs them, and where their per-user configuration (git, glab,
        // ssh) lives.
        "PATH" | "HOME" | "USER" | "LOGNAME" | "SHELL" | "TMPDIR" | "TZ" | "TERM" | "LANG" => true,
        // Reaching the model provider, git remotes and Go modules from a host that needs a proxy or its own certificate
        // authority. A proxy URL that carries a password carries it to the child.
        "HTTP_PROXY"
        | "HTTPS_PROXY"
        | "NO_PROXY"
        | "ALL_PROXY"
        | "http_proxy"
        | "https_proxy"
        | "no_proxy"
        | "all_proxy"
        | "SSL_CERT_FILE"
        | "SSL_CERT_DIR"
        | "NODE_EXTRA_CA_CERTS" => true,
        // An identity the operator gave the server to commit under, so an agent can commit on a machine whose git has
        // none. The merge queue makes each landing's commit again under its forge's account (shipping's pusher_on). Not
        // GIT_CONFIG_VALUE_n: git takes an http.extraHeader from there.
        "GIT_AUTHOR_NAME" | "GIT_AUTHOR_EMAIL" | "GIT_COMMITTER_NAME" | "GIT_COMMITTER_EMAIL" => true,
        // Where the Go toolchain lives and keeps its caches, which modules it may fetch and from where (go help
        // environment). The live server is started with GOROOT and GOPATH, and the go on its PATH, a session's shell and
        // the submit's checks all read the same settings. Not GOAUTH: it names credentials.
        "GOROOT" | "GOPATH" | "GOCACHE" | "GOMODCACHE" | "GOFLAGS" | "GOENV" | "GOTOOLCHAIN" | "GOPROXY"
        | "GONOPROXY" | "GOSUMDB" | "GONOSUMDB" | "GOPRIVATE" => true,
        _ => name.starts_with("LC_") || name.starts_with("XDG_"),
    }
}

/// The names in env (`NAME=value` entries, as `os.Environ` gives them) that look like credentials, sorted and without
/// their values. On macOS any process of the user reads the environment another was started with (ADR-0021), so a secret
/// the server was started with is one its agents' sessions can read; the names are what the owner is told to move out.
/// Going by the name is a heuristic: a credential named otherwise (a connection URI) is not found.
pub fn secret_names<S: AsRef<str>>(env: &[S]) -> Vec<String> {
    let mut names: Vec<String> = env
        .iter()
        .filter_map(|kv| kv.as_ref().split_once('='))
        .map(|(name, _)| name)
        .filter(|name| looks_secret(name))
        .map(str::to_string)
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Says whether a variable's name says it holds a credential: it names one (token, secret, password, credentials) or is
/// a key (API_KEY, SECRET_KEY, but not KEYBOARD_LAYOUT).
fn looks_secret(name: &str) -> bool {
    // Each letter maps to one letter (simple case mapping): str::to_uppercase would turn one into several (ß into SS)
    // and could make a word the name does not hold, so a letter that would expand stays as it is.
    let upper: String = name
        .chars()
        .map(|c| {
            let mut up = c.to_uppercase();
            match (up.next(), up.next()) {
                (Some(u), None) => u,
                _ => c,
            }
        })
        .collect();
    ["TOKEN", "SECRET", "PASSWORD", "CREDENTIAL"]
        .iter()
        .any(|word| upper.contains(word))
        || upper.ends_with("_KEY")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The variable names of env, in order.
    fn names(env: &[String]) -> Vec<&str> {
        env.iter()
            .map(|kv| kv.split_once('=').map_or(kv.as_str(), |(n, _)| n))
            .collect()
    }

    const NONE: &[&str] = &[];

    #[test]
    fn a_child_inherits_no_secret_of_the_server() {
        // A server's environment as supervisors and developer machines leave it: what Remit's sources read, what a host's
        // tooling holds, and what the process that started the server passed down.
        let parent = [
            "AWS_ACCESS_KEY_ID=AKIA-LEAK",
            "AWS_SECRET_ACCESS_KEY=aws-leak",
            "AWS_REGION=eu-west-1",
            "BYTEBASE_SA_EMAIL=sa@leak",
            "BYTEBASE_SA_KEY=bytebase-leak",
            "MONGODB_URI=mongodb://leak",
            "STARROCKS_PASSWORD=leak",
            "GRAFANA_PASSWORD=leak",
            "INTERCOM_ACCESS_TOKEN=leak",
            "COROOT_PUBLIC_PASSWORD=leak",
            "AMPLITUDE_SECRET_KEY=leak",
            "REMIT_FIREBASE_API_KEY=leak",
            "REMIT_MCP_0=Bearer another-sessions-token",
            "ANTHROPIC_API_KEY=sk-leak",
            "CLAUDE_CODE_OAUTH_TOKEN=leak",
            "CLAUDE_CODE_MESSAGING_TOKEN=leak",
            "CLAUDECODE=1",
            "NODE_CHANNEL_FD=3",
            "NODE_OPTIONS=--require=/tmp/x.js",
            "GIT_CONFIG_VALUE_0=Authorization: leak",
            "GOOGLE_WORKSPACE_CLI_CREDENTIALS_FILE=/x",
            "PASEO_PASSWORD=leak",
            "GOAUTH=netrc leak",
            "GOOGLE_APPLICATION_CREDENTIALS=/x",
            "SEAT_BROKER_KEY=leak",
            "GITLAB_TOKEN=leak",
            "SSH_AUTH_SOCK=/tmp/agent.1",
            "KUBERNETES_SERVICE_HOST=10.0.0.1",
        ];
        let got = of(&parent, NONE);
        for kv in &got {
            assert!(
                !kv.contains("leak") && !kv.contains("another-sessions-token"),
                "the child inherits {kv:?}"
            );
        }
        for name in [
            "CLAUDECODE",
            "NODE_CHANNEL_FD",
            "NODE_OPTIONS",
            "SSH_AUTH_SOCK",
            "KUBERNETES_SERVICE_HOST",
            "AWS_REGION",
            "GOAUTH",
            "GOOGLE_APPLICATION_CREDENTIALS",
        ] {
            assert!(
                !names(&got).contains(&name),
                "the child inherits {name}: it is not on the list of what a child needs"
            );
        }
    }
}
