//! A Google Analytics reading (ADR-0046, `source::reading_poll`) is one number of the property: a metric over the last days, or
//! over the last minutes, kept to exact dimension values. A sensor of the googleanalytics source asks for one as
//! `reading:report metric=<name> [days=1] [lag=1] [<dimension>=<value>...]` (the days that end lag days ago, in the property's
//! time zone) or `reading:realtime metric=<name> [minutes=30] [<dimension>=<value>...]`; a value with a space is written escaped
//! (country=United%20States). Google computes it, so a reading is the number the owner sees in Google Analytics, and each minute
//! the sensor reads is one event whose `value` a measure (ADR-0045) is a condition on.

use std::collections::BTreeMap;
use std::fmt;

use plugin_sdk::{Error, PollAnswer, PollItem};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use source::{READING_PREFIX, is_reading, limit, reading_poll, render};

use super::{DateRange, Field, GoogleAnalytics, MinuteRange, Request, fields, filter_of, minutes_of, parse};

const GRAMMAR: &str = "`reading:report metric=<name> [days=1] [lag=1] [<dimension>=<value>...]` or `reading:realtime metric=<name> [minutes=30] [<dimension>=<value>...]`";
const MAX_DAYS: i64 = 366;
const MAX_LAG: i64 = 30;
const DEFAULT_LAG: i64 = 1;

/// A reading as an owner or an agent asks for it.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Reading {
    realtime: bool,
    metric: String,
    days: i64,
    lag: i64,
    minutes: i64,
    dims: BTreeMap<String, String>,
}

impl Reading {
    /// The reading of `kind` (report, the default, or realtime), each zero number at its default, or why it is not one.
    fn new(
        kind: &str,
        metric: &str,
        days: i64,
        lag: i64,
        minutes: i64,
        dims: BTreeMap<String, String>,
    ) -> Result<Reading, String> {
        let mut r = Reading {
            realtime: false,
            metric: metric.trim().to_owned(),
            days: 0,
            lag: 0,
            minutes: 0,
            dims,
        };
        match kind {
            "" | "report" => {
                if minutes != 0 {
                    return Err("minutes is for a realtime reading: a report reads days".to_owned());
                }
                if !(0..=MAX_DAYS).contains(&days) {
                    return Err(format!("days is between 1 and {MAX_DAYS}"));
                }
                if !(0..=MAX_LAG).contains(&lag) {
                    return Err(format!("lag is between 0 (today) and {MAX_LAG} days"));
                }
                (r.days, r.lag) = (days.max(1), lag);
            }
            "realtime" => {
                if days != 0 || lag != 0 {
                    return Err("days and lag are for a report reading: realtime reads minutes".to_owned());
                }
                (r.realtime, r.minutes) = (true, minutes_of(minutes)?);
            }
            _ => return Err(format!("a reading is a report or realtime, not {:?}", limit(kind, 40))),
        }
        fields("metric", std::slice::from_ref(&r.metric), 1, 1)?;
        filter_of(&r.dims)?;
        Ok(r)
    }

    /// The reading a sensor's query asks for, in `GRAMMAR`. Without lag= a report reads yesterday.
    pub(super) fn parse(query: &str) -> Result<Reading, String> {
        let words: Vec<&str> = query.split_whitespace().collect();
        let Some((first, rest)) = words.split_first().filter(|(w, _)| is_reading(w)) else {
            return Err(format!("a query of Google Analytics is a reading: {GRAMMAR}"));
        };
        let kind = first.strip_prefix(READING_PREFIX).unwrap_or(first);
        let mut metric = String::new();
        let mut nums: BTreeMap<&str, i64> = BTreeMap::new();
        let mut dims = BTreeMap::new();
        let mut lag_set = false;
        for word in rest {
            let Some((key, value)) = word.split_once('=').filter(|(k, v)| !k.is_empty() && !v.is_empty()) else {
                return Err(format!(
                    "reading query: {:?} is not a word of the form name=value ({GRAMMAR})",
                    limit(word, 40)
                ));
            };
            match key {
                "metric" => value.clone_into(&mut metric),
                "days" | "lag" | "minutes" => {
                    let n = value
                        .parse::<i64>()
                        .ok()
                        .filter(|n| *n >= 1 || key == "lag")
                        .ok_or_else(|| format!("reading query: {key}={} is not a number of {key}", limit(value, 20)))?;
                    nums.insert(key, n);
                    lag_set = lag_set || key == "lag";
                }
                _ => {
                    let v = urlescape::unescape(value).ok_or_else(|| {
                        format!(
                            "reading query: the value of {} is not escaped as a URL path is (a space is %20)",
                            limit(key, 40)
                        )
                    })?;
                    dims.insert(key.to_owned(), v);
                }
            }
        }
        if metric.is_empty() {
            return Err(format!("reading query: metric= names the number to read ({GRAMMAR})"));
        }
        let num = |k: &str| nums.get(k).copied().unwrap_or(0);
        let lag = if !lag_set && kind != "realtime" {
            DEFAULT_LAG
        } else {
            num("lag")
        };
        Reading::new(kind, &metric, num("days"), lag, num("minutes"), dims)
            .map_err(|e| format!("reading query: {e} ({GRAMMAR})"))
    }
}

