//! Speaks to the services a repository is hosted on: GitLab through glab, GitHub through gh. Each is driven through its
//! own command line, logged in on the machine, so Remit holds no token of its own; and each answers in the same words
//! ([`Account`], `model::Checks`), so what Remit asks of a forge is written once. Remit lands changes itself (ADR-0114):
//! a forge says who Remit works as, where a branch stands and how its pipeline stands on a commit. Which service a host
//! is comes from the owner's configuration ([`Hosts`]), never from its name.

mod forge;
mod github;
mod gitlab;

pub use forge::{Account, Error, Hosts, Kind, Repo, parse_hosts};
pub use github::GitHub;
pub use gitlab::GitLab;
