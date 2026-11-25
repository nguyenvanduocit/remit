//! The Assistant's digest: what happened over the last day or the last week (the chains that finished and how each ended,
//! what the Assistant decided for the owner, what he overturned), posted in the Assistant's panel at the time he told it.
//! It is built from what the store holds by pure code (`digest_text`), with no model call, so it says only what is
//! recorded. A digest is a subscription of Remit that keeps the schedule he gave and runs the job of its period
//! (ADR-0035); he never fills in a form for it: he tells the Assistant, whose tools (digest_tools.rs) are the actions here.

use std::collections::BTreeSet;

use model::{
    ActionKind, Actor, Decision, DecisionKind, Event, EventStatus, OwnerKind, Schedule, Start, StartFrom, Subscription,
    SubscriptionAction, SubscriptionOwner, Time,
};
use time::macros::format_description;

use crate::orch::Orchestrator;
use chains::apply::Records;
use chains::chain_core::first_line;
use chains::subscribe::subscribe;
use chains::timewait::counted_from;
use chains::{Error, Result};
use pipeline::automation::DigestPeriod;
use pipeline::processing::Delivery;
use wire::app::ChainTone;

/// The fastest a digest may repeat: each covers a whole day or week, so one posted more often repeats itself, and a cron
/// typo such as `* * * * *` would fill the panel.
const MIN_DIGEST_EVERY: time::Duration = time::Duration::HOUR;

/// How many lines a digest lists under each heading; the rest are counted.
const DIGEST_LINES: usize = 12;

/// A chain that finished within a digest's window, and how it ended as the board says it (`standing::outcome_of`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Ended {
    pub id: String,
    pub title: String,
    pub tone: ChainTone,
    /// The board's words for how it ended ("Done", "Not done · …", "Waiting for …").
    pub text: String,
}

/// What a digest is written from: its period and window, the chains that finished in it, and the Assistant's decisions
/// for the owner and his overturns made in it, oldest first.
pub(crate) struct DigestInput<'a> {
    pub period: DigestPeriod,
    pub from: Time,
    pub ended: &'a [Ended],
    pub decided: &'a [Decision],
    pub overturned: &'a [Decision],
}

/// A time as the digest's heading says it, in the server's time zone.
fn when(t: Time) -> String {
    t.0.format(format_description!("[year]-[month]-[day] [hour]:[minute]"))
        .unwrap_or_default()
}

/// Lists lines under a heading, at most `DIGEST_LINES`, counting the rest.
fn list(b: &mut String, lines: impl ExactSizeIterator<Item = String>) {
    let more = lines.len().saturating_sub(DIGEST_LINES);
    for l in lines.take(DIGEST_LINES) {
        b.push_str(&format!("- {l}\n"));
    }
    if more > 0 {
        b.push_str(&format!("- and {more} more\n"));
    }
}

/// The digest, in Markdown: the chains that finished (done, waiting to ship, not done), what the Assistant decided for
/// the owner, and what he overturned. Pure.
pub(crate) fn digest_text(i: &DigestInput<'_>) -> String {
    let mut b = format!("**Digest · {}** (since {})\n\n", i.period.label(), when(i.from));
    if i.ended.is_empty() && i.decided.is_empty() && i.overturned.is_empty() {
        b.push_str("No chain finished, and nothing was decided for you.\n");
        return b;
    }
    let count = |tone: ChainTone| i.ended.iter().filter(|e| e.tone == tone).count();
    let (done, shipping) = (count(ChainTone::Done), count(ChainTone::Waiting));
    let not_done = i.ended.len().saturating_sub(done).saturating_sub(shipping);
    b.push_str(&format!(
        "**Finished: {}** · done {done} · waiting to ship {shipping} · not done {not_done}\n",
        i.ended.len()
    ));
    // what did not end done comes first: it is what the owner may want to look at
    let mut ended: Vec<&Ended> = i.ended.iter().collect();
    ended.sort_by_key(|e| match e.tone {
        ChainTone::Done => 2,
        ChainTone::Waiting => 1,
        _ => 0,
    });
    list(
        &mut b,
        ended
            .iter()
            .map(|e| format!("{}: {} ({})", e.text, first_line(&e.title, 120), e.id)),
    );
    b.push_str(&format!("\n**Decided for you: {}**\n", i.decided.len()));
    list(&mut b, i.decided.iter().map(|d| first_line(&d.text, 200)));
    b.push_str(&format!("\n**Overturned by you: {}**\n", i.overturned.len()));
    list(&mut b, i.overturned.iter().map(|d| first_line(&d.text, 200)));
    b
}

/// The decisions that end a chain's run (seen.rs `ENDINGS`).
const ENDINGS: [DecisionKind; 3] = [DecisionKind::Finish, DecisionKind::Stall, DecisionKind::GoalFailed];

/// Who posts a digest: Remit, on the Assistant's card.
fn digest_by() -> Actor {
    model::by_remit("digest")
}