/// The query of a sensor that reads `r`: what an agent copies into a sensor, read back by `Reading::parse` as the same reading.
impl fmt::Display for Reading {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.realtime {
            write!(
                f,
                "{READING_PREFIX}realtime metric={} minutes={}",
                self.metric, self.minutes
            )?;
        } else {
            write!(
                f,
                "{READING_PREFIX}report metric={} days={} lag={}",
                self.metric, self.days, self.lag
            )?;
        }
        for (dim, value) in &self.dims {
            write!(f, " {dim}={}", urlescape::path_segment(value))?;
        }
        Ok(())
    }
}

/// The day `n` days before today as the Data API names it, in the property's time zone.
fn days_ago(n: i64) -> String {
    match n {
        0 => "today".to_owned(),
        1 => "yesterday".to_owned(),
        n => format!("{n}daysAgo"),
    }
}

/// The event of a reading: what was read and the number. `value` is null, with `no_data`, when Google has no row.
#[derive(Debug, Serialize)]
struct Body {
    query: String,
    kind: String,
    metric: String,
    #[serde(rename = "where", skip_serializing_if = "BTreeMap::is_empty")]
    dims: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    from: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    to: String,
    #[serde(skip_serializing_if = "is_zero")]
    minutes: i64,
    #[serde(skip_serializing_if = "String::is_empty")]
    time_zone: String,
    value: Option<f64>,
    no_data: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    thresholded: bool,
}

fn is_zero(n: &i64) -> bool {
    *n == 0
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadingInput {
    /// report (default: the last days) or realtime (the last minutes)
    #[serde(default)]
    kind: String,
    /// one metric API name, such as activeUsers, sessions, eventCount, keyEvents, totalRevenue
    metric: String,
    /// report: how many days the number covers, 1 to 366; default 1
    #[serde(default)]
    days: i64,
    /// report: the last day is this many days ago, 0 (today) to 30; default 1 (yesterday). Google may still add to the last 48 hours: 2 reads settled days
    #[serde(default)]
    lag: Option<i64>,
    /// realtime: how many of the last minutes, 1 to 30; default 30
    #[serde(default)]
    minutes: i64,
    /// count only events whose dimension has exactly this value, such as {"eventName":"purchase"}
    #[serde(default, rename = "where")]
    r#where: BTreeMap<String, String>,
}

impl GoogleAnalytics {
    /// The number `r` asks for as Google computes it now: the one function the tool and the poll share.
    async fn read(&self, r: &Reading) -> Result<Body, Error> {
        let filter = filter_of(&r.dims)?;
        let mut body = Body {
            query: r.to_string(),
            kind: "report".to_owned(),
            metric: r.metric.clone(),
            dims: r.dims.clone(),
            from: String::new(),
            to: String::new(),
            minutes: 0,
            time_zone: String::new(),
            value: None,
            no_data: false,
            thresholded: false,
        };
        let mut req = Request {
            metrics: vec![Field { name: r.metric.clone() }],
            dimension_filter: filter,
            ..Default::default()
        };
        let method = if r.realtime {
            "realtime".clone_into(&mut body.kind);
            body.minutes = r.minutes;
            req.minute_ranges = vec![MinuteRange {
                start_minutes_ago: r.minutes.saturating_sub(1),
                end_minutes_ago: 0,
            }];
            "runRealtimeReport"
        } else {
            body.from = days_ago(r.lag.saturating_add(r.days).saturating_sub(1));
            body.to = days_ago(r.lag);
            // A day without events is a row of zero, not a missing row: the number that fell to nothing is the one to hear.
            req.date_ranges = vec![DateRange {
                start_date: body.from.clone(),
                end_date: body.to.clone(),
            }];
            req.keep_empty_rows = true;
            "runReport"
        };
        let (raw, _) = self.call(method, Some(&req)).await?;
        let res = parse(&raw, "reading")?;
        body.time_zone = res.metadata.time_zone;
        body.thresholded = res.metadata.subject_to_thresholding;
        let Some(cell) = res.rows.first().and_then(|row| row.metric_values.first()) else {
            body.no_data = true;
            return Ok(body);
        };
        let v: f64 = cell.value.parse().map_err(|_| {
            format!(
                "google analytics: {} is {:?}, not a number",
                r.metric,
                limit(&cell.value, 40)
            )
        })?;
        body.value = Some(v);
        Ok(body)
    }

    pub(super) async fn reading(self, input: ReadingInput) -> Result<String, Error> {
        let kind = input.kind.trim();
        let lag = match input.lag {
            Some(lag) => lag,
            None if kind != "realtime" => DEFAULT_LAG,
            None => 0,
        };
        let r = Reading::new(kind, &input.metric, input.days, lag, input.minutes, input.r#where)?;
        let body = self.read(&r).await?;
        Ok(format!(
            "{}\nA sensor of the googleanalytics source reads this number with the query: {r}",
            render(&body)?
        ))
    }

    /// The plugin's poll: a reading, polled as every reading is (`source::reading_poll`), one event per minute it is read.
    pub(super) async fn poll(&self, query: &str, cursor: &str) -> Result<PollAnswer, Error> {
        let r = Reading::parse(query)?;
        let res = reading_poll(self.now, cursor, |_| async {
            self.read(&r).await.map_err(|e| source::Error::msg(e.to_string()))
        })
        .await?;
        let items = res
            .items
            .into_iter()
            .map(|it| {
                Ok(PollItem {
                    id: it.id,
                    version: it.version,
                    body: serde_json::from_str(&it.body)?,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(PollAnswer {
            items,
            cursor: res.cursor,
        })
    }
}
