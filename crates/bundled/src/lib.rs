//! The plugins that ship with Remit: sources written against `plugin_sdk` (ADR-0034) and compiled into the server. Each is a
//! [`Definition`], which implements `plugin_sdk::Plugin`, so the server calls it in its own process with no codec between
//! (ADR-0072 decision 7), behind the same gate as an applied extension. What a plugin ships (its signals, agent templates and
//! subscriptions) is listed here under the plugin's id, and each is named for that plugin, so two plugins never ship the same
//! one.

use std::collections::BTreeMap;

use plugin_sdk::{Agent, Definition};
use source::{Clock, Http, Shared};

mod amplitude;
mod claude;
mod forge;
mod github;
mod gitlab;
mod googleanalytics;
mod intercom;
mod script;

pub use forge::{FORGE_PLUGIN, LANDING_KIND, LandingSettings, LandingWatch};

/// The bundled plugins by id, each on the clock and the HTTP client of `shared`.
#[must_use]
pub fn plugins(shared: &Shared) -> BTreeMap<&'static str, Definition> {
    BTreeMap::from([
        ("amplitude", amplitude::plugin(shared.clone())),
        ("claude", claude::plugin()),
        (FORGE_PLUGIN, forge::plugin()),
        ("github", github::plugin(shared.clone())),
        ("gitlab", gitlab::plugin()),
        ("googleanalytics", googleanalytics::plugin(shared.clone())),
        ("intercom", intercom::plugin(shared.clone())),
        (script::ID, script::plugin(shared.now.clone())),
    ])
}

/// Something a bundled plugin ships, under the id of the plugin. A subscription's processor names one of that plugin's own:
/// the server addresses it as `plugin:<id>.<processor>` (ADR-0049).
#[derive(Debug, Clone, PartialEq)]
pub struct Shipped<T> {
    pub plugin: String,
    pub item: T,
}

/// The agent templates the bundled plugins ship, in plugin order. What a plugin ships is data: the plugins are built on a
/// client that is never used.
#[must_use]
pub fn agents() -> Vec<Shipped<Agent>> {
    let shared = Shared {
        now: Clock::system(),
        http: Http::new(),
    };
    shipped(&plugins(&shared), |p| &p.agents)
}

/// What the plugins of `all` ship of one kind, in order of plugin id.
fn shipped<T: Clone>(all: &BTreeMap<&str, Definition>, of: impl Fn(&Definition) -> &Vec<T>) -> Vec<Shipped<T>> {
    all.iter()
        .flat_map(|(id, p)| {
            of(p).iter().map(|item| Shipped {
                plugin: (*id).to_owned(),
                item: item.clone(),
            })
        })
        .collect()
}
