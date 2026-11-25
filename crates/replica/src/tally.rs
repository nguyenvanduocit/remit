//! What this core saw on the link, counted for the server's telemetry and reported once a minute (ADR-0095 decision
//! 12): the bytes and decode time of each kind of answer and push, the repairs by reason and home, how long after an
//! event happened the core had it, and what failed in the page (ADR-0119). Pure: the core feeds it as frames arrive and
//! takes a report when a minute is up.

use std::collections::BTreeMap;

use link::Millis;
use wire::app::{
    Delays, FaultKind, FrameKind, FrameTally, LinkReport, PAGE_FAULTS_MAX, PageFault, RepairReason, RepairTally,
    fault_text,
};

/// How often what was counted is reported.
pub(crate) const EVERY: Millis = 60_000;

#[derive(Debug, Default)]
pub(crate) struct Tally {
    frames: BTreeMap<(FrameKind, String), Counted>,
    repairs: BTreeMap<(RepairReason, String), i64>,
    /// Milliseconds from each pushed event's `at` to the core holding it, on the core's clock.
    delays: Vec<i64>,
    /// What failed, by kind, message, where and the chain shown: at most `PAGE_FAULTS_MAX` of them, a later one of another
    /// kind dropped.
    faults: BTreeMap<(FaultKind, String, String, String), i64>,
    /// When the first thing of this minute was counted; none while nothing waits to be reported.
    since: Option<Millis>,
    /// The reports taken so far: with the page's salt, it names a report, so one sent again is kept once.
    minute: i64,
}

#[derive(Debug, Default, Clone, Copy)]
struct Counted {
    count: i64,
    bytes: i64,
    decode_us: i64,
}

impl Tally {
    /// A frame of `kind` named `name`, `bytes` long, decoded in `decode_ms` (the shell's precise clock).
    pub(crate) fn frame(&mut self, now: Millis, kind: FrameKind, name: &str, bytes: usize, decode_ms: f64) {
        self.since.get_or_insert(now);
        let c = self.frames.entry((kind, name.to_owned())).or_default();
        c.count = c.count.saturating_add(1);
        c.bytes = c.bytes.saturating_add(i64::try_from(bytes).unwrap_or(i64::MAX));
        c.decode_us = c.decode_us.saturating_add(micros(decode_ms));
    }

    pub(crate) fn repair(&mut self, now: Millis, reason: RepairReason, home: &str) {
        self.since.get_or_insert(now);
        let n = self.repairs.entry((reason, home.to_owned())).or_default();
        *n = n.saturating_add(1);
    }

    /// An event that happened at `at_ms` (the server's clock) reached the core at `now` (its own).
    pub(crate) fn delay(&mut self, now: Millis, at_ms: i64) {
        self.since.get_or_insert(now);
        self.delays
            .push(i64::try_from(now).unwrap_or(i64::MAX).saturating_sub(at_ms));
    }

    /// A failure of kind in the page, saying message, at where, while it showed chain (empty: none).
    pub(crate) fn fault(&mut self, now: Millis, kind: FaultKind, message: &str, at: &str, chain: &str) {
        self.since.get_or_insert(now);
        let key = (kind, fault_text(message), fault_text(at), chain.to_owned());
        if self.faults.len() >= PAGE_FAULTS_MAX && !self.faults.contains_key(&key) {
            return;
        }
        let n = self.faults.entry(key).or_default();
        *n = n.saturating_add(1);
    }

    /// When the counted minute is up; none while nothing was counted.
    pub(crate) fn due(&self) -> Option<Millis> {
        self.since.map(|since| since.saturating_add(EVERY))
    }

    /// What was counted, as the report of page `page`, once its minute is up at `now`; the count starts again.
    pub(crate) fn take(&mut self, now: Millis, page: &str) -> Option<LinkReport> {
        if self.due().is_none_or(|due| due > now) {
            return None;
        }
        let frames = std::mem::take(&mut self.frames)
            .into_iter()
            .map(|((kind, name), c)| FrameTally {
                kind,
                name,
                count: c.count,
                bytes: c.bytes,
                decode_us: c.decode_us,
            })
            .collect();
        let repairs = std::mem::take(&mut self.repairs)
            .into_iter()
            .map(|((reason, home), count)| RepairTally { reason, home, count })
            .collect();
        let delays = summed(std::mem::take(&mut self.delays));
        let faults = std::mem::take(&mut self.faults)
            .into_iter()
            .map(|((kind, message, at, chain), count)| PageFault {
                kind,
                message,
                at,
                chain,
                count,
            })
            .collect();
        self.since = None;
        let minute = self.minute;
        self.minute = self.minute.saturating_add(1);
        Some(LinkReport {
            page: page.to_owned(),
            minute,
            sent_at: i64::try_from(now).unwrap_or(i64::MAX),
            frames,
            repairs,
            delays,
            faults,
        })
    }
}

fn micros(ms: f64) -> i64 {
    #[expect(
        clippy::as_conversions,
        clippy::cast_possible_truncation,
        reason = "a decode's microseconds are far inside i64; `as` saturates, and no From exists for f64"
    )]
    let us = (ms * 1_000.0).round() as i64;
    us.max(0)
}

/// The delays' count, median, 95th percentile and longest; none when no event was pushed.
fn summed(mut delays: Vec<i64>) -> Option<Delays> {
    delays.sort_unstable();
    let at = |share: usize| {
        delays
            .get(delays.len().saturating_sub(1).saturating_mul(share) / 100)
            .copied()
    };
    Some(Delays {
        count: i64::try_from(delays.len()).unwrap_or(i64::MAX),
        p50_ms: at(50)?,
        p95_ms: at(95)?,
        max_ms: *delays.last()?,
    })
}
