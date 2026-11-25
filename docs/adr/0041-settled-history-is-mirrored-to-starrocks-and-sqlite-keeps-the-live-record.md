# 0041. Settled history is mirrored to StarRocks, and SQLite keeps the live record

Status: proposed, 2026-11-02. Source: the owner's request to find a good database for the complexity Remit has reached,
and his choice of StarRocks. Builds on ADR-0015 (agents read stored data through `starrocks_query`); informed by
`docs/telemetry-store/docs/research/scale-ingest-engine.md` (engine trade-offs, to be re-read against this record).

## Context

The store is one SQLite file (`internal/store/store.go`: WAL, one writer connection, a reader per core). What it holds splits in
two kinds that need different engines:

- **Live state**: cards, grants, subscriptions, the inbox, secrets, and the `events` of a running turn. An event is also the
  message waiting in a card's queue (`queued`, `delivered`, `steered`, `dropped`), taken by `DELETE ... RETURNING`
  (`events.go:266`) and read by key (`card_id`, `id`). This needs transactions, row updates, one-file snapshots (`VACUUM INTO`)
  and an owner-readable file. Every query in `events.go` is a key or range lookup of this kind.
- **Settled history**: an event "changes while its turn runs and is history after: read when the owner opens its activity, and
  never changed again" (`pack.go`). Events are most of the database (dev database: `events` 1.6 of 2.7 MB), and the design keeps
  every session's record. Questions over it are analytic (failures by tool, cost of a chain, what firings did) and today run as
  `GROUP BY` and `json_extract` over SQLite (`events.go:357`, `signal.go:190`).

The company already runs StarRocks, agents already read it through `starrocks_query`, and the telemetry-store notes weigh it as
a first-class engine for event data. StarRocks as the live store is not an option: it is an OLAP cluster, with no cheap row-level
transaction for a queue and no single file for a snapshot.

## Options

1. Stay on SQLite and tune queries. Cheapest; history stays in the same file as the live state and grows with it.
2. DuckDB as an embedded replica. Rejected: `go-duckdb` needs cgo, while the module is pure Go (`modernc.org/sqlite`,
   `docs/design.md`); the purego drivers need `libduckdb` on the host. Either breaks the one-binary install for an analytic load
   that is two small queries today.
3. **StarRocks holds the settled history; SQLite keeps everything live.** Chosen.
4. Everything to Postgres. Gives up the one-file install and still does not make history analytic.

## Decision

- **SQLite owns every record. StarRocks holds a derived copy of the settled ones**, rebuildable by replaying SQLite. A concept
  has one owner (SQLite); the copy is never written by anything else and never read to decide what Remit does next.
- **What is mirrored:** settled events (not changed for the packing interval, `pack.go`), firings and readings. A live event, a
  queue entry and a secret are never mirrored.
- **How:** a scheduler job (`history-mirror`, the registry of ADR-0035) sends settled rows in batches, in id order, over the MySQL
  protocol `internal/source` already speaks, into a **Primary Key table** per kind keyed by `(id)`, so a resend after a crash
  replaces the row and ends in the same state (idempotent). Volume is thousands of rows a day, far under where the research
  (finding 6) found Primary Key upserts costly. The cursor (last mirrored id per kind) is a row in SQLite, so a replay starts
  from it and a rebuild starts from zero.
- **The seam:** `store` exposes `SettledSince(ctx, kind, afterID, limit)`; the consumer declares the interface it needs
  (`internal/history`, imports no other package of the module, as `internal/source`). Nothing else in the core knows StarRocks.
- **Reading:** agents use `starrocks_query` against the history database (ADR-0015, read-only by its guard); no new tool, no
  new screen. The Events tab keeps reading SQLite for the window it shows. A question over old history is asked of an agent.
- **Credentials:** the mirror writes with its own account (`HISTORY_STARROCKS_HOST`, `_USER`, `_PASSWORD`, `_DB`, read from the
  `-source-env` file with ADR-0015's rules), separate from the read-only `STARROCKS_*` the agents' tool holds, so no agent can
  reach a write credential. The write account has rights on the history database only.
- **Failure:** a mirror that cannot reach StarRocks logs it once per interval and retries from its cursor; nothing in the live
  path waits for it.

## Consequences

- Analytic questions leave the one writer's file; SQLite can later drop or pack mirrored history older than a window without losing it.
- A second system to run. The cluster is shared with revenue and order queries (research finding 2), so the mirror loads in small
  batches into its own database, and the owner confirms the cluster has room.
- A derived copy can lag SQLite; an answer from it states the cursor's time.

## Revisit when

- **Security controls** (not built; this record leaves the seams): a write account scoped to the history database, TLS to the
  cluster, retention and deletion of mirrored text that holds a customer's words, redaction of secrets in event data before it
  leaves the host. Threat modelling runs when the owner asks for hardening.
- Volume reaches where Primary Key upserts cost more than appends: move events to a Duplicate Key table with batch-level load labels.
- Telemetry at the scale of the research (100k events/s) arrives: the mirror reads from its durable log, not from SQLite.
- StarRocks capacity on the shared cluster is not enough: a dedicated cluster, or ClickHouse behind the same seam.
