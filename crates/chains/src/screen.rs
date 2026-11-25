//! Whether a reading deserves a chain (ADR-0118, point 2). A subscription that starts a chain may carry a `screen`: when a
//! reading deserves one, in the words the Assistant wrote. After the signal core says which firings the subscription
//! makes, the reading each is on is put to a decider under that screen (`signal.screen@1`, the screen as what makes the
//! answer yes). Only a no the decider is sure enough to act on drops a firing; an unsure no, a yes, a refusal, a late
//! answer, no decider and an event signal kept on this machine (`EventSignal::kept`) leave it to fire as the
//! subscription always did. A twentieth of the firings dropped fire anyway (`AUDIT_PERCENT`), which is how a decider's
//! nos get outcomes. What the chain a firing started did proves the pick: it hired a worker (the pick was yes), or it
//! finished having hired none (the pick was no). The state that leaves is the reading's key, subject and value, its
//! JSON body read as JSON so that credential fields are removed, then redacted as a source's readings are
//! (`Core::judge`). One `DECIDE_FOR` bounds the asking of a whole pass of the Tick. Pure: `question_of`, `verdict_of`,
//! `state_of`, `worked`, `proved_by`.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use judgment::{Invalid, Kind, Question, SIGNAL_SCREEN};
use model::{
    CardRole, Chain, ChainStatus, Judgment, JudgmentOutcome, JudgmentPick, JudgmentSubject, JudgmentUse, OutcomeSource,
    Proved, Reading, SubjectKind, Subscription, Time, Unjudged,
};
use serde_json::{Value, json};
use sha2::Digest;
use signal::{Screen, Verdict};
use store::Tx;
use tokio::task::JoinSet;

use crate::chain_core::Core;
use crate::judge::{Judged, sampled};
use crate::signal_sources::event_signal_of;

/// The share of firings screened out, in percent, that fire anyway. A decider's no that acts is never checked by what
/// follows, so without them its precision is never measured and it could not be trusted to drop more; a twentieth bounds
/// the cost of finding out, as `hire_choice::EXPLORE_PERCENT` does for a hire.
pub const AUDIT_PERCENT: u64 = 5;

/// The most firings of one subscription put to a decider in one pass of the Tick: the firings after them wait for the
/// next pass unjudged (`Verdict::Defer`), so a burst is screened and never fires unscreened.
pub const SCREENED_PER_PASS: usize = 20;

/// How long a pass of the Tick waits for the deciders in all: the one deadline of every ask of the pass. A decider call
/// is bounded by `DECIDE_FOR` itself; the rest is for the store around it.
pub const SCREEN_WITHIN: Duration = Duration::from_secs(3);

/// What makes the answer no, as the decider reads it: the screen says what makes it yes.
const NO_WORDS: &str = "The reading does not meet that: no chain is needed for it.";

/// What the decider is asked of one reading and what it makes of it.
struct Seen {
    judgment: String,
    p: f64,
    /// Its answer is one the code acts on: a no, sure enough.
    acts: bool,
}

/// The question a subscription's screen asks of each reading: `signal.screen@1`, with the screen as what makes yes.
pub fn question_of(screen: &str) -> std::result::Result<Question, Invalid> {
    Question::new(&SIGNAL_SCREEN, Kind::noul(screen.trim(), NO_WORDS)?)
}

/// What an answer makes of the firing on reading `seq`. audited: the firing is in the sample that fires anyway. Pure.
fn verdict_of(seq: i64, seen: &Seen, audited: bool) -> Screen {
    let verdict = if seen.acts && !audited {
        Verdict::Out
    } else {
        Verdict::Keep
    };
    Screen {
        seq,
        verdict,
        judgment: seen.judgment.clone(),
        p: seen.p,
    }
}

/// The answer a stored judgment gave, as it would be acted on: a screen asked the same reading twice (a pass that
/// deferred it, a firing that did not record) reads the first answer instead of asking again. Its use is stored as
/// escalated until a drop is recorded, so what it would do is read from its threshold. Pure.
fn seen_before(j: &Judgment) -> Seen {
    Seen {
        judgment: j.id.clone(),
        p: j.p,
        acts: j.pick == JudgmentPick::No && j.threshold.is_some_and(|t| j.p >= t),
    }
}

/// The reading of subscription `sub`, which `seq` is the place of in the log, as `screen` asks about it: what a
/// judgment is about. The words of the screen are part of it, so a screen the Assistant corrected asks again about a
/// reading judged under the old words, and a decider's answer to one is never read as the answer to the other.
pub fn subject_of(sub: &str, seq: i64, screen: &str) -> JudgmentSubject {
    let words: String = sha2::Sha256::digest(screen.trim().as_bytes())
        .iter()
        .take(6)
        .map(|b| format!("{b:02x}"))
        .collect();
    JudgmentSubject {
        kind: SubjectKind::Reading,
        id: format!("{sub}:{seq}:{words}"),
        home: String::new(),
    }
}

