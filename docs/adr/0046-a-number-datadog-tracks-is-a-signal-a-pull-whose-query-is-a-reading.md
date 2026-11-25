# 0046. A number Datadog tracks is a signal: a pull whose query is a reading

Status: accepted, 2026-11-03; partly superseded by ADR-0047: the reading is the query of a sensor that writes the signal, not a pull of the signal; amended 2026-11-05: Grafana reads the same way (*Grafana*, below). Builds on ADR-0024 (a signal that pulls), ADR-0036 (an event signal holds readings) and ADR-0045 (a
measure counts only when a signal reads it). Research: `docs/research-datadog-signals.md`. Source: the owner's words of 2026-11-03: his
apps are to be tracked and monitored on Datadog, and the numbers monitored are to be signals, so the feedback loop is clear, stable and
better.

## Context

ADR-0045 says a measure is a signal and a condition, and that a signal must carry a number before a measure is drafted. The Datadog
source had one pull: the monitors that leave OK (`datadog.go`, `Poll`). A monitor is a threshold somebody already chose in Datadog,
and its event says that the threshold was crossed, not what the number was. A measure written on it cannot say "the error rate stays
under 2%": the 2% lives in Datadog, the objective only hears that it broke, and moving the threshold means editing two systems. The
first dogfood objective (Lights Daily) failed for the lack of such a number (ADR-0045, *Context*).

## Options

1. **A bundled `datadog` plugin that takes the whole source over**, as `intercom` did (ADR-0037). It ships signals and an agent, but
   moves 717 lines and their tests, loses the script form and the sensors Datadog has today (ADR-0043: `Inject` stays in the core
   for it), and adds nothing the pull needs: the source already polls. A shipped signal would also be app-less, and the number of an
   app is the owner's to name.
2. **A monitor per measure.** The Chair creates a Datadog monitor for each key result and the signal hears its transitions. The
   threshold is then in Datadog and in the objective, the signal is a state and not a number, and Remit would write to Datadog.
3. **A pull form that reads one number, on the source that already polls.** Datadog computes the number (the same endpoint its own
   notebooks use), Remit reads it on a period and each minute's number is an event. **Chosen.**

## Decision

- **The query of a Datadog pull is a monitor search, or a reading:** `reading:<source> [window=<duration>] [agg=<aggregator>]
  <search>`. `reading:` is no facet of Datadog's monitor search, so a monitor named "reading latency" is still found as before. Source
  `metrics` takes a metric query without a `by` clause and an `agg` of avg (default), min, max, sum or last; `logs` and `rum` count the
  events their search matches. The window is 1m to 31d, default 1h, and ends at the last full minute.
- **Datadog computes it:** `POST /api/v2/query/scalar` (an endpoint that reads; the script form already lists it), one query named `a`
  and the formula `a`. An answer grouped by a tag or facet is refused with the group it found: a reading is one number.
- **The event is the minute's number.** Item id `reading`, version the minute the window ends at, so a replay is the same event
  (ADR-0024). The body is `{source, query, aggregator, window, from, to, value, no_data}`: a measure is a condition on `body.value`
  (`body.value <= 0.02`). A window with no data is an event with `value` null and `no_data` true, never a zero and never silence: the
  app that stopped reporting is the loudest thing a tracked app can say, and a condition on null cannot be read, so the dry run of
  ADR-0045 says so.
- **The cursor is the minute last read.** A poll in a minute already read answers nothing; a poll after a gap answers the number as it is
  now, not the minutes between (a reading is a state, not a history). The first poll answers no item (ADR-0024) but reads once, so a wrong
  key, a wrong site or a malformed query fails at the first poll and shows on the signal's line, not a day later.
- **The tool `datadog_reading`** answers the same number with the same grammar as arguments, and ends with the query a pull takes. The Chair
  tries the number before it drafts a measure on it (ADR-0045: a measure whose signal does not read is not drafted). One function
  (`datadog.scalar`) serves the tool and the poll.
- **No shipped signal and no new agent.** An app's number is the owner's to name: the owner tells the Assistant which number of which
  app, and the Assistant sets the pull, as for any signal (*Agent-centric*). Getting an app to report to Datadog is repository work for a
  chain, the first key result of its objective (ADR-0045), and `docs/research-datadog-signals.md` says how, per kind of app.

## Consequences

- An objective's numbers can be the ones Datadog tracks: error rate, latency percentile (as a metric), log error count, RUM sessions.
  The threshold lives in the objective, in one place, and the number in Datadog, in one place.
- A reading signal's first event comes with the second read (the first only sets the cursor), at most one period after it is resumed.
- Every reading is a stored reading of `event:<name>`: one a minute is 1,440 a day for a signal, and the owner chooses the period of the
  pull (`every`, at least a minute), so a measure checked hourly pulls hourly.

## Revisit when

- **Not run against a live organisation.** The metrics shape and the RUM shape are Datadog's documented examples; the `logs` shape follows
  the documented schema and has no published example. The first live read of each is the check (`datadog_reading` on the owner's real
  organisation); the tests use a fake that answers as the documentation says.
- **Datadog's own events as pull forms** (error tracking issues, incidents, SLO status, security signals; ranked in the research):
  each is a new first word beside `reading:` in `datadogPoll`, not a new mechanism.
- **A percentile or a cardinality of a log or RUM field** needs `compute.metric`; today `count` is the one aggregator of those sources.
- **One signal per group** (a reading per service): a grouped answer becomes several items with the group in the id.
- **The newest minute is incomplete.** The window ends at the last full minute; Datadog's ingestion lag can still leave it light. A lag in
  minutes in the query is the seam (`datadogReading`).
