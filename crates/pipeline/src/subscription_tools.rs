//! The Assistant's subscription tools: how the owner has Remit do something about the events of an event signal, or
//! about a fact Remit records (fact:turn.ended, a card's reply), by telling the Assistant (ADR-0036). A subscription
//! holds a `when` over the reading and one action: tell the Chair of the chain it is about, start a chain from it, run a
//! job on the chain it is about, or run an enabled plugin's processor on it. Each is the orchestrator's own action, taken
//! as the Assistant for the owner, so he sees it on the Subscriptions tab at once and who made it. A routine (a
//! subscription that keeps a schedule) has its own tools.
//!
//! A subscription made here is Remit's own (`OwnerKind::Remit`), as the ones that ship are: it is the owner's by its
//! author, which the tab shows. The subscription tools take event and fact keys; the shipped subscriptions to facts are
//! the automation tools', and a chain's record is the Chair's watches'.

use std::collections::BTreeMap;
use std::sync::Arc;

use model::{
    ActionKind, Actor, ChainKind, ChainOrigin, Control, Duration, Flow, FlowPolicy, OwnerKind, SignalKey,
    SignalPattern, Start, StartFrom, StartSpec, Subscription, SubscriptionAction, SubscriptionOwner,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Pipeline;
use crate::action_params::check_action;
use crate::automation::subscription_ships_as;
use crate::record::{LiftHow, May, any_subscription};
use crate::routine::start_spec_of;
use crate::subscription_inspect::DryRunRead;
use chains::chain_core::{first_line, new_id};
use chains::signal_sources::{fact_noun_of, signal_key_of};
use chains::subscribe::check_subscription;
use chains::subscribe::{StartInput, start_seq, subscribe};
use chains::tools::{Effect, NoInput, Tools, as_json};
use chains::wait::{rfc3339, wait_owner};
use chains::{Error, Result};
use wire::app::SignalSource;

fn is_empty_map(m: &BTreeMap<String, String>) -> bool {
    m.is_empty()
}

/// A subscription to an event signal as the Assistant's tools take it. It is what update_subscription replaces one with.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
pub(crate) struct SubscriptionInput {
    /// what it listens to: an event signal, event:<name>, as list_event_signals shows it, or a family of them, a pattern such as event:* (* matches any run of characters, ? one); or a fact Remit records, fact:<noun>.<verb>, such as fact:turn.ended (a card's turn ended, with its reply)
    pub key: String,
    /// what it does, in one line of the owner's language, so he tells it from the others on the same signal: Tell the Chair of the chain that its pipeline failed. Its line on the Subscriptions tab is titled by it
    pub note: String,
    /// a CEL expression over body (the event's JSON), header (its headers, keyed as sent) and cause; for a fact over value (the fact's JSON: fact:turn.ended has role, opener, ended, card, chain). The readings it is false for are not heard, and a field the reading lacks makes it false. Empty: every reading. Read a real event's fields (read_event_fields) before you write one
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub when: String,
    /// for start only: when an event deserves a chain, in plain words a decider can judge true or false of one event, as a statement: The error is one a person must fix, not a retry that already succeeded. It is asked of each event `when` lets through; one it is sure deserves none is not given a chain, and is shown on the Subscriptions tab as screened out. One in twenty of those fires anyway, so Remit learns whether the decider was right; an unsure answer, a late one and none fire as they would without it. Not for a batch flow. Empty: every event `when` lets through starts a chain. What the event says leaves this machine for the decider, with credentials removed and email addresses masked, unless its event signal is kept on this machine (update_event_signal)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub screen: String,
    /// tell: send the event to the Chair of the chain it is about (give with.chain); start: start a chain from it (give title and goal); run: run a job of Remit's on it (give job, and with.chain for one that acts on a chain); plugin:<id>.<name>: run that processor of an enabled plugin on it (list_processors shows them; give params)
    pub action: String,
    /// what the action reads of the event, each a CEL expression: chain (marks, a list of a branch and a commit a chain landed, or a chain's id), summary (what happened, in a line), quote (outsiders' words it carries), author (who wrote them). tell takes all four, start takes summary, quote and author, run takes chain and summary
    #[serde(default, skip_serializing_if = "is_empty_map")]
    pub with: BTreeMap<String, String>,
    /// for run: mark-live (record the chain's change as live) or sync-landing (read the checks the forge ran on what it landed)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub job: String,
    /// for start: short name in the owner's language; it heads every chain it starts
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    /// for start: what each chain is to do, in the owner's words, self-contained: the chain's Chair reads this and the event as evidence
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub goal: String,
    /// for start: the kind of chain it starts, work (the default) or chore (a small, well-understood change); a chain an
    /// event starts takes no other. A playbook that fits is named in goal
    #[serde(default)]
    pub kind: ChainKind,
    /// for a plugin's processor: the settings it takes, a JSON object valid against the schema list_processors shows
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    /// how the events it holds for become firings over time. Empty: each, every event fires at once
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow: Option<FlowInput>,
}

/// A subscription's flow policy as the tools take it (`model::Flow`).
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct FlowInput {
    /// each: every event fires at once; delay: every event fires alone, at most one each period, none lost; last: once the events stop for a period, or max_wait_seconds passed, only the newest of each fires. tell, start and run take one event at a time, so they take each, delay or last
    pub policy: String,
    /// for delay and last: the period, in seconds
    #[serde(default, skip_serializing_if = "is_zero")]
    pub period_seconds: i64,
    /// for last: the longest the newest event waits after the first it replaced, in seconds. Empty: the period
    #[serde(default, skip_serializing_if = "is_zero")]
    pub max_wait_seconds: i64,
}

fn is_zero(n: &i64) -> bool {
    *n == 0
}

/// The flow f says; the zero Flow, each, when it says none, and each when it names no policy.
fn flow_of(f: Option<&FlowInput>) -> Result<Flow> {
    let Some(f) = f else { return Ok(Flow::default()) };
    let policy = match f.policy.trim() {
        "" => FlowPolicy::Each,
        named => named
            .parse()
            .map_err(|e: model::UnknownValue| Error::refused(format!("flow: {e}")))?,
    };
    Ok(Flow {
        policy,
        period: Duration(f.period_seconds.saturating_mul(Duration::SECOND.0)),
        max_wait: Duration(f.max_wait_seconds.saturating_mul(Duration::SECOND.0)),
    })
}

/// flow as the tools show it; none for each.
fn flow_input_of(flow: &Flow) -> Option<FlowInput> {
    if flow.policy == FlowPolicy::Each {
        return None;
    }
    Some(FlowInput {
        policy: flow.policy.to_string(),
        period_seconds: flow.period.0 / Duration::SECOND.0,
        max_wait_seconds: flow.max_wait.0 / Duration::SECOND.0,
    })
}

/// A subscription to make, and where it starts judging.
#[derive(Deserialize, JsonSchema)]
struct CreateSubscriptionInput {
    #[serde(flatten)]
    subscription: SubscriptionInput,
    /// where it starts judging the events. Empty: now, the events that come after it is made
    #[serde(default)]
    start: Option<StartInput>,
    /// true: make nothing, and answer what it would have done over the events kept, from start (empty: the kept log from its beginning), as dry_run_subscription does
    #[serde(default)]
    dry_run: bool,
}

#[derive(Deserialize, JsonSchema)]
struct UpdateSubscriptionInput {
    /// subscription id (sub_…), as list_subscriptions shows it
    id: String,
    #[serde(flatten)]
    subscription: SubscriptionInput,
    /// where it judges from after the change. Empty: where it stands, so what it has not judged yet it judges as changed
    #[serde(default)]
    start: Option<StartInput>,
    /// true: change nothing, and answer what this version would have done over the events kept, from start (empty: the kept log from its beginning), as dry_run_subscription does
    #[serde(default)]
    dry_run: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SubscriptionIdInput {
    /// subscription id (sub_…), as list_subscriptions shows it
    pub id: String,
}

/// A subscription whose hold is lifted, and why.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct LiftInput {
    /// subscription id (sub_…, rt_…), as list_subscriptions or read_signals shows it
    pub id: String,
    /// for a held subscription: what was fixed, or why the failed firing is given up, in a line; it is recorded with the lift
    #[serde(default)]
    pub why: String,
}

/// What a lift of sub's hold, how, answers.
pub(crate) fn lifted_words(sub: &Subscription, how: LiftHow) -> String {
    match how {
        LiftHow::Skip => format!(
            "Gave up the failed firing of {}: it goes on with the readings after it.",
            sub.id
        ),
        LiftHow::Retry => format!(
            "Retried subscription {}: its failed firing is tried again on the next Tick, then the ones after it.",
            sub.id
        ),
    }
}

/// Lets the Chair of chain id read, and lift the hold of, its own chain's subscriptions only.
pub fn own_subscription_of(id: &str) -> Arc<May> {
    let owner = wait_owner(id);
    Arc::new(move |s: &Subscription| {
        if s.owner != owner {
            return Err(Error::refused(format!(
                "subscription {} is not this chain's: the Assistant reads and lifts it for the owner",
                s.id
            )));
        }
        Ok(())
    })
}

/// A subscription to an event signal as the Assistant reads it: in the shape update_subscription takes, plus where it
/// stands.
#[derive(Debug, Serialize)]
struct SubscriptionRecord {
    id: String,
    #[serde(flatten)]
    subscription: SubscriptionInput,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    paused: bool,
    /// Ships with Remit: never deleted, paused or changed; edited says its settings are the owner's version.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    builtin: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    edited: bool,
    /// RFC 3339 in the server's zone.
    #[serde(skip_serializing_if = "String::is_empty")]
    last_fired: String,
    /// Who made or last changed it: owner, assistant, agent or remit.
    by: String,
}

fn subscription_record_of(s: &Subscription, now: model::Time) -> SubscriptionRecord {
    let start = start_spec_of(s);
    SubscriptionRecord {
        id: s.id.clone(),
        subscription: SubscriptionInput {
            key: s.r#match.0.clone(),
            note: s.note.clone(),
            when: s.expr.clone(),
            screen: s.screen.clone(),
            action: s.action.kind.to_string(),
            with: s.action.with.clone(),
            job: s.action.job.clone(),
            title: start.title,
            goal: start.goal,
            kind: start.kind,
            params: serde_json::from_str(&s.action.params).ok(),
            flow: flow_input_of(&s.flow),
        },
        paused: s.control == Control::Paused,
        builtin: s.builtin,
        edited: s.edited,
        last_fired: s
            .last_fired
            .map(|at| rfc3339(model::Time(at.0.to_offset(now.0.offset()))))
            .unwrap_or_default(),
        by: s.by.kind.to_string(),
    }
}

/// The name of the event signal key names; none for a key that is no event signal's.
fn event_name(key: &SignalKey) -> Option<&str> {
    signal_key_of(key)
        .filter(|k| k.source == SignalSource::Event && !k.name.is_empty())
        .map(|k| k.name)
}

/// Says key is one these tools subscribe to: an event signal's, or a fact's.
fn heard_here(key: &SignalKey) -> bool {
    event_name(key).is_some() || fact_noun_of(key).is_some()
}

/// Says s is a subscription these tools read and write: to an event signal's events, or one the owner made to a fact (the
/// shipped ones to facts are the automation tools').
fn is_owner_subscription(s: &Subscription) -> bool {
    let key = SignalKey(s.r#match.0.clone());
    s.owner.kind == OwnerKind::Remit && (event_name(&key).is_some() || (fact_noun_of(&key).is_some() && !s.builtin))
}

fn no_owner_subscription(id: &str) -> Error {
    Error::refused(format!(
        "no subscription to an event or a fact {id:?}: list_subscriptions shows the ids"
    ))
}

impl SubscriptionInput {
    /// The subscription it says: what it listens to, when it holds, what it does and how its events flow. It has no id,
    /// author, time or progress of its own yet; the caller looks up the signal it names, and `subscribe` (or
    /// `checked_subscription`, for a change) checks it can be done.
    pub(crate) fn subscription(&self) -> Result<Subscription> {
        let key = SignalKey(self.key.trim().to_owned());
        if !heard_here(&key) {
            if key.0.is_empty() {
                return Err(Error::refused(
                    "a subscription needs a key: the event signal it listens to, event:<name>, or a fact, fact:<noun>.<verb>",
                ));
            }
            return Err(Error::refused(format!(
                "key {:?}: the owner subscribes to event signals (event:<name>) and Remit's facts (fact:<noun>.<verb>); a routine has its own tools",
                key.0
            )));
        }
        let mut sub = Subscription {
            owner: SubscriptionOwner {
                kind: OwnerKind::Remit,
                id: String::new(),
            },
            r#match: SignalPattern(key.0),
            note: first_line(self.note.trim(), 160),
            expr: self.when.trim().to_owned(),
            screen: self.screen.trim().to_owned(),
            ..Subscription::default()
        };
        if sub.note.is_empty() {
            return Err(Error::refused(
                "a subscription needs a note: what it does, in one line (Tell the Chair of the chain that its pipeline failed.)",
            ));
        }
        if !sub.expr.is_empty() {
            signal::check(&sub.expr).map_err(|e| Error::refused(format!("when: {e}")))?;
        }
        let mut action = SubscriptionAction {
            kind: self
                .action
                .trim()
                .parse()
                .map_err(|e: model::UnknownValue| Error::refused(format!("action: {e}")))?,
            with: self.with.clone(),
            job: self.job.trim().to_owned(),
            ..SubscriptionAction::default()
        };
        match action.kind {
            ActionKind::Tell | ActionKind::Run => {}
            ActionKind::Plugin(_) => action.params = params_of(self.params.as_ref())?,
            ActionKind::Start => {
                let start = StartSpec {
                    title: first_line(self.title.trim(), 80),
                    goal: self.goal.trim().to_owned(),
                    kind: self.kind,
                    origin: ChainOrigin::Event,
                    ..StartSpec::default()
                };
                if start.title.is_empty() {
                    return Err(Error::refused("a start needs a title"));
                }
                if start.goal.is_empty() {
                    return Err(Error::refused(
                        "a start needs a goal: what each chain is to do, in the owner's words",
                    ));
                }
                action.start = Some(start);
            }
            _ => {
                return Err(Error::refused(format!(
                    "action {:?}: a subscription tells a Chair (tell), starts a chain (start), runs a job (run) or runs a plugin's processor (plugin:<id>.<name>)",
                    self.action
                )));
            }
        }
        if !matches!(action.kind, ActionKind::Plugin(_)) && self.params.is_some() {
            return Err(Error::refused(format!(
                "a {} takes no params: only a plugin's processor does",
                action.kind
            )));
        }
        check_action(&action)?;
        (sub.action, sub.flow) = (action, flow_of(self.flow.as_ref())?);
        Ok(sub)
    }
}

/// The settings a plugin's processor is given, as its action stores them: a JSON object, or none.
fn params_of(params: Option<&serde_json::Value>) -> Result<String> {
    match params {
        None => Ok(String::new()),
        Some(v @ serde_json::Value::Object(_)) => Ok(serde_json::to_string(v)?),
        Some(_) => Err(Error::refused(
            "params: a JSON object of the settings the processor takes",
        )),
    }
}

/// The subscription in says as a change of one, refused at once, with the reason, when no processor could do it as it is
/// set.
fn checked_subscription(input: &SubscriptionInput) -> Result<Subscription> {
    let sub = input.subscription()?;
    check_subscription(&sub)?;
    Ok(sub)
}

/// Refuses a subscription whose event signal no one made, read in r: the transaction
/// that stores it, or the store itself for a dry run, which writes nothing.
fn refuse_missing(r: &store::Reader<'_>, s: &Subscription) -> Result<()> {
    // a family (event:*) names no one signal to be there, and a fact is Remit's, there whoever made nothing
    if let Some(key) = s.r#match.exact()
        && let Some(name) = event_name(&key)
    {
        match r.event_signal_named(name) {
            Err(store::Error::NotFound) => {
                return Err(Error::refused(format!(
                    "no event signal {name:?}: list_event_signals shows the ones there are"
                )));
            }
            other => {
                other?;
            }
        }
    }
    Ok(())
}

/// The start a tool's start says; none when it says none.
fn start_or_none(start: Option<&StartInput>) -> Result<Option<Start>> {
    start.map(StartInput::start).transpose()
}

impl Pipeline {
    /// Makes a subscription to an event signal from the owner's words, by who asked, judging from where start says: by
    /// default the events that come from now on, not the ones that came before it.
    async fn create_subscription(
        &self,
        input: &SubscriptionInput,
        start: Option<&StartInput>,
        by: Actor,
    ) -> Result<Subscription> {
        let mut sub = input.subscription()?;
        self.check_processor(&sub.action)?;
        self.check_start_kind(&sub)?;
        let from = start_or_none(start)?.unwrap_or(Start {
            from: StartFrom::Now,
            ..Start::default()
        });
        let now = self.now();
        (sub.id, sub.by, sub.created_at, sub.updated_at) = (new_id("sub_"), by, now, now);
        let sub = self
            .store
            .update(move |tx| {
                refuse_missing(tx, &sub)?;
                subscribe(tx, sub, &from, now)
            })
            .await?;
        self.kick();
        Ok(sub)
    }

    /// Replaces subscription id with input, keeping whether it is paused and, unless start names a new one, how far it has
    /// judged: what a held subscription waits to redo, it redoes as changed. One that ships becomes the owner's version
    /// (edited), which a start keeps, unless what he sets is what ships.
    async fn update_subscription(
        &self,
        id: &str,
        input: &SubscriptionInput,
        start: Option<&StartInput>,
        by: Actor,
    ) -> Result<Subscription> {
        let next = checked_subscription(input)?;
        self.check_processor(&next.action)?;
        self.check_start_kind(&next)?;
        let from = start_or_none(start)?;
        let shipped = self.shipped_subscriptions();
        let now = self.now();
        self.change_subscription(id, by, move |tx, mut cur| {
            (cur.r#match, cur.note, cur.expr, cur.screen, cur.action, cur.flow) =
                (next.r#match, next.note, next.expr, next.screen, next.action, next.flow);
            if cur.builtin {
                cur.edited = !shipped
                    .iter()
                    .any(|want| want.id == cur.id && subscription_ships_as(&cur, want));
            }
            if let Some(from) = &from {
                cur.seen_seq = start_seq(tx, &cur, from, now)?;
            }
            refuse_missing(tx, &cur)?;
            Ok(cur)
        })
        .await
    }

    /// The dry run of subscription id as input would change it, checked as update_subscription checks it and not stored.
    async fn dry_run_update(
        &self,
        id: &str,
        input: &SubscriptionInput,
        start: Option<&StartInput>,
    ) -> Result<DryRunRead> {
        let next = checked_subscription(input)?;
        self.check_processor(&next.action)?;
        self.check_start_kind(&next)?;
        let key = id.to_owned();
        let mut cur = match self.store.read(move |r| r.subscription(&key)).await {
            Ok(cur) if is_owner_subscription(&cur) => cur,
            Ok(_) | Err(store::Error::NotFound) => return Err(no_owner_subscription(id)),
            Err(err) => return Err(err.into()),
        };
        (cur.r#match, cur.note, cur.expr, cur.screen, cur.action, cur.flow) =
            (next.r#match, next.note, next.expr, next.screen, next.action, next.flow);
        let checked = cur.clone();
        self.store.read(move |r| refuse_missing(r, &checked)).await?;
        let mut d = self.dry_run(cur, start_or_none(start)?).await?;
        id.clone_into(&mut d.subscription);
        Ok(d)
    }

    /// The dry run of the subscription input says, checked as create_subscription checks it and not made.
    pub(crate) async fn dry_run_draft(
        &self,
        input: &SubscriptionInput,
        start: Option<&StartInput>,
    ) -> Result<DryRunRead> {
        let sub = checked_subscription(input)?;
        self.check_processor(&sub.action)?;
        self.check_start_kind(&sub)?;
        let checked = sub.clone();
        self.store.read(move |r| refuse_missing(r, &checked)).await?;
        self.dry_run(sub, start_or_none(start)?).await
    }

    /// Stops a subscription from hearing events until it resumes, or lets it hear them again. The events that came while
    /// it was paused are not heard: it answers what comes after.
    async fn set_subscription_paused(&self, id: &str, paused: bool, by: Actor) -> Result<Subscription> {
        let now = self.now();
        self.change_subscription(id, by, move |tx, mut cur| {
            cur.control = Control::Paused;
            if !paused {
                cur.control = Control::Active;
                let head = start_seq(
                    tx,
                    &cur,
                    &Start {
                        from: StartFrom::Now,
                        ..Start::default()
                    },
                    now,
                )?;
                cur.seen_seq = cur.seen_seq.max(head);
            }
            Ok(cur)
        })
        .await
    }

    /// Stores what change makes of the subscription to an event id, read and written in one transaction so no change is
    /// lost, as by, and shows it on the Subscriptions tab.
    async fn change_subscription<F>(&self, id: &str, by: Actor, change: F) -> Result<Subscription>
    where
        F: FnOnce(&store::Tx<'_>, Subscription) -> Result<Subscription> + Send + 'static,
    {
        let (key, now) = (id.to_owned(), self.now());
        let saved = self
            .store
            .update(move |tx| {
                let cur = match tx.subscription(&key) {
                    Ok(cur) if is_owner_subscription(&cur) => cur,
                    Ok(_) | Err(store::Error::NotFound) => return Err(no_owner_subscription(&key)),
                    Err(err) => return Err(err.into()),
                };
                let mut next = change(tx, cur)?;
                (next.by, next.updated_at) = (by, now);
                Ok(tx.put_subscription(next)?)
            })
            .await?;
        self.kick();
        Ok(saved)
    }

    /// Removes a subscription the owner made. One that ships is paused, never deleted: a start would make it again.
    async fn delete_subscription(&self, id: &str) -> Result<()> {
        let key = id.to_owned();
        let cur = match self.store.read(move |r| r.subscription(&key)).await {
            Ok(cur) if is_owner_subscription(&cur) => cur,
            Ok(_) | Err(store::Error::NotFound) => return Err(no_owner_subscription(id)),
            Err(err) => return Err(err.into()),
        };
        if cur.builtin {
            return Err(Error::refused(format!(
                "the subscription {id} ships with Remit and is never deleted: pause_subscription stops it"
            )));
        }
        let key = id.to_owned();
        self.store.update(move |tx| tx.delete_subscription(&key)).await?;
        Ok(())
    }

    /// Gives the Assistant, acting as by, the tools that make, read, change, pause and delete the subscriptions to event
    /// signals, lift any subscription's hold, and read where one stands.
    pub fn add_subscription_tools(self: &Arc<Self>, t: &mut Tools, by: &Actor) {
        t.serving(self).add(
            "list_subscriptions", Effect::Reads,
            "The subscriptions to event signals' events, and the owner's to facts, each in the shape update_subscription takes: the key it listens to, its note, its `when`, its action with what it reads of the reading (params for a plugin's processor), whether it is paused, whether it ships with Remit (builtin: GitHub's and GitLab's rules; edited says the settings are the owner's version) and when it last fired. What an event caused is in list_events.",
            |o, _: NoInput| async move {
                let all = o.store.read(|r| r.subscriptions()).await?;
                let now = o.now();
                let recs: Vec<SubscriptionRecord> =
                    all.iter().filter(|s| is_owner_subscription(s)).map(|s| subscription_record_of(s, now)).collect();
                as_json(&recs)
            },
        );
        let create_by = by.clone();
        t.serving(self).add(
            "create_subscription", Effect::Writes,
            "Make a subscription for the owner: for each event of an event signal, or each reading of a fact Remit records (fact:turn.ended: a card's turn ended, with its reply), it holds for, Remit tells the Chair of the chain it is about, starts a chain from it, runs a job on the chain it is about, or runs an enabled plugin's processor on it (list_processors). One subscription does one thing: for two things he wants, make two. It hears the events that come after it, not the ones before, unless start names an earlier place in the log or time (the owner asked for the events since then to be answered too). It shows on the Subscriptions tab at once. Make one only when the owner asked for it. Read a real event with list_events and read_event_fields before you write a `when`: never guess a sender's payload, and send it first with dry_run: true to see what it would have done over the events kept. An event with no subscription is stored and does nothing.",
            move |o, i: CreateSubscriptionInput| {
                let by = create_by.clone();
                async move {
                    if i.dry_run {
                        return as_json(&o.dry_run_draft(&i.subscription, i.start.as_ref()).await?);
                    }
                    let sub = o.create_subscription(&i.subscription, i.start.as_ref(), by).await?;
                    Ok(format!("Made subscription {} to {}: {}.", sub.id, sub.r#match.0, sub.action.kind))
                }
            },
        );
        let update_by = by.clone();
        t.serving(self).add(
            "update_subscription", Effect::Writes,
            "Replace a subscription with this version: what it listens to, its `when` and its action. Every field is replaced, so start from list_subscriptions and keep what the owner did not ask to change. It goes on from where it stands, unless start names where to judge from. A paused subscription stays paused. One that ships with Remit becomes the owner's version and keeps it through a restart. dry_run: true changes nothing and answers what this version would have done over the events kept.",
            move |o, i: UpdateSubscriptionInput| {
                let by = update_by.clone();
                async move {
                    if i.dry_run {
                        return as_json(&o.dry_run_update(&i.id, &i.subscription, i.start.as_ref()).await?);
                    }
                    let sub = o.update_subscription(&i.id, &i.subscription, i.start.as_ref(), by).await?;
                    Ok(format!("Updated subscription {} to {}: {}.", sub.id, sub.r#match.0, sub.action.kind))
                }
            },
        );
        let pause_by = by.clone();
        t.serving(self).add(
            "pause_subscription", Effect::Writes,
            "Pause a subscription to events for the owner: it hears no event until it resumes, and the events that come meanwhile are not heard after. This is how one that ships with Remit is stopped.",
            move |o, i: SubscriptionIdInput| {
                let by = pause_by.clone();
                async move {
                    let sub = o.set_subscription_paused(&i.id, true, by).await?;
                    Ok(format!("Paused subscription {}.", sub.id))
                }
            },
        );
        t.serving(self).add(
            "delete_subscription", Effect::Writes,
            "Delete a subscription to events the owner named: it hears no event again. Its firings stay. One that ships with Remit is never deleted: pause_subscription stops it.",
            |o, i: SubscriptionIdInput| async move {
                o.delete_subscription(&i.id).await?;
                Ok(format!("Deleted subscription {}.", i.id))
            },
        );
        self.add_processor_tool(t);
        self.add_hold_tools(t, by);
        self.add_inspect_tools(t, Arc::new(any_subscription));
        self.add_draft_dry_run_tool(t);
    }

    /// Gives the Assistant, acting as by, what lifts the hold of any subscription for the owner, by retrying its failed
    /// firing or giving it up, and what resumes a paused subscription to events.
    fn add_hold_tools(self: &Arc<Self>, t: &mut Tools, by: &Actor) {
        let resume_by = by.clone();
        t.serving(self).add(
            "resume_subscription", Effect::Writes,
            "Resume a subscription for the owner. A held one (the Subscriptions tab and read_signals say so: a firing of it failed for good, and it waits in place) is tried again from that firing, then the ones after it: fix what failed first, and say what you fixed in why. Any subscription can be held, a routine's, a chain's or one that ships. A paused subscription to events hears the events that come from now on.",
            move |o, i: LiftInput| {
                let by = resume_by.clone();
                async move {
                    let key = i.id.clone();
                    let held = o.store.read(move |r| r.subscription(&key)).await.is_ok_and(|cur| cur.hold.is_some());
                    if held {
                        let sub = o.lift_hold(&i.id, LiftHow::Retry, &i.why, by, &any_subscription).await?;
                        return Ok(lifted_words(&sub, LiftHow::Retry));
                    }
                    let sub = o.set_subscription_paused(&i.id, false, by).await?;
                    Ok(format!("Resumed subscription {}.", sub.id))
                }
            },
        );
        let skip_by = by.clone();
        t.serving(self).add(
            "skip_firing", Effect::Writes,
            "Give up the failed firing a held subscription waits on, for the owner: nothing is done for its readings, and the subscription goes on with the ones after it. Use it when the firing can never succeed or is no longer wanted; resume_subscription tries it again instead. Say why: it is recorded.",
            move |o, i: LiftInput| {
                let by = skip_by.clone();
                async move {
                    let sub = o.lift_hold(&i.id, LiftHow::Skip, &i.why, by, &any_subscription).await?;
                    Ok(lifted_words(&sub, LiftHow::Skip))
                }
            },
        );
        let redo_by = by.clone();
        t.serving(self).add(
            "redo_firing", Effect::Writes,
            "Do again the firing a subscription's hold gave up (skip_firing, or a skip answered to its held item), for the owner: when he overturns the skip, or what failed is fixed after all. The newest given-up firing of it is tried again at once, before its later ones; nothing else of the subscription changes. Say why: it is recorded.",
            move |o, i: LiftInput| {
                let by = redo_by.clone();
                async move {
                    let f = o.redo_firing(&i.id, &i.why, by, &any_subscription).await?;
                    Ok(format!(
                        "Doing again the given-up firing of {} on {} (rev {}): it is tried on the next Tick.",
                        i.id, f.key.0, f.rev
                    ))
                }
            },
        );
    }
}

impl Pipeline {
    /// Gives the Chair, card me, what it waits and watches with: subscribe, the lift of its own chain's held
    /// subscriptions, and create_sensor for a state no signal gives.
    pub fn add_watch_tools(self: &Arc<Self>, t: &mut chains::tools::Tools, me: &chains::tools::SelfCard) {
        let subscribe_me = me.clone();
        t.serving(self).add(
            "subscribe", Effect::Writes,
            "Subscribe this chain to a time (at) or to a signal (key, with until): you come back to it then. For results that can only be seen later (a deploy, data that has to accumulate, a job that runs for hours) and for effects a signal shows (an error rate after a deploy). The chain waits without using a slot, and its sessions close, and with them anything the agents run in the background; your own session resumes with a message saying why. A time wakes you at that time. A signal needs the signals capability: a reading that meets the condition wakes you with the reading as evidence, and the deadline until wakes you if none does; when no signal gives the state you need to see (a deploy's status, a queue's depth, a page's content), make a sensor first (create_sensor) and watch its key chain:<chain id>.sensor.<name>; the watch stays open through messages and other check-backs, and finish_chain is refused while it is open. Every other agent must be idle. Run `date -Iseconds` for the current time.",
            move |o, i: chains::watch::SubscribeInput| {
                let me = subscribe_me.clone();
                async move {
                    let card = me.read(&o).await?;
                    let now = o.now();
                    let at = o.subscribe(&card, &i, now).await?;
                    Ok(chains::watch::subscribed_words(&i.key, at, now))
                }
            },
        );
        for (name, how, description) in [
            (
                "resume_subscription",
                crate::record::LiftHow::Retry,
                "Try again the failed firing a subscription of this chain is held on (fact:subscription.failed names it, with the error), then the ones after it: fix what failed first, and say what you fixed in why.",
            ),
            (
                "skip_firing",
                crate::record::LiftHow::Skip,
                "Give up the failed firing a subscription of this chain is held on: nothing is done for its readings, and it goes on with the ones after them. Say why: it is recorded.",
            ),
        ] {
            let lift_me = me.clone();
            t.serving(self).add(
                name,
                chains::tools::Effect::Writes,
                description,
                move |o, i: crate::subscription_tools::LiftInput| {
                    let me = lift_me.clone();
                    async move {
                        let card = me.read(&o).await?;
                        let own = crate::subscription_tools::own_subscription_of(&card.home);
                        let sub = o
                            .lift_hold(&i.id, how, &i.why, model::by_agent(&card.id), &*own)
                            .await?;
                        Ok(crate::subscription_tools::lifted_words(&sub, how))
                    }
                },
            );
        }
        self.add_chain_sensor_tool(t, me);
    }
}
