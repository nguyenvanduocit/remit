//! How a source reads its credential variables, and the clock it is given.

use std::collections::BTreeMap;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use jiff::Timestamp;

/// Reads a credential variable by name: empty when it is not set. A source reads only the names it declares. `Env::none()`
/// has none, so a source can be described without credentials.
#[derive(Clone, Default)]
pub struct Env(Option<Arc<Lookup>>);

/// How an Env finds a variable's value.
type Lookup = dyn Fn(&str) -> String + Send + Sync;

impl Env {
    /// The variables `lookup` answers.
    pub fn new(lookup: impl Fn(&str) -> String + Send + Sync + 'static) -> Self {
        Self(Some(Arc::new(lookup)))
    }

    /// No variable at all.
    #[must_use]
    pub const fn none() -> Self {
        Self(None)
    }

    /// The variables of a map.
    #[must_use]
    pub fn of(values: BTreeMap<String, String>) -> Self {
        Self::new(move |name| values.get(name).cloned().unwrap_or_default())
    }

    /// The value of `name`, empty for none.
    #[must_use]
    pub fn get(&self, name: &str) -> String {
        self.0.as_ref().map(|f| f(name)).unwrap_or_default()
    }
}

/// The time of day; a source that needs it is given one, so tests and the orchestrator choose what "now" is.
#[derive(Clone)]
pub struct Clock(Arc<dyn Fn() -> Timestamp + Send + Sync>);

impl Clock {
    pub fn new(now: impl Fn() -> Timestamp + Send + Sync + 'static) -> Self {
        Self(Arc::new(now))
    }

    /// The system's clock.
    #[must_use]
    pub fn system() -> Self {
        Self::new(Timestamp::now)
    }

    /// A clock stopped at `at`.
    #[must_use]
    pub fn fixed(at: Timestamp) -> Self {
        Self::new(move || at)
    }

    #[must_use]
    pub fn now(&self) -> Timestamp {
        (self.0)()
    }
}

/// The `KEY=VALUE` lines of an env file: `export` is allowed, quotes around a value are removed, a line starting with # and a
/// line without = are skipped.
pub fn parse_env(r: impl BufRead) -> std::io::Result<BTreeMap<String, String>> {
    let mut vals = BTreeMap::new();
    for line in r.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim();
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        vals.insert(k.trim().to_owned(), v.trim().trim_matches(['"', '\'']).to_owned());
    }
    Ok(vals)
}

/// The credentials of this machine: a variable of the process environment, else the first of `files` that sets it. A file
/// that is not there adds none (the vault holds only the files of the systems the owner uses); any other failure to read one
/// is reported.
pub fn load_env(process: impl Fn(&str) -> String + Send + Sync + 'static, files: &[PathBuf]) -> std::io::Result<Env> {
    let mut layers = Vec::new();
    for name in files {
        let f = match std::fs::File::open(name) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e),
        };
        layers.push(parse_env(std::io::BufReader::new(f))?);
    }
    Ok(Env::new(move |name| {
        let v = process(name);
        if !v.is_empty() {
            return v;
        }
        layers
            .iter()
            .find_map(|vals| vals.get(name).filter(|v| !v.is_empty()).cloned())
            .unwrap_or_default()
    }))
}

/// The env files of the owner's vault (its secret directory) that hold the credentials of a source, relative to it.
const VAULT_FILES: &[&str] = &[
    "devops/grafana.env",
    "devops/coroot.env",
    "devops/datadog.env",
    "services/intercom.env",
    "services/amplitude.env",
];

/// The paths of the vault's env files under `secret_dir` that sources read; one that is not there adds nothing, and no
/// directory means no vault.
#[must_use]
pub fn vault_files(secret_dir: &Path) -> Vec<PathBuf> {
    if secret_dir.as_os_str().is_empty() {
        return Vec::new();
    }
    VAULT_FILES.iter().map(|f| secret_dir.join(f)).collect()
}

/// The variables among `names` that `env` leaves empty.
pub(crate) fn missing(env: &Env, names: &[&str]) -> Vec<String> {
    names
        .iter()
        .filter(|n| env.get(n).is_empty())
        .map(|n| (*n).to_owned())
        .collect()
}
