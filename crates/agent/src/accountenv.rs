use std::collections::BTreeMap;
use std::fmt;

/// The environment a plugin gave for a session on one of its accounts (ADR-0071): the variables of Claude Code that say
/// where the session reaches the model and with which credential. Its default is an account that is a login, which sets
/// none. One that holds a variable comes from `parse_account_env` alone, so a name outside the list reaches no session,
/// and it prints as its names (`Debug`, `Display`), so no log line or error holds a value.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct AccountEnv {
    vars: BTreeMap<String, String>,
}

/// Why a plugin's answer for an account is refused. It names a variable and never holds a value.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AccountEnvError {
    #[error("the account's plugin gave no variable: a session on it would run on nothing")]
    Empty,
    #[error(
        "the account's plugin gave the variable {0:?}, which an account may not set: nothing of its answer is used"
    )]
    NotAllowed(String),
}

/// env, a plugin's answer for an account, as a session may run with it. One name outside the list refuses the whole
/// answer; an answer with no variable is refused too, as its session would run on nothing. It is the one list: the core
/// parses a plugin's answer with it, and the runtime what arrives with a session's open.
pub fn parse_account_env(env: BTreeMap<String, String>) -> Result<AccountEnv, AccountEnvError> {
    if env.is_empty() {
        return Err(AccountEnvError::Empty);
    }
    if let Some(name) = env.keys().find(|name| !account_variable(name)) {
        return Err(AccountEnvError::NotAllowed(name.clone()));
    }
    Ok(AccountEnv { vars: env })
}

/// Says whether an account may set the variable name: what Claude Code reads to reach a provider and sign in to it, and
/// nothing that changes what a session runs or where it keeps its state (PATH, NODE_OPTIONS, CLAUDE_CONFIG_DIR).
fn account_variable(name: &str) -> bool {
    matches!(
        name,
        // Anthropic's API, or a gateway that speaks it: the key or token, the address, the headers the gateway asks
        // for, and the model each of Claude Code's names stands for there.
        "ANTHROPIC_API_KEY"
            | "ANTHROPIC_AUTH_TOKEN"
            | "ANTHROPIC_BASE_URL"
            | "ANTHROPIC_CUSTOM_HEADERS"
            | "ANTHROPIC_DEFAULT_FABLE_MODEL"
            | "ANTHROPIC_DEFAULT_OPUS_MODEL"
            | "ANTHROPIC_DEFAULT_SONNET_MODEL"
            | "ANTHROPIC_DEFAULT_HAIKU_MODEL"
            // A Claude subscription, by the OAuth token the owner made with `claude setup-token` (ADR-0129).
            | "CLAUDE_CODE_OAUTH_TOKEN"
            // Amazon Bedrock: the switch, the region, its credentials, and a gateway in front of it that signs in itself.
            | "CLAUDE_CODE_USE_BEDROCK"
            | "AWS_REGION"
            | "AWS_ACCESS_KEY_ID"
            | "AWS_SECRET_ACCESS_KEY"
            | "AWS_SESSION_TOKEN"
            | "AWS_BEARER_TOKEN_BEDROCK"
            | "ANTHROPIC_BEDROCK_BASE_URL"
            | "CLAUDE_CODE_SKIP_BEDROCK_AUTH"
            // Google Vertex AI: the switch, the region and project, and a gateway in front of it that signs in itself.
            | "CLAUDE_CODE_USE_VERTEX"
            | "CLOUD_ML_REGION"
            | "ANTHROPIC_VERTEX_PROJECT_ID"
            | "ANTHROPIC_VERTEX_BASE_URL"
            | "CLAUDE_CODE_SKIP_VERTEX_AUTH"
    )
}

/// Tells Claude Code to keep its provider credentials out of the environment of what it starts (its shell, its hooks,
/// its MCP servers): Claude Code reads the account's variables, the tools a session runs do not inherit them.
const SUBPROCESS_ENV_SCRUB: &str = "CLAUDE_CODE_SUBPROCESS_ENV_SCRUB=1";

impl AccountEnv {
    /// Says the account is a login: no plugin gave variables for it.
    pub fn is_zero(&self) -> bool {
        self.vars.is_empty()
    }

    /// The variables with their values, for the one caller that carries them on: the core's link, to the runtime that
    /// starts the session. Empty for a login.
    pub fn vars(&self) -> &BTreeMap<String, String> {
        &self.vars
    }

    /// The variables as a process takes them, `NAME=value` in the order of their names, then the scrub. Nothing for a
    /// login.
    pub(crate) fn environ(&self) -> Vec<String> {
        if self.is_zero() {
            return Vec::new();
        }
        let vars = self.vars.iter().map(|(name, value)| format!("{name}={value}"));
        vars.chain([SUBPROCESS_ENV_SCRUB.to_string()]).collect()
    }
}

impl fmt::Display for AccountEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = self.vars.keys().map(String::as_str).collect();
        write!(f, "AccountEnv[{}]", names.join(" "))
    }
}

impl fmt::Debug for AccountEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
