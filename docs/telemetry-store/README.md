# telemetry-store

One place that stores the data of every system: app and landing-page events, logs, traces, metrics, imported third-party data, and what the agents did. Storage is the product. Reading, analysis, dashboards and alerts are done by AI on top of it.

Status: design draft (2026-10-29). Nothing is built. Hosting, volume and retention are still open (see `docs/design.md`, section Open questions).

- `docs/design.md` — scope, data classes, standards, storage layout, AI read interface, security, open questions.
- `docs/tech-stack.md` — proposed stack: OTel Collector, ClickHouse, R2, Postgres, Rust jobs and MCP server; deferred pieces and open decisions.
- `docs/research/scale-ingest-engine.md` — sizing at 100k events/s peak, log-first ingest, ClickHouse vs StarRocks vs BigQuery; notes, not decisions.
- `docs/research/` — public engineering research behind both: `datadog.md`, `amplitude.md`, `posthog.md`, `honeycomb-mixpanel.md`, `segment-snowplow-sentry.md`.

First consumer: `../../shopify-fleet` (the agents' feedback loop). Any other system can write to it the same way.
