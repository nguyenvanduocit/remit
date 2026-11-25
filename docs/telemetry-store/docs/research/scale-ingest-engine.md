# telemetry-store: scale, ingest path and engine (research notes)

Notes, 2026-10-29. Not a decision record: nothing here is adopted until it lands in `design.md`, `tech-stack.md` or an ADR. Labels: **verified** = checked against a cited page this session; **inferred** = general knowledge, not checked; **proposal** = design call, not yet agreed; **LOW / MEDIUM / HIGH** = confidence.

## Inputs from the owner

- Peak write rate: **100k events/s**. The owner thinks this is a peak, not an average. The average is unknown.
- Instant statistics are the main need.
- Assumption: "80%" means 80% of events must reach the hot store and be queryable; the other ~20% (noisy logs, spans) may go to cold storage only or be sampled. Hot load is then about 80k events/s at peak. Not confirmed by the owner.
- A custom bridge into the store is acceptable.
- The company already runs StarRocks and BigQuery (skills `starrocks`, `bigquery`; MCP `mcp-server-starrocks-dev`).

## Sizing

100k/s is 8.64 billion events a day. The next two inputs are assumptions (LOW): **500 bytes per event** and **8x columnar compression**.

| Hot load, average vs 80k/s peak | Hot events/day | Stored/day (compressed) | 30 days |
|---|---|---|---|
| 20% | ~1.4 B | ~80 GB | ~2.4 TB |
| 50% | ~3.5 B | ~200 GB | ~6 TB |
| 100% | ~6.9 B | ~400 GB | ~12 TB |

At peak the log carries about 50 MB/s (100k × 500 B). Measure real event size and the real average before costing anything.

## Findings

### 1. Engine for instant statistics: ClickHouse over BigQuery (MEDIUM, inferred)

- BigQuery has a per-query latency floor of seconds, bills on-demand by bytes scanned (a wrong query written by an agent costs money), and has no OTel Collector exporter known to me (unchecked, LOW). `maximum_bytes_billed` caps cost per query but is one more guard to maintain.
- ClickHouse answers sub-second, ingests in near real time, and rolls up on insert with materialized views. Limits live in the database through a settings profile (`tech-stack.md:23`).
- BigQuery stays useful for rare heavy analysis over the Parquet archive; it does not exclude ClickHouse.

### 2. StarRocks is a real candidate (MEDIUM)

- The Collector has no StarRocks exporter that a web search found. For the sibling Apache Doris, the documented path is a bridge service that turns OTLP into JSON for Stream Load (verified, search result below). StarRocks likely needs the same (inferred). The owner accepts a bridge, so this no longer rules StarRocks out.
- StarRocks Primary Key tables upsert and partially update in place (verified, StarRocks docs). ClickHouse `ReplacingMergeTree` deduplicates only during background merges and cannot change sorting-key values (verified, ClickHouse docs).
- Join and benchmark comparisons found were vendor claims (CelerData, VeloDB): unverified. No independent source on wide-table scans.
- Reusing the existing StarRocks cluster for ~0.5 TB/day of continuous writes risks starving the revenue and orders queries it serves today. Capacity unknown (LOW).

### 3. One table for events, not for all seven classes (proposal)

- One wide append-only `events` table holds events, logs, spans and agent actions (`design.md:24`: logs and spans are events with a message or a duration).
- Metrics stay in `otel_metrics_*`. Entities and the identity graph are small and mutable and live elsewhere. Aggregates are separate tables built by materialized views and carry `definition_version`. External snapshots stay as received on R2.
- Typed columns for what is filtered and grouped often; the rest in a `props` JSON column. Promote a property to a typed column when the query log shows it is filtered often, not before. ClickHouse's native JSON type is production-ready from about 25.3 (LOW, check the version in use).
- The first sorting-key column is the costliest decision. `shop_id` first (`tech-stack.md:18`) suits per-shop queries; a global "last 5 minutes across all shops" scans wide. If global queries dominate, lead with time or add a projection on `(event_name, ts)`. Needs the owner's query mix.

### 4. Postgres is optional in v0 (inferred)

It holds only the identity mapping, schema/dictionary metadata and saved queries (the PII vault is already deferred, `tech-stack.md:33`). Alternatives: `ReplacingMergeTree` plus dictionaries in ClickHouse, or a small StarRocks Primary Key table. If raw PII must be read someday, it belongs in its own restricted store whichever engine is chosen (`design.md:66`).

### 5. Ingest path: durable log first, Parquet derived (HIGH for the structure, MEDIUM for thresholds)

