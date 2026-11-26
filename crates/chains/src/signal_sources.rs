//! The sources of signals Remit supports, the columns of the Signals tab (ADR-0014): each turns what it learns into
//! readings of keys of one shape, and `signal_key_of` is the one place that reads a key's shape. Time is a source with no
//! readings: a subscription to it carries its own instant. To add a source, give it a `SignalSource`, a case in
//! `signal_key_of` and a line in `signal_source_specs`, beside the adapter that writes its readings.

use model::{
    CHAIN_KEY_PREFIX, EVENT_KEY_PREFIX, FACT_KEY_PREFIX, SCHEDULE_KEY_PREFIX, SignalKey, Subscription,
    chain_sensor_key, schedule_key,
};
use wire::app::SignalSource;

/// What a source is, apart from what is on it now.
pub struct SignalSourceSpec {
    pub source: SignalSource,
    pub title: &'static str,
    pub pattern: String,
    pub what: &'static str,
    pub who: &'static str,
}

/// The sources in the order the owner reads them: time, what comes from outside, what Remit records.
pub fn signal_source_specs() -> Vec<SignalSourceSpec> {
    let spec = |source, title, pattern: String, what, who| SignalSourceSpec {
        source,
        title,
        pattern,
        what,
        who,
    };
    vec![
        spec(
            SignalSource::Clock,
            "Time",
            format!("{SCHEDULE_KEY_PREFIX}<subscription>"),
            "Time itself: a subscription keeps its own schedule (a cron, an interval or a moment), and each instant that comes fires it, recorded under its key at that instant. Nothing is stored before: time is no signal to read, only what it fired. An instant missed while the server was down fires once when it is back, then the next counts from then.",
            "A Chair, to check back on its chain at a time; each routine, to start a chain.",
        ),
        spec(
            SignalSource::Event,
            "Events",
            format!("{EVENT_KEY_PREFIX}<name>"),
            "One reading for each event that comes in, whichever way: a post to the signal's door, `remit emit`, a sensor of a source Remit reads itself, an agent's emit_event. It is stored before its writer is answered; its value is what arrived and what Remit found in it.",
            "Subscriptions the owner or Remit made: each tells the Chair of the chain an event is about, starts a chain from it, or runs a job on it. A chain cannot subscribe to one.",
        ),
        spec(
            SignalSource::Landing,
            "Landings",
            format!("{CHAIN_KEY_PREFIX}<chain>.landing.<repository>"),
            "How a chain's change stands on its way onto its base: the landing, its state (judging, ready, checking, landed, returned, reverted), the version land submitted, the commit it landed as, the forge's checks on that commit, its latest change verdicts and who made the version; a reading each time one of them moves, with what moved it.",
            "The Chair of that chain, and of each chain that serves the same objective, to wake when the change lands or comes back.",
        ),
        spec(
            SignalSource::Live,
            "Change is live",
            format!("{CHAIN_KEY_PREFIX}<chain>.live"),
            "The change of a chain is live, recorded with who said so: the owner through the Assistant, or a rule of the event signal his deploys write.",
            "The Chair of that chain only, to verify the effect once the change is live.",
        ),
        spec(
            SignalSource::Sensor,
            "Sensors",
            chain_sensor_key("<chain>", "<name>").0,
            "The state a sensor reads for a chain: a script the Chair made, run every few minutes behind a source's connection while a watch on its key is open, or the checks a forge's pipeline ran on what a chain landed, which Remit reads from its forge (`landing`). Each reading is a state that differs from the one before.",
            "The Chair of that chain only, which makes a script sensor (create_sensor) and watches its key, to wake when the state meets its condition.",
        ),
        spec(
            SignalSource::Fact,
            "Facts",
            format!("{FACT_KEY_PREFIX}<noun>.<verb>"),
            "One reading for each change of Remit's own state that something answers: a chain queued, a finish requested, a check stated, a card's turn ended. The value names what it is about, never words of the work but for turn.ended, which carries the reply cut for the processor that reads it; the step that changes the state stores it.",
            "The subscriptions that ship with Remit, to hire a judge; the owner, through the Assistant, to start a chain, tell a Chair or run a plugin's processor.",
        ),
    ]
}

/// What a key says of itself: its source, for a chain's record the chain, and its name within the source (an event
/// signal's name, a fact's noun.verb, a landing's repository, a sensor's name).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyOf<'a> {
    pub source: SignalSource,
    pub chain_id: &'a str,
    pub name: &'a str,
}

/// Reads key: its source, and for a chain's record the chain. None for a key no source writes.
pub fn signal_key_of(key: &SignalKey) -> Option<KeyOf<'_>> {
    let k = key.as_str();
    let of = |source, chain_id, name| Some(KeyOf { source, chain_id, name });
    if k.starts_with(SCHEDULE_KEY_PREFIX) {
        return of(SignalSource::Clock, "", "");
    }
    if let Some(name) = k.strip_prefix(FACT_KEY_PREFIX) {
        return of(SignalSource::Fact, "", name);
    }
    if let Some(name) = k.strip_prefix(EVENT_KEY_PREFIX) {
        return of(SignalSource::Event, "", name);
    }
    let (id, record) = k.strip_prefix(CHAIN_KEY_PREFIX)?.split_once('.')?;
    if let Some(repo) = record.strip_prefix("landing.") {
        return of(SignalSource::Landing, id, repo);
    }
    match record {
        "live" => of(SignalSource::Live, id, ""),
        _ => of(SignalSource::Sensor, id, record.strip_prefix("sensor.")?),
    }
}

/// The source of key; `SignalSource::None` for a key no source writes.
pub fn signal_source_of(key: &SignalKey) -> SignalSource {
    signal_key_of(key).map_or(SignalSource::None, |k| k.source)
}

/// The key subscription s acts on: the one its schedule fires under, or its pattern.
pub fn heard_on(s: &Subscription) -> SignalKey {
    if s.schedule.is_some() {
        return schedule_key(&s.id);
    }
    SignalKey(s.r#match.0.clone())
}

/// The noun of the fact key, fact:<noun>.<verb>: what changed. None for a key of any other source.
pub fn fact_noun_of(key: &SignalKey) -> Option<&str> {
    let k = signal_key_of(key).filter(|k| k.source == SignalSource::Fact)?;
    k.name.split('.').next().filter(|noun| !noun.is_empty())
}

/// The name of the event signal key is a reading of: event:<name>, or event:<name>.<word> that a plugin's sensor of that
/// signal writes (`plugin_key`). None for a key of any other source.
pub fn event_signal_of(key: &SignalKey) -> Option<&str> {
    let k = signal_key_of(key).filter(|k| k.source == SignalSource::Event && !k.name.is_empty())?;
    k.name.split('.').next()
}
