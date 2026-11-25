# Research: Datadog as the source of an app's signals

Asked on 2026-11-03. The owner's need: his apps are tracked and monitored on Datadog, and what is monitored becomes a signal, so the
feedback loop is clear, stable and better. Decision it led to: [ADR-0046](adr/0046-a-number-datadog-tracks-is-a-signal-a-pull-whose-query-is-a-reading.md).
Builds on ADR-0024 (everything that comes in is an event signal) and ADR-0047 (a sensor reads), ADR-0036 (a signal holds readings), ADR-0045 (a measure is a signal and a condition).

Coverage: Datadog's API reference pages fetched on 2026-11-03 (listed under *Sources*); `internal/source/datadog.go` read whole.
Not run against a live Datadog organisation: no `DATADOG_*` variable is set in the development shell (the owner's vault was not looked in). Labels: **HIGH** read on a Datadog page or in our code, **MEDIUM** inferred from them, **LOW** not checked.

## The question

A feedback loop closes when a change is judged by what the running app does afterwards. ADR-0045 fixes the shape of that judgement:
a number, read from a signal, compared by a condition. What does Datadog offer as a source of such signals, and what has to be true of
an app for Datadog to have the number?

## What Datadog can say, and how it fits a signal

A signal is a stream of readings (ADR-0036). A Datadog fact is either **a number** (it has a value at each minute: a measure reads it)
or **an event** (it happens once: a rule reacts to it).

| Datadog fact | Endpoint | Kind | Scope | State |
|---|---|---|---|---|
| A metric, a log count or a RUM count over a window | `POST /api/v2/query/scalar` | number | `timeseries_query` (metrics) | **Built** (ADR-0046) |
| A monitor leaves OK (Alert, Warn, No Data) | `GET /api/v1/monitor/search`, `/monitor/{id}` | event | `monitors_read` | Built (ADR-0024) |
| A deploy or other change posted as an event | `GET /api/v2/events` | event | `events_read` | Tool only; a sensor's query is next (below) |
| An Error Tracking issue appears, regresses, changes state | `POST /api/v2/error-tracking/issues/search` | event | `error_tracking_read` | Not built |
| An incident opens or changes severity or state | `GET /api/v2/incidents` | event | `incident_read` | Not built |
| A security signal is raised | `POST /api/v2/security_monitoring/signals/search` | event | `security_monitoring_signals_read` | Not built |
| An SLO's SLI and remaining error budget | `GET /api/v2/slo/{id}/status` (public beta) | number | `slos_read` | Not built |

Facts per row (HIGH, each from its reference page):

- **Scalar query.** One request, one or more named queries and a formula; `from` inclusive and `to` exclusive in epoch milliseconds. A
  metrics query takes `query` and an `aggregator` of `avg`, `min`, `max`, `sum`, `last`, `percentile`, `mean`, `l2norm` or `area`. An
  event-based query (`logs`, `rum`, `spans`, `events`, `security_signals`, `ci_tests`, `ci_pipelines` and more) takes `compute.aggregation`
  (`count`, `cardinality`, `pc75` to `pc99`, `sum`, `min`, `max`), a `search.query` and `indexes`. The answer is a list of columns:
  `group` columns (the tags it was split by) and `number` columns with `values`. The page's example for an event-based query is RUM;
  the schema lists logs with the same fields but shows no log example (MEDIUM for the log request, HIGH for metrics and RUM).
- **Error Tracking search.** Body `data.attributes`: `query`, `from` and `to` (epoch ms), a `track` (`trace`, `logs` or `rum`) or a
  `persona`, `order_by`, `states` (`OPEN`, `ACKNOWLEDGED`, `RESOLVED`, `IGNORED`, `EXCLUDED`); at most 100 issues a request. An issue
  has `first_seen`, `last_seen`, `state`, `service`, `error_type`, `error_message`, `first_seen_version`, `last_seen_version`, and a
  `regression` object only when a resolved issue came back; the counts (`total_count`, `impacted_users`, `impacted_sessions`) are on the
  search result. Fits an event: a new issue (`first_seen` after the cursor) and a regression (`regressed_at`), whose version is the
  state and the time.
- **Incidents.** `GET /api/v2/incidents`, `page[size]` up to 100, `page[offset]`; attributes `title`, `severity` (`SEV-0` to `SEV-5`,
  `UNKNOWN`), `state`, `created`, `modified`, `detected`, `resolved`, and the customer impact fields. Public beta. Fits an event keyed on
  `modified`.
- **Security signals.** Search by `filter.query`, `filter.from`, `filter.to`, `sort`, `page.cursor`, `page.limit`; each signal has
  `message`, `tags`, `timestamp`. Cursor-paged, so a sensor can hold its cursor.
- **SLO status.** Path `slo_id`, query `from_ts` and `to_ts` in epoch seconds; answers `sli`, `error_budget_remaining`, `state`. Public
  beta, "subject to change". Fits a number, and its `error_budget_remaining` is the same SRE quantity ADR-0045 cites.
- **Auth for the new pages.** The reference pages list the OAuth scope names above; the keys our source sends are an API key and an
  application key (`DD-API-KEY`, `DD-APPLICATION-KEY`), and whether an application key suffices for Error Tracking, Incidents and
  SLO status is **LOW**: not on those pages. Our existing source reads logs, spans, metrics, monitors and events with the two keys.

### Ranking for the feedback loop

1. **Numbers first (built).** A measure needs a number; a monitor only says a threshold broke. The scalar query gives the number over
   any window, for metrics, logs and RUM, computed by Datadog, so the owner can reproduce it in Datadog's own notebook.
2. **Monitors (built).** Keep as the owner's pager: a threshold he already trusts becomes an event.
3. **Error Tracking issues.** The best event for "did my change break something": a new issue whose `first_seen_version` is the version
   just deployed. It names the version, which monitors and metrics do not.
4. **Deploy events (`/api/v2/events`).** Lets a rule place a change on the timeline (`sources:`, `tags:service:`); the loop can
   compare the number before and after a deploy (ADR-0045 *Revisit when*: a baseline).
5. **SLO status.** The cleanest measure when an SLO exists. Public beta today (LOW stability).
6. **Incidents and security signals.** Worth hearing, rarely a measure; build when asked.

## How an app gets into Datadog

A signal needs an app that reports. Datadog has no number for an app that sends nothing, and `datadog_reading` says so (`no_data`). This
is repository work for a chain, and it is the first key result of an objective (ADR-0045): the key result is the instrumentation and its
test; the measure follows once the signal reads.

| App | Route | What is tracked | Notes |
|---|---|---|---|
| A backend service | The Datadog Agent and a tracing library (`dd-trace`) | Request counts, errors and latency as `trace.*` metrics, spans, logs | The Agent runs beside the service; metrics name the service by the `service` tag, which a reading's query uses |
| Any process (a job, a CLI, a function) | `POST /api/v2/series` with an API key | Whatever the app counts: a gauge or a count, with tags | HIGH: the page's examples send only `DD-API-KEY`; points may be at most 1 hour old and 10 minutes ahead; 500 KB a payload. Needs no Agent, so it fits a serverless or one-off job |
| A browser app or a static page | The browser RUM SDK (`@datadog/browser-rum`) | Sessions, views, actions, errors, load times | HIGH: needs an `applicationId` and a `clientToken` from the Datadog app, and `site`, `service`, `env`, `version`; `sessionSampleRate` samples sessions, so a RUM count is of sampled sessions unless it is 100. The SDK sends to Datadog from the browser, which a rule of "no network request from the page" forbids (Lights Daily's first guardrail, ADR-0045 *Context*). The owner chooses: keep the rule and read plays elsewhere, or change the rule and measure plays through RUM |
| Remit itself | None built | | The system's own feedback loop (chains merged, goals met) is in its own database and signals; sending it to Datadog is a separate decision |

Cost and shape of what is sent (MEDIUM, general knowledge of Datadog, not read on a page today): every distinct metric name and tag
combination is a billed custom metric, so a tag with unbounded values (a user id) multiplies the bill. Name a few metrics, tag by
`service` and `env`, and keep ids out of tags. Datadog metrics reach the API with a delay of a minute or more, which is why a reading
ends at the last full minute and a short window can read light (ADR-0046 *Revisit when*).

## What a measure on Datadog looks like

The Chair tries the number with `datadog_reading`, then asks for the signal (the Assistant makes the sensor), then drafts the measure. The metric names, tags and RUM attributes below are illustrations: the app's own names go there.

| Measure | Sensor query | Condition |
|---|---|---|
| The app's error rate stays low | `reading:metrics window=1h agg=avg avg:app.error_rate{service:lights-daily,env:prod}` | `body.value <= 0.02` |
| No request fails in a day | `reading:metrics window=1d agg=sum sum:trace.http.request.errors{service:api}.as_count()` | `body.value == 0` |
| Error logs stay few | `reading:logs window=1h service:api status:error` | `body.value <= 20` |
| A named person played | `reading:rum window=7d @type:action @action.name:play` | `body.value >= 1` |
| The app still reports | any of the above | `!body.no_data` |

`body.value` is `null` and `body.no_data` true when nothing reports in the window (ADR-0046), so a measure on a silent app is
*unverified* in the dry run (ADR-0045), never met.

## Risks and open facts

- **Live shape of the log query** (MEDIUM): confirm with `datadog_reading` source `logs` on the first real organisation.
- **Application key scopes** (LOW for Error Tracking, Incidents, SLO): the key the owner enters must hold the read scope of each product
  a signal reads; a missing one is Datadog's refusal, shown on the signal's line after three failed reads.
- **Rate limits** (LOW, not read): one scalar query a minute per reading signal is small; a hundred reading signals are not. The seam is
  the sensor period and `sensorItemsMax`.
- **Public beta** (HIGH for Incidents and SLO status): Datadog says the shapes may change; a form built on them is a pinned shape, as
  `intercomVersion` is.

## Sources

- Scalar query: https://docs.datadoghq.com/api/latest/metrics/query-scalar-data-across-multiple-products.md
- Error Tracking: https://docs.datadoghq.com/api/latest/error-tracking/ and `/search-error-tracking-issues.md`
- Incidents: https://docs.datadoghq.com/api/latest/incidents/ and `/get-a-list-of-incidents.md`
- Security signals: https://docs.datadoghq.com/api/latest/security-monitoring/get-a-list-of-security-signals.md
- SLOs: https://docs.datadoghq.com/api/latest/service-level-objectives/ and `/get-slo-status.md`
- Submit metrics: https://docs.datadoghq.com/api/latest/metrics/submit-metrics.md
- Browser RUM setup: https://docs.datadoghq.com/real_user_monitoring/application_monitoring/browser/setup/client/