/// What a decider is shown of a reading. An event's body is JSON text: it is read as JSON, so that the fields named as
/// credentials are removed from it (`source::redact_value`) and not only the tokens that look like one.
fn state_of(r: &Reading) -> Value {
    let mut value = r.value.as_ref().map_or(Value::Null, |v| v.0.clone());
    if let Some(body) = value.pointer_mut("/event/body")
        && let Some(parsed) = body
            .as_str()
            .and_then(|text| serde_json::from_str::<Value>(text).ok())
            .filter(|v| v.is_object() || v.is_array())
    {
        *body = parsed;
    }
    json!({"signal": r.key.as_str(), "subject": r.subject, "reading": value})
}

/// Whether a finished chain did work: it hired a worker besides its Chair (hired counts them). None when it ended before
/// it could say (the owner ended it, it stalled, its goal check could not run). Pure.
fn worked(chain: &Chain, hired: usize) -> Option<bool> {
    match chain.unjudged {
        Some(Unjudged::Owner | Unjudged::Stalled | Unjudged::Failed) => None,
        _ => Some(hired > 0),
    }
}

/// What a pick proved, given whether the chain worked: yes said it would, no said it would not. Pure.
fn proved_by(pick: &JudgmentPick, worked: Option<bool>) -> Proved {
    match worked {
        None => Proved::Unknown,
        Some(w) if w == (*pick == JudgmentPick::Yes) => Proved::Right,
        Some(_) => Proved::Wrong,
    }
}

/// The firings of one subscription a screen is asked about: the reading each is on, in the order they fire.
pub struct Asks {
    pub sub: Subscription,
    pub readings: Vec<Reading>,
}

impl Core {
    /// What each subscription's screen makes of the firings in asks, by subscription id: every answer within
    /// `SCREEN_WITHIN`, so a pass of the Tick waits for the deciders once however many subscriptions screen. A firing
    /// no decider answered in time has no screen, and fires as the subscription does with none; one past
    /// `SCREENED_PER_PASS` of its subscription is deferred. Nothing for a subscription whose screen cannot be asked, and
    /// for a reading of an event signal kept on this machine.
    pub async fn screen_firings(&self, asks: Vec<Asks>) -> HashMap<String, Vec<Screen>> {
        let mut out: HashMap<String, Vec<Screen>> = HashMap::new();
        let Some(me) = self.me() else { return out };
        let Some(kept) = self.kept_signals().await else {
            return out;
        };
        let mut asking: JoinSet<Option<(String, Screen)>> = JoinSet::new();
        for Asks { sub, readings } in asks {
            let question = match question_of(&sub.screen) {
                Ok(q) => q,
                Err(err) => {
                    tracing::warn!(subscription = %sub.id, %err, "screen: a question that cannot be asked");
                    continue;
                }
            };
            // the events of a signal kept on this machine are not asked about and take no place among those that are
            let mut asked = 0usize;
            for r in readings {
                if event_signal_of(&r.key).is_some_and(|name| kept.contains(name)) {
                    continue;
                }
                if asked >= SCREENED_PER_PASS {
                    out.entry(sub.id.clone()).or_default().push(Screen {
                        seq: r.seq,
                        verdict: Verdict::Defer,
                        judgment: String::new(),
                        p: 0.0,
                    });
                    continue;
                }
                asked = asked.saturating_add(1);
                let subject = subject_of(&sub.id, r.seq, &sub.screen);
                let (core, question, id) = (Arc::clone(&me), question.clone(), sub.id.clone());
                let audited = sampled(&subject.id, AUDIT_PERCENT);
                asking.spawn(async move {
                    let seen = core.seen(subject, &r, question).await?;
                    Some((id, verdict_of(r.seq, &seen, audited)))
                });
            }
        }
        let collect = async {
            while let Some(done) = asking.join_next().await {
                match done {
                    Ok(Some((id, screen))) => out.entry(id).or_default().push(screen),
                    Ok(None) => {}
                    Err(err) => tracing::warn!(%err, "screen: a firing's judgment ended"),
                }
            }
        };
        if tokio::time::timeout(SCREEN_WITHIN, collect).await.is_err() {
            // a judgment that came too late is stored, and the next pass that asks the same reading reads it (`seen`)
            tracing::warn!("screen: the firings were not all judged in time; the rest fire");
        }
        out
    }