- Parquet cannot be appended to row by row; the footer is written when the file closes. Writing Parquet at receive means buffering in memory (loss on a crash) or acking late. Frequent closes make many small files. Fixing both needs a write-ahead log in front, which is Kafka built by hand.
- A schema-typed file at ingest also fixes the schema too early; the original OTLP lets the schema change by replay.
- Proposal: the write path only appends to a durable log and acks. Every store is a consumer with its own offset. The cold consumer writes large Parquet files (hundreds of MB, hourly) to R2.
- At 100k/s peak a durable log is needed. It is the trigger `tech-stack.md:33` names for Kafka. A spiky peak makes the log the buffer: consumers can be sized near the average and lag briefly at peak, instead of buying a database sized for the peak.
- Kafka-compatible services backed by object storage (WarpStream, Redpanda tiered storage) may lower cost at higher latency (inferred; compare prices in a trial).
- Direct file writes are acceptable only at low volume with tolerated loss and minutes of delay. Neither holds here.

### 6. No Primary Key table for events (MEDIUM, inferred, to measure)

At billions of rows a day, the key index for tens of days of events is huge and 100k/s upserts are costly. Events are append-only. Deduplicate per batch: a consumer commits its log offset only after the load succeeds, with a load-deduplication token (Stream Load label, ClickHouse `insert_deduplication_token`). Primary Key tables suit only the small mutable data.

### 7. Instant statistics come from rollups (proposal)

Scanning billions of raw rows is neither instant nor cheap. Known figures (counts per minute, per shop, per event name) are summed on insert into `AggregatingMergeTree`-style tables. Raw is for deep and ad-hoc queries.

### 8. Not every event goes hot (proposal)

| Tier | Holds | Where |
|---|---|---|
| Hot | business events, errors, rollups | ClickHouse (or StarRocks) |
| Cold | all raw, including logs and detailed spans | Parquet on R2 |
| Dropped or sampled | low-value noise | edge, `sample_rate` column (`tech-stack.md:35`) |

## Signals (proposal; see `docs/adr/0013`)

Automation needs signals; signals need telemetry; telemetry needs data. Design runs the other way: the decision to automate, then the signal that triggers it, then the telemetry, then the data.

- A signal is a named aggregate (`definition_version`, `design.md:28`) plus a rule plus a consumer. It is declared in the repository beside the event JSON Schemas, checked in CI.
- Each signal states: the question it answers, its source events, its threshold and window, the agent and tool that react, and what it must never trigger. Start with three to five signals.
- A change ships with its signal (ADR-0013). A deploy is an event that names the change and the chain that owns it, so a bad reading wakes that chain.
- A reading is untrusted text until verified (`design.md:68`). Signals trigger proposals and wakes; they never lift a rail.
- Two loops, two scales: Remit's own loop (feedback, outcomes, logs) is thousands of events a day on its own records; the 100k/s store serves the products it builds.

## Proposed architecture

```
App → Edge (Cloudflare) → Collector (several, stateless)
                              ↓
                          Durable log (Kafka-compatible, days)
          ┌───────────────────┼───────────────────┐
   Hot consumer         Rollup consumer       Cold consumer
   large batches        sums on write         large Parquet files
          ↓                   ↓                     ↓
      Hot store           Rollup tables        R2 (long-term truth)
```

The hot consumer reads through one narrow source interface: today the Collector, later the log, with no change on the database side. Keep the OTLP-to-row mapping a pure function and the batch send the I/O shell.

## What would change in the existing docs if adopted

- `tech-stack.md:33`: Kafka (or compatible) moves from deferred to built.
- `tech-stack.md:18`: `ReplacingMergeTree` for `events` is replaced by an append-only table with batch-level deduplication; the sorting key is decided by the query mix.
- `tech-stack.md:31`: the Rust ingest gateway becomes the bridge/consumer and validates JSON Schema and sets `schema_status`; the `otel_logs` materialized view for `events` is no longer needed.
- `tech-stack.md:21` / `design.md:53`: Postgres is optional in v0.
- A new ADR records the log-first decision and the engine choice once confirmed.

## Open questions

- [ ] Real average rate versus the 100k/s peak, and the real average event size.
- [ ] Is "80%" the share of events that must be hot (the assumption above)?
- [ ] What counts as "instant": seconds or a minute, and is it allowed to degrade at peak?
- [ ] Per-shop or global queries: the sorting key depends on it.
- [ ] Shared StarRocks cluster or a dedicated one; spare capacity of the current cluster.
- [ ] ClickHouse versus StarRocks: decide with a trial on realistic data. Refuted if the representative queries exceed the latency threshold on a few tens of millions of rows, or if batched loading cannot keep up with the event rate.
- [ ] Does `opentelemetry-collector-contrib` have a StarRocks path? Not checked (LOW).

## Sources

- Apache Doris as OTel backend (bridge via Stream Load): https://oneuptime.com/blog/post/2026-02-06-apache-doris-opentelemetry-analytics-backend/markdown
- ClickHouse and OpenTelemetry: https://clickhouse.com/docs/guides/use-cases/observability/build-your-own/integrating-opentelemetry
- StarRocks Primary Key table: https://docs.starrocks.io/docs/table_design/table_types/primary_key_table.md
- ClickHouse updating data: https://clickhouse.com/docs/concepts/features/operations/update/overview.md
- CelerData comparison (vendor): https://celerdata.com/clickhouse-alternatives-comparisons
