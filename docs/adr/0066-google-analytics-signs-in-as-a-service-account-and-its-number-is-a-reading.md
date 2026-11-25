# 0066. Google Analytics signs in as a service account, and its number is a reading

Status: accepted, 2026-11-05. Builds on ADR-0034 (a plugin is a go-plugin process), ADR-0037 (a plugin that ships with Remit),
ADR-0046 (a number a source tracks is a reading), ADR-0047 (a sensor writes the signal) and ADR-0064 (a reading enters once). Source:
the follow-up ADR-0064 names under *Revisit when*, "a Google Analytics plugin", which the owner approved as the next plan.

## Context

The owner's web and app traffic lives in a Google Analytics 4 property: how many people came, from where, to which pages, and what they
did. An agent cannot read it today, and a measure (ADR-0045) cannot be written on it, because no source or plugin reads it. The Data API
v1beta (https://developers.google.com/analytics/devguides/reporting/data/v1) reads a property with three methods that only read:
`runReport` (metrics by dimensions over date ranges), `runRealtimeReport` (the last 30 minutes of a standard property) and `getMetadata`
(the metrics and dimensions a property takes, its custom ones included). Two questions had more than one answer: how the plugin signs
in, and what a sensor polls.

## Options

**Signing in.** Google's Data API takes an OAuth 2.0 access token for the scope `analytics.readonly`; it takes no API key for a property's
data.

| Option | Gains | Costs |
|---|---|---|
| A. **A service account's JSON key** | One value the owner pastes into the write-only inbox field (ADR-0017); the plugin signs a JWT with it and asks Google for a token (https://developers.google.com/identity/protocols/oauth2/service-account); no person's session to expire; the owner sees in Analytics which account reads | The owner makes the account in Google Cloud and adds its email to the property as a Viewer |
| B. The owner's own OAuth login (device flow, ADR-0053) | No Cloud project | A refresh token tied to a person, one more flow the server runs, and every read is the owner himself |
| C. An access token the owner pastes | Nothing to sign | It ends within an hour |

Chosen A. The key never leaves the plugin: the JWT it signs and the token Google answers are made per call and held nowhere.
`golang.org/x/oauth2/jwt` signs it (the module is already a dependency, through the MCP SDK); `golang.org/x/oauth2/google` is not used,
because its default credentials pull in `cloud.google.com/go/compute/metadata`, which the plugin does not need.

**What a sensor polls.**

| Option | Gains | Costs |
|---|---|---|
| A. **A reading (ADR-0046)**: one number Google computes now, each minute the sensor reads one event | The same shape as Datadog's and Grafana's readings, through the one `source.ReadingPoll`; a measure is a condition on `body.value`; a number Google revises in its first 48 hours is read again on the next period | A day's history is not replayed: a reading is a state |
| B. One event per settled day | A history, one event per day | A day's number still changes for 24 to 48 hours (https://support.google.com/analytics/answer/11198161), so an event would carry a number that later becomes false; the cursor would need the property's time zone; a second poll shape beside the reading |

Chosen A. A report reading is `reading:report metric=<name> [days=1] [lag=1] [<dimension>=<value>...]`, the `days` days that end `lag`
days ago in the property's time zone (Google resolves `NdaysAgo` there, so the plugin needs no zone); a realtime reading is
`reading:realtime metric=<name> [minutes=30] [<dimension>=<value>...]`. A dimension word keeps exact values (`country=United%20States`).

## Decision

- The bundled plugin `googleanalytics` (`internal/bundled/googleanalytics.go`, `googleanalytics_reading.go`), capability `customers`
  beside Amplitude (how customers use the product), with the variables `GOOGLE_ANALYTICS_SERVICE_ACCOUNT_KEY` (secret) and
  `GOOGLE_ANALYTICS_PROPERTY_ID`, and the read tools `report`, `realtime`, `metadata` and `reading`.
- One call signs in once: the key signs an RS256 JWT for `analytics.readonly` with the key's `token_uri` as audience, and the access token
  is the bearer of that call's one request. An error names the variable, the account's email and what Google said, never the key; a
  refusal of the property (403) says which email to add as a Viewer.
- A reading asks for one metric with `keepEmptyRows`, so a day without events is zero, not silence; no row at all is `value` null with
  `no_data` true. The body is `{query, kind, metric, where, from, to | minutes, time_zone, value, no_data, thresholded}`. One function
  (`googleAnalytics.read`) serves the tool and the poll, and the tool ends with the query a sensor takes.
- No shipped signal, no agent: which number of which property is the owner's to name, through the Assistant, as for Datadog (ADR-0046).

## Consequences

- An objective's numbers can be the property's: active users, sessions, key events, revenue, kept to a country, a page or an event.
- A default report reading reads yesterday, which Google may still be filling; `lag=2` reads settled days. The tool's schema says so.
- Every call asks Google for a token: one more request per call, and a poll every minute signs in every minute.

## Revisit when

- **Signing in on every call costs** (Google's token quota, latency): a token cache in the plugin process, keyed by the key's hash, held
  until shortly before `expires_in`. The seam is `googleAnalytics.api`.
- **A script needs the Data API** (a pivot, a cohort, a funnel the tools do not ask): `sdk.Auth` adds a stored credential and cannot mint
  a token; a scheme that signs a JWT with a variable and adds the bearer is the seam in the injecting proxy (ADR-0018).
- **One reading per group** (active users per country as several numbers): the grouped answer becomes several items with the group in the
  id, as ADR-0046 names for Datadog.
- **Not run against a live property.** The request and answer shapes are Google's documented ones; the tests use a fake that answers as
  the documentation says and checks the JWT's signature, issuer, scope and audience. The first live call (`googleanalytics_metadata` on the
  owner's property) is the check. Google documents that `keepEmptyRows` returns the rows whose metrics are 0 but only for data the property
  recorded (a `purchase` it never logged has no row); whether a report with no dimension over days without any event answers a zero row is
  not documented, and the plugin answers `no_data` when it does not.
- **The key is the account's for good.** A key the owner rotates in Google Cloud is pasted again; a key's age is not tracked.

Enforced by: TestGoogleAnalyticsReportSignsInAsTheServiceAccountAndAnswersTheRowsWithTheirTotal,
TestAGoogleAnalyticsNumberIsAReadingTheFirstPollProvesAndTheNextMinuteWrites, TestAGoogleAnalyticsReadingWithoutDataIsNullNotZero,
TestGoogleAnalyticsRefusesWhatIsNotAQueryOrACredentialBeforeAskingGoogle, TestGoogleAnalyticsSaysWhoNeedsAccessWhenThePropertyRefusesTheServiceAccount,
TestGoogleAnalyticsSaysTheKeyIsRefusedWithoutShowingIt, TestGoogleAnalyticsSaysThePropertysQuotaIsSpentAndToAskOnceLater.