- **The Datadog plugin.** When plugins carry scripts (ADR-0034 *Revisit when*), the source can move whole, as Intercom did.
- **An application key's scopes** (`timeseries_query` for metrics, the read scope of the product for logs and RUM) are the owner's to give;
  the missing one is Datadog's refusal shown on the signal's line.

## Grafana (amended 2026-11-05)

The follow-up ADR-0064 named: Grafana as a poll source. The decision is the same for a second source, so it is this ADR's, not a new one.

- **The reading poll is one rule for every source** (`source.ReadingPoll`, `internal/source/reading.go`): the cursor is the minute
  last read, the first poll reads once and answers no item, the item is `reading@<minute>`, the body is rendered and carries
  `value` (null with `no_data` true when there is no number). Datadog's poll and Grafana's call it with a function that reads their
  number for the minute; a plugin can call it too. A source owns only its grammar after `reading:` and how it computes the number.
- **Grafana's reading is `reading:promql [datasource=<uid or name>] <PromQL expression>`**: the value of an instant query at the
  minute, on a Prometheus-compatible datasource, through the datasource proxy `grafana_promql` already reads with
  (`GET /api/datasources/proxy/uid/<uid>/api/v1/query`, https://grafana.com/docs/grafana/latest/developers/http_api/data_source/).
  The window and the aggregation are the expression's own (`rate(x[5m])`, `sum(...)`), so the grammar has neither. An empty
  datasource is the only prometheus one. The body is `{datasource, query, at, value, no_data}`. An empty vector and NaN (PromQL's
  0/0) are no data; more than one series, a range vector or ±Inf are refused with what to change: a reading is one number.
  `grafana_promql` ends an instant answer that is one number with the query a sensor takes, as `datadog_reading` does.
- **Grafana's other poll is what fires**, as Datadog's is its monitors: the query is a word of the alert rules' names, as
  `grafana_alerts` takes it (empty: every rule), read with the same call (`GET /api/prometheus/grafana/api/v1/rules`, which lists
  each rule's instances unless `limit_alerts` is given; source: `pkg/services/ngalert/api/prometheus/api_prometheus.go` in
  grafana/grafana). Each instance that is Alerting, NoData or Error is an item: id `<rule uid>/<fingerprint of its labels>`,
  version `<state>@<activeAt>`. Grafana sets `activeAt` (the state's `StartsAt`) on every change of state, so an instance that
  fires, resolves and fires again is two events, and Pending, Normal and Recovering are none. The cursor is a `PollMark` over
  `activeAt`, as Datadog's is over the time a monitor's state changed. The body carries the rule, its folder and summary, the
  instance's state, labels and value, and the rule's page (`<GRAFANA_URL>/alerting/grafana/<uid>/view`).
- **The token never leaves.** Grafana's poll returns through `withoutSecrets` with every spelling of its Authorization
  (`authSecrets`), as the script form does.

Revisit when, for Grafana:

- **Not run against a live Grafana.** The rules shape is Grafana's source (`definitions/prom.go`), the instant-query shape
  Prometheus' documented one; the first live read on the owner's Grafana is the check (`grafana_promql`, then a sensor).
- **An evaluation in flight when a poll reads.** An instance whose evaluation started before the newest `activeAt` handed over and
  was written after it is behind the mark and missed; Datadog's monitor poll has the same edge. A lag on the mark is the seam
  (`grafana.poll`).
- **Many instances.** One GET lists every instance of the matched rules; a Grafana with thousands needs the endpoint's own filters
  (`rule_name`, `state`, `matcher`) in the query's grammar.
- **A Loki or SQL reading** is a new word after `reading:` in `parseGrafanaReading`, through `/api/ds/query`, not a new mechanism.

Enforced by: TestAReadingSignalsFirstPollMarksNowAndStillReadsDatadogSoABadQueryOrKeyShowsAtOnce, TestAReadingPollAnswersTheNumberOnceAMinuteAsAnEventANewMinuteIsANewReading, TestAReadingWithNoDataIsAnEventWithoutANumberNotAZeroAndNotSilence, TestALogAndARumReadingCountTheEventsOfTheirSearchOverTheWindow, TestAnAnswerThatIsGroupedIsRefusedWithWhyBecauseAReadingIsOneNumber, TestAReadingQueryThatIsNotOneIsRefusedWithTheGrammar, TestAMonitorSearchThatMerelyStartsWithTheWordReadingIsStillAMonitorSearch, TestAnAgentTriesAReadingBeforeItBecomesASignalAndIsToldTheQueryToPull, TestTheDatadogKeysNeverReachAReadingsBodyOrError, TestAPolledDatadogReadingIsANumberAMeasureReads, TestProbeSaysAReadingWithNoNumberCannotBeRead, TestAGrafanaReadingIsOneNumberAMinuteAndItsFirstPollOnlyMarksNowYetStillAsksGrafana, TestAGrafanaReadingWithNoSeriesOrNoValueIsAnEventWithoutANumberNotAZero, TestAGrafanaReadingOfManySeriesIsRefusedWithWhyBecauseAReadingIsOneNumber, TestAGrafanaReadingQueryThatIsNotOneIsRefusedWithTheGrammar, TestANewGrafanaSignalHearsAnAlertThatFiresAndNotOneThatResolvesOrIsPending, TestAGrafanaAlertPollHoldsToItsLimitAndTheNextPollGoesOn, TestTheGrafanaTokenNeverReachesAPollsBodyOrError, TestAnAgentTriesAGrafanaInstantQueryAndIsToldTheQueryASensorTakes, TestAPolledGrafanaReadingIsANumberAMeasureReads
