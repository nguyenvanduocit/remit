//! A page's link report as the telemetry database's wide events (ADR-0095 decision 12): one record per kind of frame,
//! per repair, for the delays and per fault, all naming the page as their trace and the report's minute as their span,
//! so a report sent again is kept once. Its faults, kept for the first time, also go to Datadog as the page's errors
//! (ADR-0119). The page is not trusted: `keep` cuts what it sent to the contract's bounds (`bounded`), lets a session send at
//! most `PAGE_FAULTS_MAX` faults a minute of the server's clock however it names its reports (`FaultBudget`), keeps a
//! fault's chain only when the session's own rule, read from the store, says it may name it, and passes the page's name
//! on as its session only when it has a core's shape. `records_of` and `bounded` are pure; `keep` hands them to the
//! orchestrator.

use std::sync::{Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use link::Failure;
use orch::Orchestrator;
use serde_json::{Map, Value, json};
use telemetry::{Kind, Record};
use wire::Ack;
use wire::app::{LinkReport, PAGE_FAULTS_MAX, fault_text};

use crate::convert::failed;

/// The most characters of a page's name that are kept: the core names itself with `PAGE_NAME_HEX` hex digits.
const PAGE_NAME_MAX: usize = 64;
/// How many hex digits a core's name has.
const PAGE_NAME_HEX: usize = 32;
/// The most times one fault is said to have happened in a minute: one each frame of a page drawn 60 times a second is
/// 3,600.
const FAULT_COUNT_MAX: i64 = 100_000;

/// Whose session sent a link report, which says which chains its faults may name.
pub(crate) enum Sender<'a> {
    /// The owner's: any chain there is.
    Owner,
    /// A mentor's, signed in with the worker key of this id: a chain one of his helps is on.
    Mentor(&'a str),
}

impl Sender<'_> {
    /// Whether the session may name chain, as the store holds it now.
    async fn may_name(&self, o: &Orchestrator, chain: &str) -> bool {
        match self {
            Self::Owner => o.owner_names(chain).await,
            Self::Mentor(key_id) => o.mentor_names(key_id, chain).await,
        }
    }
}

/// Keeps report, as from's page sent it, in the server's telemetry as received now: `bounded`, with the faults budget lets
/// through, each naming its chain only when from may name it. Those faults go to Datadog the first time the report is
/// kept.
pub(crate) async fn keep(
    o: &Orchestrator,
    from: Sender<'_>,
    report: LinkReport,
    budget: &FaultBudget,
) -> Result<Ack, Failure> {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
    let mut report = bounded(report);
    let taken = budget.take(now_ms, report.faults.len());
    report.faults.truncate(taken);
    for f in &mut report.faults {
        if !f.chain.is_empty() && !from.may_name(o, &f.chain).await {
            f.chain.clear();
        }
    }
    let kept = o
        .record_link(records_of(&report, now_ms))
        .await
        .map_err(|e| failed("link_report", &e))?;
    if kept {
        o.page_faults(session_of(&report.page), &report.faults);
    }
    Ok(Ack {})
}

/// page as a session's name: a core names itself with `PAGE_NAME_HEX` hex digits, and a name of another shape is none.
fn session_of(page: &str) -> &str {
    if page.len() == PAGE_NAME_HEX && page.bytes().all(|b| b.is_ascii_hexdigit()) {
        page
    } else {
        ""
    }
}

/// report within the contract's bounds: its name cut at `PAGE_NAME_MAX` characters, at most `PAGE_FAULTS_MAX` faults, each
/// text at most `PAGE_FAULT_TEXT_MAX` characters long and each count between 1 and `FAULT_COUNT_MAX`.
fn bounded(mut report: LinkReport) -> LinkReport {
    report.page = report.page.chars().take(PAGE_NAME_MAX).collect();
    report.faults.truncate(PAGE_FAULTS_MAX);
    for f in &mut report.faults {
        f.message = fault_text(&f.message);
        f.at = fault_text(&f.at);
        f.chain = fault_text(&f.chain);
        f.count = f.count.clamp(1, FAULT_COUNT_MAX);
    }
    report
}

/// The faults one session may still send: at most `PAGE_FAULTS_MAX` in a minute of the server's clock, counted as they
/// are taken, so a page that names a new report each time sends no more.
#[derive(Debug, Default)]
pub(crate) struct FaultBudget(Mutex<(i64, usize)>);

impl FaultBudget {
    /// How many of n faults received at now_ms (the server's clock, in milliseconds) are taken.
    pub(crate) fn take(&self, now_ms: i64, n: usize) -> usize {
        let minute = now_ms.div_euclid(60_000);
        let mut spent = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if spent.0 != minute {
            *spent = (minute, 0);
        }
        let took = n.min(PAGE_FAULTS_MAX.saturating_sub(spent.1));
        spent.1 = spent.1.saturating_add(took);
        took
    }
}

