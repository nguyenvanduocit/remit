use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::time::Duration;

use serde::{Deserialize, Deserializer};

/// A kind of forge Remit speaks to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    GitLab,
    GitHub,
}

impl Kind {
    /// The kind as the owner's configuration names it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GitLab => "gitlab",
            Self::GitHub => "github",
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A repository on a forge: the host it is on and its path there (GitLab's group/project, GitHub's owner/name).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Repo {
    pub host: String,
    pub project: String,
}

/// The account the forge's command line is logged in as on a host: the one Remit lands changes as.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Account {
    pub login: String,
    pub name: String,
    pub email: String,
}

/// Which kind of forge each host is.
pub type Hosts = HashMap<String, Kind>;

/// Why a forge call, or the owner's list of hosts, gave nothing usable.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0:?} is not host=kind")]
    NotHostKind(String),
    #[error("{pair:?}: the kind of forge is {kind:?}, not gitlab or github")]
    UnknownKind { pair: String, kind: String },
    /// The command line could not be run at all.
    #[error("{forge} {method} {path}: {source}")]
    Run {
        forge: &'static str,
        method: &'static str,
        path: String,
        source: procgroup::Error,
    },
    /// The command line ran and refused; `answer` is the end of what it printed.
    #[error("{forge} {method} {path}: {status}: {answer}")]
    Refused {
        forge: &'static str,
        method: &'static str,
        path: String,
        status: ExitStatus,
        answer: String,
    },
    /// An answer that is not what the API says it gives.
    #[error("{forge}: {what}: unexpected answer: {answer}")]
    Unexpected {
        forge: &'static str,
        what: String,
        answer: String,
    },
}

/// Reads the owner's list of hosts, "host=kind" separated by commas: "github.com=github,gitlab.example=gitlab".
pub fn parse_hosts(s: &str) -> Result<Hosts, Error> {
    let mut hosts = Hosts::new();
    for pair in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let Some((host, kind)) = pair.split_once('=') else {
            return Err(Error::NotHostKind(pair.to_owned()));
        };
        let (host, kind) = (host.trim().to_lowercase(), kind.trim());
        if host.is_empty() {
            return Err(Error::NotHostKind(pair.to_owned()));
        }
        let kind = match kind {
            "gitlab" => Kind::GitLab,
            "github" => Kind::GitHub,
            _ => {
                return Err(Error::UnknownKind {
                    pair: pair.to_owned(),
                    kind: kind.to_owned(),
                });
            }
        };
        hosts.insert(host, kind);
    }
    Ok(hosts)
}

/// A forge's command line: glab and gh take the same api subcommand (`api --hostname H [-X M] PATH`).
#[derive(Debug, Clone)]
pub(crate) struct Cli {
    /// The program, from the owner's configuration.
    pub(crate) bin: PathBuf,
    /// The forge, as an error names it.
    pub(crate) name: &'static str,
}

/// `LOCKED_TRIES` and `LOCKED_PAUSE` bound the wait on a repository the forge has locked: GitHub locks one for the seconds
/// a change of visibility or a transfer takes, and every write answers "Repository has been locked" until it is done.
const LOCKED_TRIES: u32 = 6;
const LOCKED_PAUSE: Duration = Duration::from_secs(5);

/// A parameter of an API call, sent as a string, so a value starting with @ is never read as a file.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Field<'a> {
    key: &'a str,
    value: &'a str,
}

/// The field key=value.
pub(crate) fn raw<'a>(key: &'a str, value: &'a str) -> Field<'a> {
    Field { key, value }
}

impl Cli {
    /// Calls the API of host, explicitly: left to itself the command line picks the host from the current directory's
    /// remote. It gives what the API answered. Dropping the future ends the command line's whole process group.
    pub(crate) async fn call(
        &self,
        host: &str,
        method: &'static str,
        path: &str,
        fields: &[Field<'_>],
        extra: &[&str],
    ) -> Result<Vec<u8>, Error> {
        let mut args: Vec<String> = ["api", "--hostname", host, "-X", method]
            .into_iter()
            .chain(extra.iter().copied())
            .chain([path])
            .map(str::to_owned)
            .collect();
        for f in fields {
            args.push("-f".to_owned());
            args.push(format!("{}={}", f.key, f.value));
        }
        let mut cmd = procgroup::command(&self.bin, &args);
        let mut tried = 1;
        loop {
            let out = cmd.output().await.map_err(|source| Error::Run {
                forge: self.name,
                method,
                path: path.to_owned(),
                source,
            })?;
            if out.status.success() {
                return Ok(out.stdout);
            }
            let answer = format!(
                "{} {}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            if tried < LOCKED_TRIES && answer.contains("Repository has been locked") {
                tokio::time::sleep(LOCKED_PAUSE).await;
                tried = tried.saturating_add(1);
                continue;
            }
            return Err(Error::Refused {
                forge: self.name,
                method,
                path: path.to_owned(),
                status: out.status,
                answer: tail(&answer, 400),
            });
        }
    }

    pub(crate) async fn get(&self, host: &str, path: &str, extra: &[&str]) -> Result<Vec<u8>, Error> {
        self.call(host, "GET", path, &[], extra).await
    }

    /// The error for an answer that is not what the API says it gives.
    pub(crate) fn unexpected(&self, what: impl Into<String>, raw: &[u8]) -> Error {
        Error::Unexpected {
            forge: self.name,
            what: what.into(),
            answer: tail(&String::from_utf8_lossy(raw), 300),
        }
    }
}

/// The last n bytes of s, trimmed, marked as cut when they are not all of it; the cut moves forward to a character's start.
fn tail(s: &str, n: usize) -> String {
    let s = s.trim();
    let Some(mut start) = s.len().checked_sub(n).filter(|&start| start > 0) else {
        return s.to_owned();
    };
    while !s.is_char_boundary(start) {
        start = start.saturating_add(1);
    }
    format!("…{}", s.get(start..).unwrap_or_default())
}

/// Reads a JSON null as the field's zero value, as a missing member reads; serde alone refuses one for a string or a bool.
pub(crate) fn nullable<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}
