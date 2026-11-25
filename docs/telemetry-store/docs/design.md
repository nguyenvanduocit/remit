# telemetry-store: design v0

Draft, 2026-10-29. Labels: **verified** = checked against the cited page this session; **proposal** = my design call, not yet agreed.

## Intent

Agents and people need one place to see what happened across frontend, users, apps, backends and the agents themselves. The store keeps everything fast to write and cheap to keep. AI does the reading: it writes the queries, builds the rollups and answers the questions. The store stays small and boring so the AI layer can change every month without touching it.

## Scope

In v0:
- Ingest: accept OpenTelemetry (OTLP) from apps, landing pages and backends.
- Store: keep raw events, logs, traces and metrics, imported third-party snapshots, identity and properties, and documents.
- Serve AI: a read-only SQL interface, a data dictionary, and a place to save verified queries and derived results.

Out of v0: dashboards, alert rules, funnel and cohort engines, a query UI. AI produces these on demand and writes useful results back as derived data.

## Data classes

Three questions decide the class. Did it happen once (event)? Is it true now and can it change (entity)? Can it be recomputed from other data (aggregate)?

| Class | What it is | Changes in place? | Examples | Rule |
|---|---|---|---|---|
| Events | Something that happened: user behaviour, system, business, agent actions. Logs and spans are events with a message or a duration. | Never (append-only) | `product_viewed`, a deploy, a refund, "Release agent rolled back build 42" | Carry ids and event-specific attributes only. |
| Metrics | Numbers the instrumentation already aggregated | Never | crash-free rate, request latency | Produced at the source, not derived. |
| Entities | Current truth about a user, device, shop, agent | Yes | plan, locale, app version, email, phone | Keep history (valid-from, valid-to). Events reference an entity by opaque id. |
| Identity graph | Identifiers that belong to one person | Yes | email, phone, device id, cookie id, fingerprint mapped to `person_id`, with source, time, confidence | The stitching table. Required to join anything across sources. |
| Aggregates | Rollups, sessions, funnels, cohorts, user features, text digests | Rebuilt | daily active users, funnel conversion, LTV | Never a source of truth. Carries a `definition_version`. Rebuildable from events. |
| External snapshots | Third-party data stored as received | Replaced per run | GA4 export, store console reports, billing, cloud spend | Add `source`, `fetched_at`, `run_id`. Do not normalise early. |
| Documents | Unstructured text and blobs | Rarely | tickets, reviews, chat transcripts, replay files | Object storage, plus a pointer row so agents can find them. |

## Standards (what is used for what)

| Layer | Standard | Status |
|---|---|---|
| Wire format and data model | OpenTelemetry (OTLP: logs, traces, metrics) plus W3C Trace Context | An OTel Event is a `LogRecord` with a name (verified) |
| Event names and attributes | OTel semantic conventions: no dynamic values in names, ids in attributes, flat attributes, `error.type` on events that can fail | verified |
| Identity attributes | `user.id`, `user.hash`, `session.id`, `session.previous_id` from OTel. `device.id` and `shop.id` under our own namespace. | The OTel ones exist, all at Development status (verified) |
| Commerce event names | Shopify standard customer events, used as-is: `page_viewed`, `product_viewed`, `product_added_to_cart`, `cart_viewed`, `search_submitted`, `checkout_started`, `checkout_completed` and others. In-app events follow the same snake_case style. | 15 names listed (verified) |
| Raw storage schema | The OTel ClickHouse exporter tables: `otel_logs`, `otel_traces`, `otel_metrics_*` | Logs and traces beta, metrics alpha (verified) |
| Schema governance | One JSON Schema file per event name in this repo, checked in CI. The data dictionary is generated from them. | proposal |

Because the OTel parts are not stable: pin the semantic-conventions version and keep every attribute name in one mapping file, so a rename is a one-file change.

Not used: ECS (Elastic and OTel announced a goal to converge into one OTel-maintained schema; not finished, verified), CloudEvents, Snowplow/Iglu (from memory, not checked).

## Storage layout (proposal)

| Store | Holds | Why |
|---|---|---|
| ClickHouse (hot) | `otel_*` raw tables plus one derived, typed `events` view (ts, event_name, person_id, session_id, shop_id, device_id, props JSON) | Fast scans and cheap compression for events, logs and traces. Plain SQL that AI writes well. |
| Parquet on object storage (R2) | Raw archive of everything ingested, documents and blobs | Cheapest long-term copy. Lets us replay and rebuild every aggregate. |
| Postgres (small) | Identity graph, PII vault (encrypted, restricted), schema and dictionary metadata, saved verified queries | Small, mutable, relational data. |

The first slice is the app's own events plus errors through OTel and Sentry. GA4, store consoles and Shopify imports follow. Assumption: this slice order is mine.

## AI read interface (proposal)

- Read-only SQL through an MCP server, using a read-only database user with row and time limits and a query timeout.
- A data dictionary, generated from the JSON Schemas, that the agent reads first.
- Every answer returns the SQL and sample rows as evidence.
- Agents save a query once its result is checked. Derived results are written back as aggregates with a `definition_version`.

## Security

- PII separation: raw email, phone and fingerprints live only in the Postgres vault. The analytics store holds `person_id` and hashed identifiers. Deleting a person means removing the mapping, so events stay but no longer point to anyone.
- Each event carries a consent and PII class field from day one.
- Event text (user messages, error strings, tickets) is untrusted. The read path validates it before an agent sees it, because agents that read it can be prompt-injected.
- Agent actions form their own append-only audit stream: who acted, with which permission, which signal triggered it (OTel span link, from memory), and the outcome.

## Open questions

- [ ] Where does it run: own cluster (cheaper, we operate it) or managed ClickHouse (costs more, no ops)? Default: managed for the first month, then decide with real volume.
- [ ] Expected volume per day, and retention for hot versus archive.
- [ ] Who stitches identity: the ingest edge or a later job?
- [ ] Is a custom ingest service needed, or does the OTel Collector with the ClickHouse exporter cover v0? (PII split and identity stitching at write time may need custom code, written in Rust per the owner's preference.)
- [ ] Which data does the owner care about that fits none of the seven classes (session replay, experiments, AI conversation logs)?

## Sources

- OTel events: https://opentelemetry.io/docs/specs/semconv/general/events/
- OTel ClickHouse exporter: https://github.com/open-telemetry/opentelemetry-collector-contrib/tree/main/exporter/clickhouseexporter
- OTel `user.*`: https://opentelemetry.io/docs/specs/semconv/registry/attributes/user/
- OTel `session.*`: https://opentelemetry.io/docs/specs/semconv/registry/attributes/session/
- Shopify standard events: https://shopify.dev/docs/api/web-pixels-api/standard-events
- ECS and OTel convergence: https://opentelemetry.io/blog/2023/ecs-otel-semconv-convergence/
