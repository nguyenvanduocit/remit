# telemetry-store: tech stack v0

Draft, 2026-10-29. Builds on `design.md` and `research/*.md`. Labels: **verified** = checked against the cited page or source this session; **proposal** = my design call, not yet agreed.

## Principle

No custom code in the write path until a requirement forces it. The OTel Collector takes OTLP in and writes ClickHouse and R2. Rust is written only for what the Collector cannot do: the read interface and background jobs. Each deferred piece has a trigger (see Deferred).

## Stack

| Layer | Choice | Why | Status |
|---|---|---|---|
| Wire format | OTLP (HTTP + gRPC), OTel Events as `LogRecord` with `EventName` | `design.md` standard. The exporter's logs table has an `EventName` column. | verified |
| Ingest | OpenTelemetry Collector (contrib build): `otlp` receiver, `memory_limiter`, `redaction`, `clickhouse` exporter, `awss3` exporter | Transport, batching, retry, redaction and inserts come ready. | verified components |
| Durability in front of the store | `sending_queue.storage` pointing at the `file_storage` extension | Without it the exporter queue is in memory and is lost on restart. With it, delivery is at-least-once. | verified (exporter README) |
| PII at ingest | `redaction` with `allowed_keys` (fails closed) and `hash_function: hmac-sha256` | Keyed hash, so identifiers stay joinable across sources and are not guessable. | verified; processor is alpha for logs and metrics, beta for traces |
| Hot store | ClickHouse (managed to start), own DDL with `create_schema: false` | The exporter's default tables use plain `MergeTree` and `ORDER BY (toStartOfFiveMinutes(Timestamp), ServiceName, Timestamp)`, which does not lead with tenant or person. The README recommends managing the schema yourself for production. | verified |
| Typed events | One `events` table, `ReplacingMergeTree`, filled by a materialized view from `otel_logs` where `EventName != ''`. Order key starts with `(shop_id, …)` and includes `event_id`. Reads dedupe with `LIMIT 1 BY event_id`. | At-least-once delivery plus `MergeTree` means duplicates, so dedupe must live in the table. Replacing engines dedupe only at merge, and PostHog and Sentry Snuba accept the same eventual consistency (see research). | proposal |
| Raw archive | `awss3` exporter to R2 with `marshaler: otlp_proto`, path-style addressing, custom `endpoint` | Exact, replayable copy: replay is feeding the files back through the Collector. The exporter writes OTLP JSON or protobuf only, not Parquet. R2 compatibility is not tested. | verified formats; R2 MEDIUM |
| Cold analytics copy | Scheduled `INSERT INTO FUNCTION s3(…, format=Parquet)` from ClickHouse to R2 | Parquet where analysts and AI scan cheaply. ClickHouse documents Parquet writes to S3; R2 is untested. | verified function; R2 MEDIUM |
| Small relational store | Postgres: identity mapping, schema and dictionary metadata, saved queries | As in `design.md`. | proposal |
| Background jobs | Rust (`tokio`, `clickhouse` 0.15, `sqlx` 0.9, `jsonschema` 0.60, `object_store` 0.14): schema check, identity stitch, deletion, Parquet export | Typed, no runtime null or data-race classes, as the projects rules ask. | crate versions verified on crates.io, 2026-10-29 |
| AI read interface | Rust MCP server (`axum` 0.8) over a read-only ClickHouse user with a settings profile (`max_execution_time`, `max_result_rows`, `max_rows_to_read`, `max_bytes_to_read`), a schema-first describe tool, a query tool that returns SQL plus sample rows | PostHog enforces the same limits at the database and tool layer, not in the prompt (see `research/posthog.md`). | proposal |
| Schema governance | One JSON Schema per event name in this repo, CI check, generated dictionary. Violations are flagged (`schema_status` column) and stored, not rejected, at first. | Amplitude's "unexpected" state and Sentry's tolerant Relay show store-and-flag is safer until schemas settle (see research). | proposal |
| Edge | Existing company Cloudflare layer in front of the Collector's public OTLP endpoint | No separate tunnel or cache. | per owner's standing rule |

## Deferred (with triggers)

| Not built | Build it when |
|---|---|
| Rust ingest gateway (validate and reject at the edge, quarantine, dedupe at write) | Bad events pollute results even with `schema_status`, or a writer needs a synchronous reject. |
| PII vault in Postgres holding raw email or phone | Something must read the raw value (contact a person). For joins, the keyed hash is enough. |
| Kafka | Sustained volume where the file-backed Collector queue stops being enough. PostHog's own capture path needs Kafka; ours does not at v0 volume. |
| Sharded or replicated own cluster | Real volume and cost data say managed is too expensive. |
| Sampling | Volume demands it. The `sample_rate` column (default 1) exists from day one. |

## Known risks

- Collector components are beta or alpha (exporter: logs and traces beta, metrics alpha; redaction: logs alpha). Pin the Collector version and keep the config in this repo.
- `awss3` writes one object per batch (alpha). Many small objects need a later compaction job.
- Browser-side OTel logs and events support is not checked (LOW). Fallback: a thin first-party snippet that posts OTLP/JSON.
- ClickHouse exporter dedupe behavior beyond the table engine is not documented in the README (only the engine default is verified).

## Open decisions

- [ ] GDPR: delete the mapping only (`design.md`), or also delete events as Mixpanel and Amplitude do. Legal call.
- [ ] Hosting: managed ClickHouse first (assumed here) or own.
- [ ] Is the raw-PII vault needed in v0, or is the keyed hash enough?
- [ ] Rust for jobs and MCP server (assumed here, per projects rules).

## Sources

- OTel ClickHouse exporter README: https://github.com/open-telemetry/opentelemetry-collector-contrib/tree/main/exporter/clickhouseexporter
- Exporter DDL template: `exporter/clickhouseexporter/internal/sqltemplates/logs_table.sql` in the same repo
- `exporterhelper` persistent queue: https://github.com/open-telemetry/opentelemetry-collector/blob/main/exporter/exporterhelper/README.md
- `awss3` exporter: https://github.com/open-telemetry/opentelemetry-collector-contrib/tree/main/exporter/awss3exporter
- `redaction` processor: https://github.com/open-telemetry/opentelemetry-collector-contrib/tree/main/processor/redactionprocessor
- ClickHouse `s3` table function: https://clickhouse.com/docs/sql-reference/table-functions/s3
- Crate versions: https://crates.io/api/v1/crates/<name>, fetched 2026-10-29
