//! How current each model is (ADR-0095 decision 11): what the link's state says of every model, and what the read of
//! its home says of a model being opened or read again. Pure: the core gives what it knows each step, and a freshness
//! is dated from the step it began.

use std::collections::HashMap;

use link::Millis;

use crate::io::Tab;
use crate::shell::{Freshness, Model, Phase, StaleReason};

/// What a model is read with, and so what being read again means for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Home {
    /// The snapshot: the catalog, who signed in, the archived pages read.
    App,
    /// The open chain, read when it is opened and again on a reconnect or its home's reset.
    Chain,
    /// The Assistant's stream, read when asked for and again on a reconnect or its home's reset.
    Assistant,
    /// The open event.
    Event,
    /// The core's own: what the owner typed and sent that the server does not have yet, always current.
    Own,
}

fn home_of(model: &Model) -> Home {
    match model {
        Model::OpenChain
        | Model::Cards
        | Model::Edges
        | Model::Decisions
        | Model::ChainDrafts
        | Model::Workflows
        | Model::Helps
        | Model::AgentStandings
        | Model::Verification
        | Model::Away
        | Model::Retrospectives
        | Model::Landings
        | Model::DatadogLinks
        | Model::ChainEvents
        | Model::CardEvents(_) => Home::Chain,
        Model::AssistantEvents => Home::Assistant,
        Model::OpenEvent => Home::Event,
        Model::Typed | Model::Outbox => Home::Own,
        // The Assistant's view and feed come with the snapshot; only its stream is read on its own.
        Model::Who
        | Model::Chains
        | Model::Inbox
        | Model::Standings
        | Model::Board
        | Model::Playbooks
        | Model::Agents
        | Model::Skills
        | Model::Sources
        | Model::Capabilities
        | Model::Health
        | Model::Workload
        | Model::AtWork
        | Model::Signals
        | Model::Boards
        | Model::Insights
        | Model::Events
        | Model::EventSignals
        | Model::Sensors
        | Model::ClientKeys
        | Model::Extensions
        | Model::Feedback
        | Model::Objectives
        | Model::Projects
        | Model::Employees
        | Model::Mentors
        | Model::MentorStandings
        | Model::Assistant
        | Model::AssistantCard
        | Model::AssistantAgent
        | Model::AssistantDrafts
        | Model::AssistantFeed
        | Model::Archived
        | Model::Repo
        | Model::Repos => Home::App,
    }
}

/// What the core knows at the end of a step.
pub(crate) struct Link {
    pub(crate) now: Millis,
    pub(crate) phase: Phase,
    /// The first snapshot is in, or the server turned this page away: there is a page to call stale.
    pub(crate) settled: bool,
    /// When the link went, once there was a page to keep showing.
    pub(crate) lost_at: Option<Millis>,
    pub(crate) assistant_read: bool,
    /// Each tab, with whether its chain and its event are being read.
    pub(crate) tabs: Vec<Reads>,
}

/// What a tab is reading of what it opened.
pub(crate) struct Reads {
    pub(crate) tab: Tab,
    pub(crate) chain: bool,
    pub(crate) event: bool,
}

/// A freshness without its date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Live,
    Stale(StaleReason),
    Repairing,
}

fn kind(home: Home, read: bool, link: &Link) -> Kind {
    if home == Home::Own {
        return Kind::Live;
    }
    match link.phase {
        _ if !link.settled => Kind::Repairing,
        Phase::Offline => Kind::Stale(StaleReason::Offline),
        Phase::Stale => Kind::Stale(StaleReason::Behind),
        Phase::Syncing => Kind::Repairing,
        Phase::Live if read => Kind::Repairing,
        Phase::Live => Kind::Live,
    }
}

/// Which home's freshness a model of a tab has: the tab's own chain and event, else the one every tab shares.
fn of_tab(home: Home, tab: Tab) -> (Option<Tab>, Home) {
    match home {
        Home::Chain | Home::Event => (Some(tab), home),
        Home::App | Home::Assistant | Home::Own => (None, home),
    }
}

/// Each home's freshness and the step it began, a tab's chain and event for that tab.
#[derive(Debug, Default)]
pub(crate) struct Fresh {
    began: HashMap<(Option<Tab>, Home), (Kind, Millis)>,
    lost_at: Option<Millis>,
}

impl Fresh {
    pub(crate) fn update(&mut self, link: &Link) {
        self.lost_at = link.lost_at;
        let shared = [
            (Home::App, false),
            (Home::Assistant, link.assistant_read),
            (Home::Own, false),
        ]
        .map(|(home, read)| ((None, home), kind(home, read, link)));
        let tabs = link.tabs.iter().flat_map(|r| {
            [
                ((Some(r.tab), Home::Chain), kind(Home::Chain, r.chain, link)),
                ((Some(r.tab), Home::Event), kind(Home::Event, r.event, link)),
            ]
        });
        let now: HashMap<_, _> = shared.into_iter().chain(tabs).collect();
        // A tab gone takes its homes with it.
        self.began.retain(|key, _| now.contains_key(key));
        for (key, kind) in now {
            match self.began.get(&key) {
                Some((was, _)) if *was == kind => {}
                _ => {
                    self.began.insert(key, (kind, link.now));
                }
            }
        }
    }

    /// How current `model` of tab `tab` is: what shows of a model gone stale is from when the link went, or from when
    /// reading it again began to fail.
    pub(crate) fn of(&self, tab: Tab, model: &Model) -> Freshness {
        let Some((kind, began)) = self.began.get(&of_tab(home_of(model), tab)).copied() else {
            return Freshness::Repairing;
        };
        match kind {
            Kind::Live => Freshness::Live { as_of: began },
            Kind::Repairing => Freshness::Repairing,
            Kind::Stale(reason) => Freshness::Stale {
                since: self.lost_at.unwrap_or(began),
                reason,
            },
        }
    }
}