/// The subscription that posts the digest of period on schedule cron, by who asked, its first instant counted from now.
fn digest_sub(period: DigestPeriod, cron: &str, by: Actor, now: Time) -> Result<Subscription> {
    let cron = cron.trim();
    if cron.is_empty() {
        return Err(Error::refused(
            "cron is empty: say when to post it, e.g. '0 18 * * *' for every day at 18:00",
        ));
    }
    let sub = counted_from(
        Subscription {
            id: period.sub_id().to_owned(),
            owner: SubscriptionOwner {
                kind: OwnerKind::Remit,
                id: String::new(),
            },
            note: format!("Post the digest of {} in the Assistant's panel", period.label()),
            schedule: Some(Schedule {
                cron: cron.to_owned(),
                ..Schedule::default()
            }),
            action: SubscriptionAction {
                kind: ActionKind::Run,
                job: period.job().to_owned(),
                ..SubscriptionAction::default()
            },
            by,
            created_at: now,
            updated_at: now,
            ..Subscription::default()
        },
        now,
    )?;
    // counted_from gives the first instant: the second is counted from it
    let first = sub.due_at.unwrap_or(now);
    match signal::cron_next(cron, first) {
        Ok(second)
            if first
                .0
                .checked_add(MIN_DIGEST_EVERY)
                .is_some_and(|soonest| second.0 >= soonest) =>
        {
            Ok(sub)
        }
        _ => Err(Error::refused(format!(
            "cron {cron:?} repeats more often than once an hour; a digest covers a whole {}",
            match period {
                DigestPeriod::Day => "day",
                DigestPeriod::Week => "week",
            }
        ))),
    }
}

impl Orchestrator {
    /// The digests the owner gets, by period.
    pub(crate) async fn digests(&self) -> Result<Vec<(DigestPeriod, Subscription)>> {
        let mut out = Vec::new();
        for p in DigestPeriod::ALL {
            match self.store.read(move |r| r.subscription(p.sub_id())).await {
                Ok(s) => out.push((p, s)),
                Err(store::Error::NotFound) => {}
                Err(err) => return Err(err.into()),
            }
        }
        Ok(out)
    }

    /// Posts the digest of period at the times cron gives, by who asked: a digest of that period already set takes the new
    /// time.
    pub(crate) async fn set_digest(&self, period: DigestPeriod, cron: &str, by: Actor) -> Result<Subscription> {
        let now = self.now();
        let mut sub = digest_sub(period, cron, by, now)?;
        let saved = self
            .store
            .update(move |tx| {
                match tx.subscription(&sub.id) {
                    Ok(cur) => sub.created_at = cur.created_at,
                    Err(store::Error::NotFound) => {}
                    Err(err) => return Err(err.into()),
                }
                subscribe(
                    tx,
                    sub,
                    &Start {
                        from: StartFrom::Now,
                        ..Start::default()
                    },
                    now,
                )
            })
            .await?;
        self.kick();
        Ok(saved)
    }

    /// Stops the digest of period.
    pub(crate) async fn stop_digest(&self, period: DigestPeriod) -> Result<()> {
        self.store
            .update(move |tx| {
                match tx.subscription(period.sub_id()) {
                    Ok(_) => {}
                    Err(store::Error::NotFound) => {
                        return Err(Error::refused(format!(
                            "no digest of {} is set: list_digests shows the ones that are",
                            period.label()
                        )));
                    }
                    Err(err) => return Err(err.into()),
                }
                Ok::<_, Error>(tx.delete_subscription(period.sub_id())?)
            })
            .await?;
        self.kick();
        Ok(())
    }

    /// The effect of a digest's firing: the digest of period, read from the store as it stands at the firing, posted on
    /// the Assistant's card in the write that stores the firing done, so it is posted once.
    pub(crate) async fn fire_digest(&self, d: &Delivery, period: DigestPeriod) -> Result<()> {
        let to = d.now;
        let from = to.0.checked_sub(period.span()).map_or_else(Time::default, Time);
        let (ended_chains, decided, overturned) = self
            .store
            .read(move |r| {
                let mut ids = BTreeSet::new();
                for kind in &ENDINGS {
                    ids.extend(r.decisions_between(kind, from, to)?.into_iter().map(|d| d.chain_id));
                }
                let mut chains = Vec::new();
                for id in ids {
                    match r.chain(&id) {
                        // one reopened since is not finished
                        Ok(c) if *c.status() == model::ChainStatus::Done => chains.push(c),
                        Ok(_) | Err(store::Error::NotFound) => {}
                        Err(err) => return Err(err),
                    }
                }
                let decided = r.decisions_between(&DecisionKind::Assist, from, to)?;
                let overturned = r.decisions_between(&DecisionKind::Overturn, from, to)?;
                Ok::<_, store::Error>((chains, decided, overturned))
            })
            .await?;
        let ids: Vec<String> = ended_chains.iter().map(|c| c.id.clone()).collect();
        let standings = self.standings_of(&ids).await;
        let ended: Vec<Ended> = ended_chains
            .into_iter()
            .map(|c| {
                let st = standings.iter().find(|s| s.chain_id == c.id);
                Ended {
                    tone: st.map_or(ChainTone::NotDone, |s| s.tone),
                    text: st.map_or_else(|| "Finished".to_owned(), |s| s.text.clone()),
                    id: c.id,
                    title: c.title,
                }
            })
            .collect();
        let text = digest_text(&DigestInput {
            period,
            from,
            ended: &ended,
            decided: &decided,
            overturned: &overturned,
        });
        let to_card = self.assistant_card().await?;
        let now = self.now();
        self.change_home(Records {
            events: vec![Event {
                home: to_card.home.clone(),
                card_id: to_card.id.clone(),
                status: EventStatus::Message,
                title: format!("Digest of {}", period.label()),
                text,
                by: Some(digest_by()),
                created_at: now,
                updated_at: now,
                ..Event::default()
            }],
            fired: Some(d.done(&format!("posted the digest of {}", period.label()), "", "")),
            ..Records::default()
        })
        .await
        .map(|_| ())
    }
}