/// The records of `report`, received at `now_ms` on the server's clock. The core measured its delays on its own clock:
/// they are moved onto the server's by how far the core's clock was from it when it sent the report.
pub(crate) fn records_of(report: &LinkReport, now_ms: i64) -> Vec<Record> {
    let at_ns = now_ms.saturating_mul(1_000_000);
    let skew = now_ms.saturating_sub(report.sent_at);
    let record = |name: &str, value: i64, attrs: Value| Record {
        at_ns,
        kind: Kind::Event,
        name: name.to_owned(),
        value: Some(float(value)),
        trace_id: report.page.clone(),
        span_id: report.minute.to_string(),
        parent_id: String::new(),
        attrs: match attrs {
            Value::Object(map) => map,
            _ => Map::new(),
        },
    };
    let mut records: Vec<Record> = report
        .frames
        .iter()
        .map(|f| {
            record(
                "remit.link.frame",
                f.bytes,
                json!({ "kind": f.kind, "frame": f.name, "count": f.count, "bytes": f.bytes, "decode_us": f.decode_us }),
            )
        })
        .collect();
    records.extend(report.repairs.iter().map(|r| {
        record(
            "remit.link.repair",
            r.count,
            json!({ "reason": r.reason, "home": r.home, "count": r.count }),
        )
    }));
    records.extend(report.faults.iter().map(|f| {
        record(
            "remit.page.fault",
            f.count,
            json!({ "kind": f.kind, "message": f.message, "at": f.at, "chain": f.chain, "count": f.count }),
        )
    }));
    if let Some(d) = &report.delays {
        records.push(record(
            "remit.link.delay",
            d.count,
            json!({
                "count": d.count,
                "p50_ms": d.p50_ms.saturating_add(skew),
                "p95_ms": d.p95_ms.saturating_add(skew),
                "max_ms": d.max_ms.saturating_add(skew),
                "skew_ms": skew,
            }),
        ));
    }
    records
}

fn float(n: i64) -> f64 {
    #[expect(
        clippy::as_conversions,
        clippy::cast_precision_loss,
        reason = "a count or a byte total is far inside f64's exact range"
    )]
    let f = n as f64;
    f
}

#[cfg(test)]
mod tests {
    use wire::app::{FaultKind, PAGE_FAULT_TEXT_MAX, PageFault};

    use super::*;

    #[test]
    fn a_page_cannot_send_more_faults_or_longer_texts_than_the_bounds() {
        let long = "x".repeat(PAGE_FAULT_TEXT_MAX * 4);
        let fault = PageFault {
            kind: FaultKind::Uncaught,
            message: long.clone(),
            at: long.clone(),
            chain: long.clone(),
            count: i64::MAX,
        };
        let report = LinkReport {
            page: long,
            minute: 0,
            sent_at: 0,
            frames: Vec::new(),
            repairs: Vec::new(),
            delays: None,
            faults: vec![fault; PAGE_FAULTS_MAX * 3],
        };
        let got = bounded(report);
        assert_eq!(got.page.chars().count(), PAGE_NAME_MAX);
        assert_eq!(got.faults.len(), PAGE_FAULTS_MAX);
        for f in &got.faults {
            for text in [&f.message, &f.at, &f.chain] {
                assert_eq!(text.chars().count(), PAGE_FAULT_TEXT_MAX);
            }
            assert_eq!(f.count, FAULT_COUNT_MAX);
        }
    }

    #[test]
    fn a_session_sends_no_more_faults_a_minute_however_it_names_its_reports() {
        let budget = FaultBudget::default();
        let minute = 60_000 * 1_000;
        assert_eq!(budget.take(minute, PAGE_FAULTS_MAX - 2), PAGE_FAULTS_MAX - 2);
        assert_eq!(budget.take(minute + 30_000, 5), 2, "the minute's room is what is left");
        assert_eq!(budget.take(minute + 59_999, 5), 0);
        assert_eq!(budget.take(minute + 60_000, 5), 5, "the next minute starts again");
    }

    #[test]
    fn only_a_core_shaped_name_goes_on_as_a_session() {
        assert_eq!(
            session_of("0af7651916cd43dd8448eb211c80319c"),
            "0af7651916cd43dd8448eb211c80319c"
        );
        for other in [
            "",
            "0af7651916cd43dd8448eb211c80319",
            "user@example.com",
            "0af7651916cd43dd8448eb211c80319z",
        ] {
            assert_eq!(session_of(other), "", "{other}");
        }
    }
}
