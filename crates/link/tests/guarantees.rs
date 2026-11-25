//! The five guarantees of the link (ADR-0072 decision 3), each driven through two halves in memory on a fake clock.

use std::collections::HashMap;
use std::future::{Future, ready};
use std::sync::{Mutex, PoisonError};

use link::Payload;
use link::memory::{Pair, now_or_never};
use link::{Closing, Config, ErrorCode, Event, Failure, Request};

mod demo {
    link::contract! {
        capability: "demo";
        reads { echo(text: String) -> String, why = "answers what it is given"; }
        changes {
            bump(by: i64) -> i64;
            pause(id: String) -> i64, order = id;
            resume(id: String) -> i64, order = id;
        }
        idempotent {}
        pushes { bumped(total: i64); }
    }
}

/// Acts once per request id and answers each retry as it answered the first, as the server's `requests` table does.
#[derive(Default)]
struct Counter {
    total: Mutex<i64>,
    answered: Mutex<HashMap<String, i64>>,
}

impl demo::Handler for Counter {
    fn echo(&self, text: String) -> impl Future<Output = Result<String, Failure>> + Send {
        ready(Ok(text))
    }

    fn bump(&self, request: Request, by: i64) -> impl Future<Output = Result<i64, Failure>> + Send {
        let mut answered = self.answered.lock().unwrap_or_else(PoisonError::into_inner);
        let answer = *answered.entry(request.id).or_insert_with(|| {
            let mut total = self.total.lock().unwrap_or_else(PoisonError::into_inner);
            *total = total.saturating_add(by);
            *total
        });
        ready(Ok(answer))
    }

    fn pause(&self, _: Request, _: String) -> impl Future<Output = Result<i64, Failure>> + Send {
        ready(Ok(0))
    }

    fn resume(&self, _: Request, _: String) -> impl Future<Output = Result<i64, Failure>> + Send {
        ready(Ok(0))
    }
}

fn cfg() -> Config {
    Config {
        call_timeout: 1_000,
        behind_bytes: 200,
        ping_every: 10_000,
    }
}

fn answers(events: &[Event], id: link::CallId) -> Vec<Result<Payload, Failure>> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Answer { id: got, result } if *got == id => Some(result.clone()),
            _ => None,
        })
        .collect()
}

fn calls(events: &mut Vec<Event>) -> Vec<(link::Incoming, String, Payload)> {
    let mut out = Vec::new();
    events.retain(|e| match e {
        Event::Call { id, method, body, .. } => {
            out.push((*id, method.clone(), body.clone()));
            false
        }
        _ => true,
    });
    out
}

fn serve_all(pair: &mut Pair, handler: &Counter) {
    for (id, method, args) in calls(&mut pair.acceptor_events) {
        // The handler here answers at once; one that would wait leaves its call unanswered, which the test sees.
        if let Some(answer) = now_or_never(demo::serve(handler, &method, &args)) {
            pair.acceptor.reply(id, answer);
        }
    }
    pair.settle();
}

#[test]
fn a_changing_call_replayed_after_reconnect_acts_once() {
    let handler = Counter::default();
    let action = demo::client::bump(Request { id: "r1".into() }, 2);

    // The server acts, and the session ends before its answer is sent.
    let mut first = Pair::admitted(cfg(), &["demo"], &[]);
    let sent = first.dialer.call(first.now, action.clone());
    first.settle();
    let (_, method, args) = calls(&mut first.acceptor_events).remove(0);
    now_or_never(demo::serve(&handler, &method, &args)).unwrap().unwrap();
    first.acceptor.closed(Closing::Done);
    first.dialer.closed(Closing::Done);
    first.settle();
    assert_eq!(
        answers(&first.dialer_events, sent.id())[0].as_ref().unwrap_err().code,
        ErrorCode::Disconnected
    );

    // The client reconnects and retries the same action: one effect, the first answer.
    let mut again = Pair::admitted(cfg(), &["demo"], &[]);
    let retry = again.dialer.call(again.now, action);
    again.settle();
    serve_all(&mut again, &handler);
    assert_eq!(
        retry.decode(answers(&again.dialer_events, retry.id())[0].clone()),
        Ok(2)
    );
    assert_eq!(*handler.total.lock().unwrap(), 2);
}
