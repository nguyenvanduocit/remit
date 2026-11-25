# Remit signal pipeline as built (2026-11-05)

These are internal findings from a read-only code map and a design critique of the remit repository. The coordinator spot-checked the citations marked (checked) itself.

## Shape
- ADR-0049: sensors sense, readings append to one log with a global `seq`, subscriptions hold `Match` (GLOB pattern), `SeenSeq` cursor, CEL `when`, `Window{Cooldown, Until}`, `Once`, one action (processors run/end/deadline/assist in code; tell/wake/start/hire through an agent; `plugin:<id>.<name>`). A firing is keyed (subscription, key, rev) and stored with its effect (`AddFiring ON CONFLICT DO NOTHING`).
- Routines, check-backs, watches and automations are all subscriptions plus a private clock sensor (`clock:<subid>`), told apart by four predicates. A routine runs one at a time and catches up once (Temporal BufferOne, unnamed).

## Entry points (not one router)
- Five write paths, three triggers: `Orchestrator.Emit` (door, CLI, poll sensor, emit_event, plugin Process emits) stores and only kicks; `emit` (script sensor, rollup, plugin Sense) stores and judges at once; `emitTransient` (clock, transient plugin readings) is never stored; facts call `tx.AddReading` in the caller's tx then `react`/`kick`; chain records go through `putSignals`. (checked: AddReading callers event.go:109, lifecycle.go:97, check_review.go:73, assist.go:27, signals.go:474.)
- In effect a transactional outbox: each producer appends in its own tx and the tick catches up by cursor (`SubscriptionsWithUnread`).
- One global mutex `tickMu` around emit, emitTransient and Tick (signals.go:66,100; schedule.go:67) (checked).

## Defects and risks
1. A firing that errors (e.g. missing processor) hits the `default` branch of `settle`, which only logs; no hold reason, retried every tick forever, absent from the board (signals.go:322-324) (checked; reproduced by probe test).
2. A false `when` records nothing; no tool answers "why didn't it fire".
3. Cooldown + tell: the core groups readings held during a cooldown, the tell processor refuses more than one reading (tell.go:49, chain_jobs.go:20), so the subscription is stuck for good with no reason (reproduced twice) (tell.go:49 checked).
4. Plugin `Process` runs under `tickMu` with no timeout (plugin/host.go:186-191, plugin_pipeline.go:244) (checked). One hung plugin stops all wakes and admissions.
5. Retention ignores cursors: `ForgetEventReadings` (store/signal.go:133) deletes by time, `ForgetReadings` (store/signal.go:197) keeps newest N; neither looks at `SeenSeq` (checked). And it runs only at start-up (request.go:178-183) (checked), so a long-running server keeps everything; facts are never trimmed.
6. A paused event signal still accepts a plugin sensor's readings (reproduced).
7. A Chair's wildcard watch (`rollup:*`) created later starts at SeenSeq=0 and hears history (reproduced); other creators start at `LastSeq` (three copies).
8. Plugin dedupe hits a unique index error instead of being treated as a retry, silencing the sensor (reproduced, latent); a transient plugin reading sent twice fires twice because its firing rev is `At` in ms and `At` is unset (reproduced, latent).
9. Some effects run outside the firing tx (fireEnd, fireRun, pluginProcessing, mark-live); fine only while each job is idempotent.

## Duplication and naming
- The "re-read; changed → errNotDue; AddFiring; PutSubscription" tx block is copied five times; "already fired" is handled three ways; "one reading per firing" checked in five processors.
- "signal" means both state (current value, keep newest N) and stream (events, keep by time); CEL exposes both `value` and `body`.
- "Reading" names a signal value (model/signal.go:114), the message an agent is reading (orch/agentstanding.go:135, reaching the browser as wire.Reading), and Datadog's `reading:` query (checked).
- "Routine" names every start subscription; "Event" means inbound events and chain events.
- Time lives in two places: clock sensors and subscription windows (`Until`, `Cooldown`).
- Rollup overlaps the Datadog reading sensor and runs its own poll loop outside the Sensor model.

## Doc drift
- design.md: "emit is where a reading enters" (false), routine stored in one tx (two txs, idempotent anyway), "quiet time" window (gone), `assist` processor missing, `reset_routine` (absent), "events kept 7 days" (only at start-up), paused signal takes no event (false for plugin sensors).
- ADR index statuses disagree with files for 0024, 0036, 0043, 0046, 0047; 0035 still relies on SeenRev; 0037 still "proposed".

## Ranked improvements proposed internally
1. Make non-firing visible: hold with reason, after N errors record a failed firing and move on (dead letter), `explain_subscription(sub, reading)` tool built on `signal.Probe`, recent holds/errors on the Subscriptions tab.
2. Timeout on plugin Process now; later run processors outside tickMu from an outbox row, ordered per subscription.
3. Declare signal kind state vs stream: retention, start position, envelope with `subject`.
4. Never forget past the lowest active cursor, or record "skipped N".
5. Start point (now, seq, time) and dry run of `when` over kept readings on create.
6. Rollup becomes a sensing kind or goes.
7. Name overlap policies; reconsider the 1:1 private clock sensor.
8. Fix ADR index/design.md drift; fitness test for status lines.
