//! The stretch of time a report reads and the windows it is split into.

use std::fmt;

use model::{OutcomeRange, OutcomeRangeChoice, OutcomeSpan, OutcomeWindow, OutcomeWindowChoice};

struct RangeSpec {
    key: OutcomeRange,
    text: &'static str,
    days: i64,
    label: &'static str,
}

struct WindowSpec {
    key: OutcomeWindow,
    text: &'static str,
    days: i64,
    label: &'static str,
    unit: &'static str,
    last: &'static str,
    before: &'static str,
}

/// Every range, in the order they are offered, with the words each is shown in.
const RANGES: [RangeSpec; 5] = [
    RangeSpec {
        key: OutcomeRange::Days7,
        text: "7d",
        days: 7,
        label: "Last 7 days",
    },
    RangeSpec {
        key: OutcomeRange::Days30,
        text: "30d",
        days: 30,
        label: "Last 30 days",
    },
    RangeSpec {
        key: OutcomeRange::Days90,
        text: "90d",
        days: 90,
        label: "Last 90 days",
    },
    RangeSpec {
        key: OutcomeRange::Days180,
        text: "180d",
        days: 180,
        label: "Last 180 days",
    },
    RangeSpec {
        key: OutcomeRange::Days365,
        text: "365d",
        days: 365,
        label: "Last year",
    },
];

/// Every window, in the order they are offered, with the words each is shown and read in.
const WINDOWS: [WindowSpec; 3] = [
    WindowSpec {
        key: OutcomeWindow::Days1,
        text: "1d",
        days: 1,
        label: "By day",
        unit: "day",
        last: "the last day",
        before: "the day before",
    },
    WindowSpec {
        key: OutcomeWindow::Days7,
        text: "7d",
        days: 7,
        label: "By week",
        unit: "week",
        last: "the last 7 days",
        before: "the 7 days before",
    },
    WindowSpec {
        key: OutcomeWindow::Days30,
        text: "30d",
        days: 30,
        label: "By 30 days",
        unit: "month",
        last: "the last 30 days",
        before: "the 30 days before",
    },
];

fn range_spec(range: OutcomeRange) -> &'static RangeSpec {
    match range {
        OutcomeRange::Days7 => &RANGES[0],
        OutcomeRange::Days30 => &RANGES[1],
        OutcomeRange::Days90 => &RANGES[2],
        OutcomeRange::Days180 => &RANGES[3],
        OutcomeRange::Days365 => &RANGES[4],
    }
}

fn window_spec(window: OutcomeWindow) -> &'static WindowSpec {
    match window {
        OutcomeWindow::Days1 => &WINDOWS[0],
        OutcomeWindow::Days7 => &WINDOWS[1],
        OutcomeWindow::Days30 => &WINDOWS[2],
    }
}

/// Why a span cannot be read: the window does not fit twice in the range, so there would be no period before the last
/// to compare it with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpanError {
    pub range: OutcomeRange,
    pub window: OutcomeWindow,
}

impl fmt::Display for SpanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (w, r) = (window_spec(self.window).text, range_spec(self.range).text);
        write!(f, "a window of {w} does not fit twice in a range of {r}")
    }
}

impl std::error::Error for SpanError {}

/// The stretch of time a report reads, split into periods of one window, in words for whoever shows it. Only
/// [`Span::new`] (and the default) makes one, so its range always holds at least two windows: the last and the one
/// before.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    range: OutcomeRange,
    window: OutcomeWindow,
    /// The last period, in words: "the last 7 days".
    last: &'static str,
    /// The period before it: "the 7 days before".
    before: &'static str,
    /// One period's name, to count them back: "week".
    unit: &'static str,
    /// The choices to read instead, in order. Each fits when it holds the other of this span at least twice: a range
    /// this window fits in twice, a window that fits twice in this range.
    ranges: Vec<OutcomeRangeChoice>,
    windows: Vec<OutcomeWindowChoice>,
    periods: usize,
    window_days: i64,
}

/// One day in nanoseconds, the unit every instant of a report is compared in.
const DAY_NANOS: i128 = 86_400_000_000_000;

impl Span {
    /// The span for range `range` split by window `window`; an error when the window does not fit twice in the range.
    pub fn new(range: OutcomeRange, window: OutcomeWindow) -> Result<Span, SpanError> {
        if window_spec(window).days >= range_spec(range).days {
            return Err(SpanError { range, window });
        }
        Ok(Self::fitting(range, window))
    }

    /// The span for a window that fits twice in the range, which the caller has checked.
    fn fitting(range: OutcomeRange, window: OutcomeWindow) -> Span {
        let (r, w) = (range_spec(range), window_spec(window));
        // Both lengths are a few hundred days at most and the window is at least one, so neither division fails.
        let periods = r
            .days
            .saturating_add(w.days)
            .saturating_sub(1)
            .checked_div(w.days)
            .unwrap_or(0);
        Span {
            range,
            window,
            last: w.last,
            before: w.before,
            unit: w.unit,
            ranges: RANGES
                .iter()
                .map(|x| OutcomeRangeChoice {
                    key: x.key,
                    label: x.label.to_owned(),
                    fits: w.days < x.days,
                })
                .collect(),
            windows: WINDOWS
                .iter()
                .map(|x| OutcomeWindowChoice {
                    key: x.key,
                    label: x.label.to_owned(),
                    fits: x.days < r.days,
                })
                .collect(),
            periods: usize::try_from(periods).unwrap_or(0),
            window_days: w.days,
        }
    }

    /// The span in words for whoever shows it, with the choices to read instead.
    #[must_use]
    pub fn shown(&self) -> OutcomeSpan {
        OutcomeSpan {
            range: self.range,
            window: self.window,
            last: self.last.to_owned(),
            before: self.before.to_owned(),
            unit: self.unit.to_owned(),
            ranges: self.ranges.clone(),
            windows: self.windows.clone(),
        }
    }

    #[must_use]
    pub fn range(&self) -> OutcomeRange {
        self.range
    }

    #[must_use]
    pub fn window(&self) -> OutcomeWindow {
        self.window
    }

    #[must_use]
    pub fn last(&self) -> &'static str {
        self.last
    }

    #[must_use]
    pub fn before(&self) -> &'static str {
        self.before
    }

    #[must_use]
    pub fn unit(&self) -> &'static str {
        self.unit
    }

    #[must_use]
    pub fn ranges(&self) -> &[OutcomeRangeChoice] {
        &self.ranges
    }

    #[must_use]
    pub fn windows(&self) -> &[OutcomeWindowChoice] {
        &self.windows
    }

    /// How many periods the span is split into.
    #[must_use]
    pub fn periods(&self) -> usize {
        self.periods
    }

    /// One window's length in days.
    pub(crate) fn window_days(&self) -> i64 {
        self.window_days
    }

    /// One window's length in nanoseconds.
    pub(crate) fn window_nanos(&self) -> i128 {
        i128::from(self.window_days).saturating_mul(DAY_NANOS)
    }
}

/// What a report reads unless asked otherwise: thirteen weeks, the last against the one before. A week fits twice in
/// 90 days.
impl Default for Span {
    fn default() -> Self {
        Self::fitting(OutcomeRange::Days90, OutcomeWindow::Days7)
    }
}