    /// The names of the event signals whose events stay on this machine; none when they could not be read, and then no
    /// event is sent: every firing goes on unscreened.
    async fn kept_signals(&self) -> Option<HashSet<String>> {
        match self.store.read(|r| r.event_signals()).await {
            Ok(all) => Some(all.into_iter().filter(|es| es.kept).map(|es| es.name).collect()),
            Err(err) => {
                tracing::warn!(%err, "screen: the event signals could not be read; nothing is screened");
                None
            }
        }
    }

    /// What a decider says of reading r under question: the answer already given about it, or a new one.
    async fn seen(&self, subject: JudgmentSubject, r: &Reading, question: Question) -> Option<Seen> {
        let (key, id) = (question.key(), subject.id.clone());
        match self.store.read(move |s| s.reading_judgment(&key, &id)).await {
            Ok(Some(before)) => return Some(seen_before(&before)),
            Ok(None) => {}
            Err(err) => tracing::warn!(%err, "screen: an earlier judgment could not be read"),
        }
        let judged: Judged = self
            .judge(subject, &state_of(r), vec![question])
            .await
            .pop()
            .flatten()?;
        Some(Seen {
            judgment: judged.judgment,
            acts: judged.pick == JudgmentPick::No && judged.used == JudgmentUse::Acted,
            p: judged.p,
        })
    }
}

/// Records in tx what the chain a screened firing started proved of the screen's pick, as the commit that finishes the
/// chain stores it: called on the change to done only, so a chain saved again does not score again, and recorded again
/// when the chain is reopened and finished, it replaces the one before. What cannot be recorded is logged: a score is
/// never worth a chain's finish.
pub(crate) fn finished_start(tx: &Tx<'_>, chain: &Chain, at: Time) {
    if chain.routine_id.is_empty() || *chain.status() != ChainStatus::Done {
        return;
    }
    if let Err(err) = score(tx, chain, at) {
        tracing::warn!(chain = %chain.id, %err, "screen: what the chain proved could not be recorded");
    }
}

/// Scores the judgment the firing that started `chain` carries (`Firing::screen`), if it carries one.
fn score(tx: &Tx<'_>, chain: &Chain, at: Time) -> store::Result<()> {
    let Some(screen) = tx.screen_of_start(&chain.routine_id, &chain.id)? else {
        return Ok(());
    };
    let judgment = match tx.judgment(&screen) {
        Ok(j) => j,
        Err(store::Error::NotFound) => return Ok(()),
        Err(err) => return Err(err),
    };
    let hired = tx
        .cards(&chain.id)?
        .iter()
        .filter(|k| CardRole::of(k) == CardRole::Worker)
        .count();
    let outcome = JudgmentOutcome {
        proved: proved_by(&judgment.pick, worked(chain, hired)),
        source: OutcomeSource::Chain,
        at,
    };
    match tx.set_judgment_outcome(&screen, &outcome) {
        Err(store::Error::NotFound) => Ok(()),
        done => done,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seen(acts: bool) -> Seen {
        Seen {
            judgment: "ju_1".to_owned(),
            p: 0.95,
            acts,
        }
    }

    #[test]
    fn only_a_no_that_acts_and_is_not_audited_drops_a_firing() {
        let verdict = |acts, audited| verdict_of(1, &seen(acts), audited).verdict;
        assert_eq!(verdict(true, false), Verdict::Out);
        assert_eq!(verdict(true, true), Verdict::Keep, "an audited no fires");
        assert_eq!(verdict(false, false), Verdict::Keep, "an unsure no fires");
    }

    #[test]
    fn a_pick_is_right_when_the_chain_did_what_it_said() {
        assert_eq!(proved_by(&JudgmentPick::Yes, Some(true)), Proved::Right);
        assert_eq!(proved_by(&JudgmentPick::Yes, Some(false)), Proved::Wrong);
        assert_eq!(proved_by(&JudgmentPick::No, Some(true)), Proved::Wrong);
        assert_eq!(proved_by(&JudgmentPick::No, Some(false)), Proved::Right);
        assert_eq!(proved_by(&JudgmentPick::Yes, None), Proved::Unknown);
    }

    #[test]
    fn a_json_body_is_read_as_json_so_its_credential_fields_are_removed() {
        let r = Reading {
            value: Some(model::ForeignJson(
                json!({"event": {"body": "{\"api_key\": \"sk-secret\", \"title\": \"x\"}"}}),
            )),
            ..Reading::default()
        };
        let sent = source::redact_value(&state_of(&r));
        assert!(!sent.to_string().contains("sk-secret"), "{sent}");
        assert!(sent.to_string().contains("title"), "{sent}");
    }
}
