# 0064. A reading enters once, is judged by a pure evaluator, and every outcome is recorded before it is delivered

Status: accepted, 2026-11-05; all eight steps below are built, in their order, each landing whole. Supersedes, of ADR-0049: the router
as `emit` judging at once, transient readings, the clock as a sensor, a firing stored together with its effect, and the cooldown that
groups readings; of ADR-0036: "a subscription's `Cooldown` stays" and `o.emit` judging inline; of ADR-0024: `Orchestrator.Emit` as the
one write path (it becomes the event signal's adapter over `ingest`). Builds on ADR-0014 (signal is the backbone), ADR-0035 (every
act Remit does on its own is a firing), ADR-0045 (a dry run proves a measure before Apply) and ADR-0062 (the Assistant decides
for the owner, he overturns). Research: `docs/reports/Signal pipeline prior art.md` and `docs/research_notes/Signal pipeline prior
art/` (the as-built map with its reproduced defects is `remit_current_state.md`). Source: the owner's question of 2026-11-05
("is there a plan to refactor the whole thing?") and his "ok" to the eight-step plan this record decides.

## Context

The customer is the owner, and through him every agent that sets up a subscription and expects it to act: a Chair's watch that
must wake it, an event rule that must tell a Chair, a routine that must start a chain. What they need from the pipeline is that it
acts, and that when it does not, it says why where they look. Today it decides things it never writes down:

- **A failing firing is invisible and retried for ever.** `settle`'s `default:` branch only logs (`internal/orch/signals.go:322-324`);
  no hold reason, no failed record, absent from the board. Reproduced with a missing processor.
- **A false condition leaves no trace**, and no tool answers "why didn't it fire".
- **Cooldown plus tell is stuck for good.** The core groups the readings held in a cooldown (`internal/signal/evaluate.go`,
  `readingOutcome`); `fireTell` refuses more than one (`internal/orch/tell.go:49`); the refusal falls into the log-only branch, the
  group never shrinks. The "one reading" check is copied in six processors (`tell.go:49`, `chain_jobs.go:20`, `assist.go:34`,
  `hire.go:59`, `teach_memory.go:34`, `start_event.go:19`): how many readings a firing carries has two owners.
- **One hung plugin stops the scheduler.** `pluginProcessing.Process` calls `p.o.runner.Process(ctx, …)` with the tick's context
  (`internal/orch/plugin_pipeline.go:244`) under `tickMu` (`internal/orch/schedule.go:67`), with no deadline.
- **Retention ignores cursors and runs only at start-up.** `ForgetEventReadings` deletes by age, `ForgetReadings` keeps the newest N
  (`internal/store/signal.go:133,197`); neither reads `seen_seq`; both run from `settleRequests` (`internal/orch/request.go:178-183`).
  Facts are never trimmed; rollup trims itself by count on each write (`rollup.go`, `rollupReadingsKept`).
- **A paused event signal still takes a plugin sensor's readings.** `Emit` refuses a paused signal (`internal/orch/event.go:95`), the
  private `emit` (`signals.go:62-83`) has no such check, and plugin, script and rollup readings go through it. Reproduced.
- **Readings enter by many doors.** `tx.AddReading` is called at `event.go:109`, `signals.go:72`, `signals.go:474` (`putSignals`),
  `lifecycle.go:97`, `check_review.go:73`, `assist.go:27` and `teach.go:82`, plus `emitTransient` that stores nothing
  (`signals.go:94-110`). Each decides pause, time and dedupe on its own; a plugin's duplicate hits the unique index as an error
  (`readings_dedupe`) and a transient plugin reading sent twice fires twice (its firing rev is an unset `At`). Reproduced, latent.
- **The decision and its effect are interleaved.** The "re-read, changed → `errNotDue`, `AddFiring`, `PutSubscription`" block is copied
  at `signals.go:410,479`, `tell.go:126`, `hire.go:159`, `automation.go:161` and `plugin_pipeline.go:266-279`; `errFiredAlready` is handled in
  six places. `fireEnd`, `fireRun`, the plugin processor and mark-live do their effect outside the firing's transaction.
- **Time lives in two places.** Every routine, check-back, goal deadline and shipped job is a subscription plus a private clock sensor
  `clock:<subid>` (`internal/orch/timewait.go`), told apart by predicates (`isRoutine` `routine.go:94`, `Subscription.Watch`
  `model/signal.go:253`, `isAutomation` `automation_tools.go:45`, `isEventSubscription` `subscription_tools.go:71`). A routine runs one at a
  time and catches up once (`clockSensing.Sense` keeps its instant while the firing is held, `sensing.go:132-167`): Temporal's
  BufferOne, unnamed.
- **Where a new subscription starts is decided in five places.** `watchedFrom` judges an exact key's newest reading and a pattern
  from now (`watch.go:124-128`, which fixed the wildcard watch the research reproduced starting at `SeenSeq = 0`); the others copy
  `LastSeq` (`subscription_tools.go:151,195`, `automation.go:97`, `help.go:233`). A sixth creator would decide it again.
- **"Reading" names two things**: a signal's value (`model/signal.go`, `Reading`) and the message an agent is on
  (`orch/agentstanding.go:135`, reaching the browser as `wire.Reading`, `web/src/rpc/define.ts:1133`).

What Remit already has that no platform studied has: one SQLite writer, so recording is exactly once with no fencing, and one log
with a global `seq`, so a cursor backfill and a dry run over past readings are a loop over the same evaluator.

## Options

The prior art (Grafana with Alertmanager, Temporal, Restate, Inngest, statecharts, Home Assistant; URLs in the report) converges on one
shape: a pure evaluator inside the coordinator, every outcome stored with a reason a machine reads, user I/O outside the coordinator
under a deadline it must have, a retry lifecycle that ends in a visible paused state, time as a stored due time, named overlap and
flow policies, retention that follows the slowest consumer. The whole-pipeline choice:

1. **Patch each defect where it shows** (a `WithTimeout` here, a `len != 1` there, a pause check in `emit`). Rejected: every defect
   above is a concept with two owners; a patch adds a seventh copy, and the next processor or writer reopens it.
2. **Adopt an engine** (Temporal or Restate as the scheduler). Rejected: Remit's log, cursor and single writer are already the
   stronger half; an engine adds a second store of truth and a process to run, and the owner configures through agents' tools, not
   an engine's UI.
3. **Rebuild the pipeline in place as ingest → log → pure evaluate → record → deliver → retention**, each stage one owner. Chosen.

Inside it, three choices had more than one defensible answer.

**Delivery order.**

| Option | Gains | Costs |
|---|---|---|
| A. One delivery worker for everything | Simplest | One slow processor delays every subscription: the outage moves, it does not end |
| B. **One at a time per subscription**, subscriptions in parallel | A subscription's firings keep the order of its cursor (a Chair is told in order); a hung processor costs its own subscription until its deadline (Bulkhead, vol. 03) | Two readings about unrelated subjects on one busy subscription wait for each other |
| C. One at a time per (subscription, subject) | Most parallel; Restate's per-key model | Needs `subject` from every writer first; Restate shows per-key order turning into per-key starvation unless the blocker is visible |

Chosen B. It is Kafka's partition-per-key with the subscription as the key (Competing Consumers, vol. 03: ordering holds only within a
partition). C is the *Revisit when*; `Reading.Subject` (step 1) is its seam.

**Where the policies live.**

| Option | Gains | Costs |
|---|---|---|
| A. **Named fields on the subscription** (`Flow`, `Overlap`, `Schedule`), interpreted only by `signal.Evaluate`; each processor declares what it takes (`ProcessorSpec`); one validator refuses impossible pairs at create | One owner for the policy (the core), one for arity (the processor), the pair checked once before it is stored (Inngest's registration failure, Temporal's Update validator) | The subscription record grows four small fields |
| B. The processor chooses how readings reach it | No new fields | The owner and agents cannot choose drop, delay or batch for one rule; the processor is again the second owner of a core rule |
| C. Shared named policy records that subscriptions reference | Reuse across rules | A new entity no one asked for; a change to a shared policy rewrites queued work (Temporal v2 freezes the policy on each buffered start for that reason) |

Chosen A, and each firing freezes the action and subscription revision it was decided under (`Firing.Do`, `Firing.SubRev`).

**Retention floor.**

| Option | Gains | Costs |
|---|---|---|
| A. Age or count only (today) | Bounded storage | A held or paused subscription loses the readings it has not judged: hold-in-place would silently lose work |
| B. One global floor, `min(seen_seq)` of every live subscription | One query | One paused subscription on `fact:teach.*` keeps the whole log |
| C. **Per key: a reading is kept while a live subscription whose `Match` lets its key through has not judged it**, or a pending or failed firing covers it; beyond that, by the signal's kind | Loses nothing anyone still needs, frees what no one needs; Postgres replication slots, with `max_slot_wal_keep_size = -1` as the default | A `GLOB` join in a periodic job; a forgotten paused subscription holds its keys' readings (shown, not capped) |

Chosen C, with no age cap by default (AGENTS.md: Remit caps none).

Mental models: **Inversion** (what guarantees a silent failure? any path where the evaluator decides and nothing is written; so the
outcome set is closed and every member is recorded), **Second-order thinking** (hold-in-place is only safe once retention respects
cursors, so the retention floor moves into step 3, ahead of the periodic job in step 6), **Gall's Law** (a working pipeline evolves
in steps that each leave it working, never a parallel rewrite).

## Decision

The pipeline has six stages, each with one owner: ingest, the log, evaluate, record, deliver and retention. The sections below
decide them, with time and the owner's surface beside them; the names are the stored shapes, and the build does not re-decide them.

### Ingest: one door for every write

`internal/orch/ingest.go` holds the only caller of `store.Tx.AddReading`:

```go
type Write struct {
	Key     model.SignalKey
	Subject string          // what the reading is about within its key (a pull request, a chain, a customer); "" = the key itself
	Value   json.RawMessage
	Dedupes []string        // the writer's ids for this occurrence, the first stored; a retry carries one of them (Emit's body hash
	                        // gives the current and the previous window)
	Cause   model.Cause
	Writer  model.Writer    // set by the calling code, never read from a payload
}
type Ingested struct { Reading model.Reading; Duplicate bool }
func (o *Orchestrator) ingest(ctx context.Context, tx store.Tx, w Write) (Ingested, error)
```

- `model.Writer{Kind WriterKind; ID string}`, `WriterKind` one of `sensor`, `door`, `cli`, `agent`, `plugin`, `fact`, `record`
  (a chain record), `schedule`; `ID` is the sensor id, the plugin id or the fact's noun. `model.Reading` gains `Subject` and `Writer`.
- In order: the key's writer may write it (a plugin only its own signals, the check now in `pluginProcessing.emit`); the signal is not
  paused and the writing sensor is not paused (`ErrSignalPaused`, nothing stored, the caller answers); `At` is ingest's `now`, never the
  writer's (a source must not move a window by naming a time; a producer's own time goes in `Value`); any of `Dedupes` already stored for the
  key answers the stored reading with `Duplicate: true`, an outcome and not an error (Inngest's event id); else the reading is appended.
- It runs in the caller's transaction: a fact or chain record is written with the change that caused it or not at all, which keeps
  today's transactional-outbox property. The caller kicks the scheduler after commit, as `react` does.
- `Orchestrator.Emit` stays the event signal's adapter (name, size, dedupes from sender id or body) and calls `ingest`; `emit`,
  `emitTransient`, `routeTransient`, `putSignals`' reading loop and the inline judging go. Only the Tick evaluates.
- Every reading is stored. Transient readings go (`Input.Fresh`, `Planned.Transient`, `freshFirings`, the SDK's `Emitter.Transient`
  and `SensorKind.Transient`, its proto fields reserved): a firing is recorded before it is delivered (*Record, then deliver*), so what it
  covers must be durable, and "not worth keeping" is retention's job (*Start position and retention*). The owner's ADR-0049
  wish, that a temporary sensor costs nothing to keep, is kept by retention, not by a second routing path.

### The log

Unchanged: one `readings` table, a global `seq` (`log_seq`), a per-key `rev`, the newest reading of a key never deleted (a `rev`
counts on from it). Each signal has a kind, `model.SignalKind` = `state | stream`, given by its key's family in one function,
`model.SignalKey.Kind()`: `event:*` and `fact:*` are streams (occurrences); `chain:*.mr.*`, `chain:*.sensor.*`, `chain:*.live` and
`rollup:*` are states (a current value). The kind sets the start positions a subscription may take and what retention keeps beyond
the floor.

### Evaluate: a pure function with a closed set of effects

`internal/signal` holds the whole decision, per subscription, with no I/O and no clock:

```go
type Input struct {
	Sub      model.Subscription
	Readings []model.Reading   // matched by Sub.Match, past Sub.SeenSeq, ascending by Seq
	Chain    *ChainFacts       // the owning chain's facts; nil for none
	Pending  int               // firings of Sub recorded and not yet settled
	Running  bool              // a chain Sub started is not done (overlap)
	Now      time.Time
}
func Evaluate(in Input) []Effect
func Apply(sub model.Subscription, effects []Effect) model.Subscription   // the one place a subscription's state moves
type Effect interface{ effect() }  // sealed: exactly these
type Fire    struct { Firing model.Firing; Readings []model.Reading }
type Skip    struct { Seq int64; Key model.SignalKey; Reason SkipReason; Detail string }
type Hold    struct { Reason model.HoldReason; Detail string }
type Due     struct { At time.Time }   // the next instant it acts with no new reading; zero: none
type Advance struct { Seq int64 }
type End     struct{}
```

`SkipReason` is `condition_false`, `condition_error` (the error text in `Detail`; an error counts as false, as SCXML's guard does, and
is recorded), `own_chain`, `collapsed`, `overlap`, `missed_catchup`. The order of evaluation is fixed and documented on `Evaluate`:
control (paused or ended: nothing), health (held: nothing, the cursor stays), key match, cursor, own-chain cause, condition, flow,
overlap, then `Fire`. A deadline that has come ends the subscription and fires it `expired` when nothing else fired.

The policies, fields of `model.Subscription`:

```go
type FlowPolicy string // each | delay | batch | last
type Flow struct {
	Policy  FlowPolicy
	Period  time.Duration // delay, batch, last
	MaxWait time.Duration // last: fires the newest reading at the latest this long after the first it collapsed; 0 = Period
}
type Overlap string    // queue | buffer_one
```

- `each`: every reading that holds fires alone (the default). `delay`: every reading fires alone, no sooner than `Period` after the last
  firing; none is lost (Inngest throttle). `batch`: at most one firing per `Period`, carrying every reading held, grouped by (key,
  subject) (today's cooldown). `last`: once readings stop for `Period` or `MaxWait` passes, the newest of each (key, subject) fires and
  the others are `collapsed` (Inngest debounce). Dropping readings (rate limit) is not built: no rule asks to lose readings.
- `queue`: every firing is delivered, one at a time, in order (every action but `start` from a schedule). `buffer_one`: while a run the
  subscription started is at work or a firing waits, one more firing waits and every further one is `overlap` (a routine: Temporal's
  BufferOne, today's behaviour, by name).
- Each processor declares what it takes, `orch.ProcessorSpec{Takes model.ProcessorTakes; Deadline time.Duration}` with
  `model.ProcessorTakes{Readings Takes; Expires bool}` and `Takes` = `one | many`. `Processing` gains `Spec() ProcessorSpec`; a plugin's processor is `one`, `Expires: true` (its protocol carries one
  reading, or the key on expiry).
- `model.Subscription.Check(t model.ProcessorTakes) error` is the one validator, run by the one create and update path: refuses
  `batch` for a `one` processor, a deadline for a processor that does not expire, an action no processor serves, a CEL expression that
  does not compile, and a subscription with neither or both of `Match` and `Schedule`. It answers the agent at once, with the reason.
- `Subscription.Window` goes: its cooldown is `Flow`, its deadline the field `Until time.Time`.
- `Firing` gains `Seqs []int64` (the readings it covers; a batch by subject is not a contiguous range of revs) in place of `FromRev`.

### Record, then deliver

**Record** runs under `tickMu`, one short transaction per subscription per pass (`internal/orch/record.go`): it re-reads the
subscription once (its revision is the one evaluated, else the pass reads it again), stores each `Fire` as a firing `pending`, each
`Skip` as a near-miss, and `Apply`'s subscription. It is the only caller of `AddFiring`. `AddFiring`'s conflict is a duplicate, counted.

```go
type FiringStatus string // pending | done | failed | skipped | cancelled
type FiringCause  string // reading | expired | scheduled   (replaces Firing.Expired)
// model.Firing gains:
	Status      FiringStatus
	Cause       FiringCause
	Seqs        []int64
	Do          SubscriptionAction // the action as decided; a later edit does not rewrite queued work
	SubRev      int64
	Attempts    int
	NextAttempt time.Time
	LastError   string
	Waiting     string             // why delivery waits without failing: admission, the last run at work, the quota
```

Near-misses are rows of `near_misses(subscription_id, seq, reason, at, doc)`, the newest `nearMissesKept = 20` per subscription, trimmed
in the same transaction (Home Assistant's separate "did not trigger" traces, so near-misses never evict firings); `own_chain` is only
counted. Counts live on the subscription: `Tally{Fired, Failed, ConditionFalse, ConditionError, OwnChain, Collapsed, Overlap,
MissedCatchup, Duplicate int64}` (Temporal's describe counters).

The subscription's state is three small enums, not one flattened set (the statecharts note's 18 combinations):

```go
type Control string // active | paused | ended        (replaces Paused bool and Once)
type HoldReason string // firing_failed | processor_missing
type Hold struct { Reason HoldReason; Detail string; Firing FiringKey; Since time.Time; By model.Actor } // nil: healthy
type FiringKey struct { Subscription string; Key model.SignalKey; Rev int64 }
// the window is DueAt (*Time is the subscription's own*)
```

**Deliver** runs outside `tickMu` (`internal/orch/deliver.go`). After a record pass the scheduler starts one worker per subscription
that has a `pending` firing whose `NextAttempt` has come, whose control is not `paused` and which is not held; a worker delivers that
subscription's firings one at a time in `(at, seq)` order, and stops when none is due. A restart finds the pending firings and delivers
them: the cursor moved when the firing was durably recorded, not when the effect succeeded (Restate's Kafka offset).

```go
type Delivery struct {
	Firing    model.Firing
	Readings  []model.Reading
	RequestID string // FiringKey's text: the effect's idempotency key
	Now       time.Time
}
type Delivered struct {
	Outcome string                                            // what it did, in words, for the board
	Chain, Card string
	Write   func(ctx context.Context, tx store.Tx) error      // the effect's writes, committed with the firing's `done`
}
// Processing:
	Spec() ProcessorSpec
	Deliver(ctx context.Context, d Delivery) (Delivered, error)
```

- A processor's I/O (a plugin's `Process`) runs inside `Deliver` under `context.WithTimeout(ctx, spec.Deadline)`; its database writes
  are `Write`, which the worker commits in one transaction with the firing's `done`, its subscription's `Tally` and, for an ended
  subscription with nothing left pending, its deletion (a chain's) or its keeping (Remit's, `keptWhenEnded`). A database effect
  therefore happens once; an outside effect is at-least-once and keyed by `RequestID` (a plugin is given it as `ProcessRequest.request_id`;
  a `start` names its chain from the firing, as `fireStart` does today; a `tell` queues its message under it).
- `Deadline` is required: the registry refuses a zero. The plugin deadline being built now as a constant on `plugin_pipeline.go:244`
  becomes the plugin processor's `Deadline`.
- What is not a failure waits: admission (no provider), the start quota, a `buffer_one` run at work set `Waiting` and leave the firing
  `pending`; the next kick tries again. The in-memory `holds` map goes; the reason is stored.
- A failure retries with backoff (`deliveryBackoff` from 30 s doubling to 15 min, `deliveryAttempts = 5`); an error a processor marks
  permanent (`errPermanent`: a missing processor, a refused parameter) fails at once. A failure that is final is settled by the pure
  `signal.Settle(sub, firing, err, now) (model.Subscription, model.Firing, []Effect)`: the firing is `failed` with `Attempts` and
  `LastError`, the subscription gets `Hold{firing_failed}` **in place** (its cursor and its later firings wait behind it), and
  `fact:subscription.failed` is ingested in the same transaction with the subscription, the firing key and the error. It is not
  dead-lettered: Restate and Temporal park failing work and keep its position, so a fix resumes it and loses nothing (step 3's
  retention floor is what makes that true).
- A hold is lifted by `resume_subscription` (retry: the failed firing goes back to `pending` with `Attempts = 0`) or by `skip_firing`
  (the firing is `skipped`, the next one goes), each with a `why` recorded in `Hold.By` and a `fact:subscription.resumed`. Who may: the
  subscription's owner (a chain's Chair for its own; the Assistant for Remit's and the owner's, ADR-0062). A hold is health, not a
  rail: pause and stop stay the owner's and the Assistant's alone.
- Deleting a subscription (by its owner, or with its chain) marks its `pending` firings `cancelled` in the same write.

### Time is the subscription's own

```go
type Schedule struct {
	Cron          string        // five fields or @every; "" = once, at At
	At            time.Time
	CatchupWindow time.Duration // how late a missed instant may still fire; 0 = no limit (today)
	Last          time.Time
}
// model.Subscription: Schedule *Schedule; DueAt time.Time   (Once goes; Until replaced Window.Until in step 2)
```

`DueAt` is the one stored next instant, the column `due_at` the scheduler already indexes (`subscriptions_due`): the earliest of the
schedule's next instant, `Until`, and the flow's release; `Apply` computes it from `Due`, and `Subscription.Due()` returns it. A scheduled firing has `Cause: scheduled`,
key `model.ScheduleKey` (`schedule:<subscription id>`) and rev the instant in Unix milliseconds, so a retry fires once and the board
shows what time caused it. A missed instant older than `CatchupWindow` is `missed_catchup`; after downtime one firing catches up and
the next instant counts from now (today's `ClockSensed`). A routine is a subscription with a `Schedule`, `Overlap: buffer_one` and a
`start`; a check-back and a goal deadline a one-off `Schedule`; a shipped job a cron `Schedule` and a `run`. The clock sensor kind,
`clockSensing`, `waitFor`'s sensor half, `endClock`, `ClockKeyPrefix` and the kind predicates go; the board's kind is read from the
action and owner by one function. A clock shared by several subscriptions is not built: nothing asks for one.

### Start position and retention

- The one create path is `func (o *Orchestrator) subscribe(ctx, tx store.Tx, sub model.Subscription, from model.Start) (model.Subscription, error)`,
  with `model.Start{From StartFrom; Seq int64; At time.Time}`, `StartFrom` = `now | seq | time | current`. It is required: each tool
  says its own default (a watch and an event rule `now`). `current` is for one exact key of a state signal (its newest reading is
  judged); on a pattern it is refused. An update keeps the cursor (fix and resume, the point of holding in place) unless it names a new
  start: Grafana's reset-on-edit is rejected because it would drop what a held subscription waits to redo.
- Retention is a shipped subscription, `sub_run_forget`, a `run` of job `forget` every 15 minutes. A reading is deleted only when all
  hold: it is not the newest of its key; no live (not `ended`) subscription whose `Match` lets its key through has `seen_seq` below it;
  no `pending` or `failed` firing covers it; and it is past its kind's history: a stream's `eventsKept` (7 days), a state's newest
  `stateReadingsKept = 1000`. Done and skipped firings go after `eventsKept`; pending and failed never. `ForgetEventReadings`,
  `ForgetReadings`, rollup's own trim and the forget in `settleRequests` go. Facts are streams like any other.

### What the owner and agents see

Tools for the Assistant, and for a Chair on its own chain's subscriptions (capability `signals`): `describe_subscription` (cursor
against head and unjudged count, control, hold, `DueAt`, the last firing, pending and failed firings with attempts and next attempt,
recent near-misses, `Tally`); `explain_subscription(id, seq)` (`Evaluate` on that one reading: the effects and, for a condition,
`signal.Probe`'s reason); `dry_run_subscription(subscription or draft, start)` (the evaluator over the kept log from the start, effects
returned and none run, at most `dryRunReadings = 5000` per call and the output says where it stopped and that chain conditions read
today's facts); `create_subscription` and `update_subscription` take `dry_run: true` and answer the dry run instead of storing
(ADR-0045's rule for measures, extended); `resume_subscription` and `skip_firing` as in *Record, then deliver*. A dry run takes no lock. On the
Subscriptions tab each line shows its control, its hold with the failed firing and error, what delivery waits on, and its near-misses;
a held subscription counts on the tab and in what waits for the owner, which the Assistant takes first (ADR-0062).

### Cleanup

The agent-standing `Reading` becomes `OnMessage` (`orch.OnMessage`, `wire.OnMessage`), as the owner decided on 2026-11-05; `Reading` names only a signal's value. Datadog's
`reading:` query keyword stays: it is a word of a query language, not a type. `docs/design.md` and this index are aligned, and a fitness
test checks that each row's status in `docs/adr/README.md` is its file's status line.

Rollup is removed, as the owner decided on 2026-11-05: the `rollup:*` source, its adapter (`rollup.go`), the flag
`-telemetry-rollups` and everything that renders or names it. `docs/telemetry-store/README.md` says "Nothing is built", the contract
`rollup.go` read was "assumed", and the adapter ran its own poll loop, timeout, silence count and trim beside the sensor model. When the
store exists, a poll sensor on a `telemetry` source is one sensor kind; the alternative weighed, that sensor kind now, would have kept
chains' rollup watches working against a store no one runs.

## The steps

Each step lands whole (callers, migration, deleted old code, tests, `docs/design.md` for what it lands) and leaves the pipeline working.
Migrations: the last at writing is 89 (`migrations_duels.go`); 90 may be taken by the agent-learning session's coach. This refactor's
first migration is 91 or the next free number when the step is built; each step takes the next free number then, none is reserved.
A migration that reaches the owner's database is preceded by the backup AGENTS.md names.

**Peer areas.** The agent-learning session (ADR-0063) owns `teach.go` and `teach_memory.go` (`fireTeach`, the `teach` job), the
`teach.*` fact keys (`fact.go:31-36`), `assist.go`'s overturn path, the four shipped `teach` subscriptions (`automation.go`, `teach(…)`)
and the teach line in `signalboard.go`; its coach may touch `automation.go` and `subscription_tools.go`. The assist slice (`fireAssist`,
`recordAssist`, `sub_assist_owner`, `keyOwnerAsked`, `ActionAssist`) is finished and its tests must stay green. `fireTeach`, the teach
subscriptions and `fireAssist` are callers that steps 1 to 4 rewire like any other processor or writer; **each of those steps messages
the agent-learning session before touching those files and waits for its answer.** `sdk/` and `internal/plugin` changes (step 1's
transient removal, step 4's `request_id`) wait until the plugin `Process` deadline being built now has landed, and are agreed with
whoever then works there.

| Step | What lands | Migration (stored data) and how it is verified | Gate: tests written first, failing before |
|---|---|---|---|
| 1. Ingest | `ingest`, `Write`, `Writer`, `Reading.Subject`; every `AddReading` caller moved (`event.go`, `signals.go` ×2, `lifecycle.go`, `check_review.go`, `assist.go`, `teach.go`); transient readings removed; only the Tick evaluates | None: `subject` and `writer` are new optional fields of a reading's document | `TestAPausedEventSignalTakesNoReadingFromAnyWriter` (defect 6), `TestAReadingWrittenTwiceIsStoredOnceAndAnsweredAsADuplicate`, `TestAFactIsStoredOnlyIfItsCallersTransactionCommits`, fitness `TestOnlyIngestWritesReadings` |
| 2. Evaluate | `signal.Evaluate`, `Effect`, `Apply`, `Flow`, `Overlap`, `ProcessorSpec`, `Subscription.Check`, `Firing.Seqs`; the six arity checks, the six tx blocks, the `errFiredAlready` handlers and `errNotDue` returns in processors go | Each subscription's `window.cooldown` becomes `flow`: `batch` for a `wake`, `delay` for every other action (none loses a reading), `each` where there was none; `window.until` becomes `until`; `firings.fromRev` becomes `seqs` from the readings still stored. Verify: subscription and firing counts unchanged; count with a cooldown before = count with a flow other than `each` after; no document keeps `$.window` or `$.fromRev` | `TestACooldownTellIsDelayedAndNeverStuck` (defect 3), `TestCreateRefusesABatchForAProcessorThatTakesOneReading`, `TestEvaluateTakesItsStepsInTheDocumentedOrder`, `TestEachFlowFiresWhatItNames`, fitness `TestOnlyTheRecordStageAddsFirings` |
| 3. Lifecycle | `FiringStatus`, attempts, `Hold`, `Control`, `Tally`, `near_misses`, `signal.Settle`, `fact:subscription.failed`, `resume_subscription` retry and `skip_firing`; the start-up forget gains the floor of *Start position and retention* (cursors and pending or failed firings) so a hold loses nothing | `firings` gains `status` (every stored firing `done`, `attempts` 1, `cause` from `expired`); `subscriptions` gains `control` from `paused` (an over routine, `ended`), `paused` goes and the indexes that read it (`subscriptions_key`, `subscriptions_due`) are rebuilt on `control`; table `near_misses`. Verify: firing count unchanged and all `done`; subscription count unchanged; paused count before = `control='paused'` after | `TestAFiringThatKeepsFailingHoldsItsSubscriptionWithTheReason` (defect 1), `TestAHeldSubscriptionLosesNoReadingAndResumesWhereItStopped`, `TestAFailedFiringIsAFactASubscriptionHears`, `TestAFalseConditionLeavesANearMiss`, `TestForgettingKeepsWhatALiveCursorHasNotJudged` |
| 4. Delivery | `deliver.go`, `Delivery`, `Delivered`, per-subscription workers outside `tickMu`, required `Deadline`, `RequestID` to every processor and `ProcessRequest.request_id`, `Waiting` in place of `holds` | None beyond step 3's | The crash test first: `TestAFiringRecordedBeforeACrashIsDeliveredOnceAfterTheRestart`; then `TestAHungProcessorCostsOnlyItsOwnSubscription` (defect 4), `TestTheRegistryRefusesAProcessorWithNoDeadline`, `TestTwoFiringsOfOneSubscriptionAreDeliveredInOrder`, `TestEveryProcessorIsGivenTheFiringKeyAsItsRequestID` |
| 5. Time | `Schedule`, `DueAt`, `Overlap` on routines, `CatchupWindow`, `ScheduleKey`; clock sensors and `timewait.go`'s sensor half removed | Each `clock` sensor becomes its subscription's `schedule` (cron, one-off `at`, `last`), its `clock.next` the subscription's `due_at`, `match` and `$.once` cleared; routines `overlap: buffer_one`; the sensors deleted. Verify: clock sensors before = subscriptions with a schedule after; every routine's `due_at` equals its sensor's `next` (migration test on a fixture, and `list_routines` on the dev database before and after shows the same next times); no `kind='clock'` sensor remains; subscription count not down | `TestTheTimeMigrationKeepsEveryNextInstant`, `TestABufferOneRoutineRunsOnceAfterItsRunEndsAndCountsTheSkips`, `TestAMissedInstantPastTheCatchupWindowIsCounted`, and the routine tests in `routine_test.go` kept green |
| 6. Kind, start, retention | `SignalKind`, `Start`, the one `subscribe` path (the `LastSeq` copies go), `sub_run_forget` | None: a start is applied at create, not stored; the shipped subscription is seeded by `SeedSubscriptions` | `TestALateWildcardWatchStartsAtNow` (defect 7, pinned on the one path; it passes today through `watchedFrom`), `TestForgettingRunsWhileTheServerIsUp`, `TestAStateKeepsItsNewestAndAStreamItsWindow`, `TestCurrentIsRefusedOnAPattern` |
| 7. Surface | the four tools, `dry_run` on create and update, the Subscriptions tab lines and counts, a `web/verify` page with a held, a waiting and a near-missing subscription | None | `TestDryRunSaysWhatASubscriptionWouldHaveDoneOverTheLog`, `TestExplainSaysWhyAReadingDidNotFire`, `TestAHeldSubscriptionWaitsForTheOwner`; looked at in a browser as AGENTS.md says |
| 8. Cleanup | `OnMessage`; rollup removed; `docs/design.md` and the index aligned | None: its readings age out as a state's. Verify: no subscription, reading or firing of `rollup:*` on the database it reaches | fitness `TestEveryIndexStatusIsItsRecordsStatus` |

Every step keeps green, beside its own tests: `TestAFiringIsStoredOnceWhoeverRetriesIt` (`internal/store`); the assist tests
`TestAChairsQuestionReachesTheAssistantFirst`, `TestAStuckAssistantLeavesItToTheOwner`, `TestTheOwnerOverturnsAnAssistantsDecision`
(`internal/orch/assist_test.go`, named here as the gate for steps 2 to 4); and the teach tests in `internal/orch/teach_test.go`.

**Step 3 as built** (migration 91), where it differs from the text above:

- A retry is the next evaluation, not a delivery worker (step 4). A firing is stored `pending` only once an attempt failed; the next
  pass derives the same `Fire`, finds that firing by its key and tries it again once `NextAttempt` has passed. After
  `signal.DeliveryAttempts` (5) attempts, 30 s doubling, or at the first for a `signal.Permanent` error (a missing processor, an
  action only Remit's own subscriptions hold), it is `failed` and the subscription holds in place.
- `Hold` has no `By`: who lifted a hold is in `fact:subscription.resumed` (with how and why) and the subscription's `By`.
- What waits rather than fails (admission, a start whose last chain is at work, a pending firing whose next attempt has not come)
  is said in the in-memory `holds` until step 4's `Waiting` replaces it; a wait stores no firing and counts no attempt, so a
  teaching is still admitted once.
- No `scheduled` cause, `Firing.Do` or `Firing.SubRev` yet: causes are `reading` and `expired`.
- The migration ends every Remit clock subscription whose clock has no next instant, not only routines; a paused one stays
  `paused`. A routine that does not repeat is `ended` after its run, and `withWords` revives it.
- Skipping the only firing of a `Once` subscription ends it, as firing it would have. A pending firing the subscription no longer
  derives (it changed meanwhile, or decided another) is `cancelled`; deleting a subscription cancels its open firings and deletes
  its near-misses.
- The floor is in every forget that exists today (`ForgetEventReadings` at start-up, `ForgetReadings` in the sensor and rollup
  trims), as option C says, not only in the start-up one.
- A Chair lifts holds on its own chain's subscriptions with resume_subscription and skip_firing, given to every Chair beside its
  watch tools; the Assistant lifts any.
- `CountFirings` and `WakesSince` count `done` firings only; a near-miss's `At` is the evaluation's time.

**Step 4 as built** (no migration: `Firing.Waiting` is a new optional field of a firing's document), where it differs from the text
above:

- `Delivered` has no `Write`, and `Deliver` answers only an error. Several effects are a chain's step (`apply`, `messageChair`,
  `sendToStandalone`), whose transaction is the chain's, under its lock; the processor stores its effect with the firing done itself, the
  done carried into that transaction as the change's `records.fired` (or through `storeDone` for an effect that is the store's alone).
  The rule holds as written: the effect and the firing's `done` (with `Tally.Fired` and, for an ended chain's subscription with nothing
  left pending, its deletion) commit together or not at all.
- `Delivery` also carries the subscription as the worker read it and the room admission weighs. With no `Firing.Do` yet, a firing is
  done by its subscription's action as it is when it is delivered.
- A recorded firing is queued work: the record stage no longer cancels a pending firing the subscription would not decide again
  (that cancel goes); only deleting the subscription, or setting a wait of it again (`waitFor`, through
  `store.Tx.CancelPendingFirings`: a goal check's deadline moved on resume answers its new instant, not the one that came while the
  chain was paused), cancels its open firings. One cancelled while its own effect deleted the
  subscription (a goal check's deadline ending the check) is stored `done`, since the effect is done.
- A processor that finds nothing to act on (`errNothingToDo`) has its firing stored `done`, saying so.
- The cursor and a clock move on at record time, so what is decided and not delivered shows on the Subscriptions tab from its firing:
  a routine's run is due at the time it was decided, a firing that waits holds its line with the reason it stored (`Firing.Waiting`),
  and a failed attempt waiting out its backoff holds nothing (its firing's row says why it failed) until it fails for good.
- A firing queued by a start is admitted on the Tick its delivery kicks, not in the Tick that recorded it.
- A wake is delivered outside the Tick, after the Tick's queued starts, so two rules the Tick gave by its order are kept by admission:
  a due wake's slot is promised to it while its firing waits for delivery (`promisedSlots`, a queued start leaves it free), and the
  entries that take a slot (the Tick's starts, `fireWake`) take it one at a time under `slotMu`, so two cannot take the last one. A
  wake's apply that waits for its chain's lock makes the Tick's starts wait with it.
- The worker delivers at the time of the pass that started it; a failed attempt's backoff counts from when it failed (a deadline after
  it began). The deadline is applied around every processor (`process`); the plugin's own error names the processor that did not answer.

**Step 5 as built** (migration 92), where it differs from the text above:

- `Subscription.Due()` is `DueAt`, or `Until` when that comes first or `DueAt` is not counted yet, so a subscription made with a
  deadline is due by it without its maker copying it into `DueAt`. What the owner reads as a line's next time (`dueOf`) is the
  schedule's `DueAt` or the deadline, never a flow's release.
- An instant a schedule skips (`missed_catchup`, `overlap`) is counted in `Tally` only. A near-miss is keyed by the reading it judged
  (`near_misses` has the key `(subscription_id, seq)`), and a scheduled instant is no reading.
- `Subscription.Check` refuses a schedule with a condition, a flow, a deadline, both a cron and an instant, a one-off with no instant,
  a negative catch-up window, or a processor that cannot fire with no reading.
- The migration also moves what the text does not name. A Remit subscription on a clock with no sensor left (a routine that ran
  its one time) gets a one-off schedule at its last firing and no `due_at`. Each clock's firings are rekeyed to `schedule:<id>`,
  caused `scheduled`, covering no reading; they keep the revision they had (the clock reading's), and a new one's is its instant.
  The clocks' readings and their near-misses are deleted. A watch's `due_at` is recounted from its deadline to the millisecond. Only Remit's scheduled `start` subscriptions get `buffer_one`; a check-back, a goal
  deadline and a shipped job keep `queue`.
- The board's Time source keeps its wire name `clock` (`SignalSource`); `signalKeyOf` reads `schedule:<id>` as it, and a line's key is
  `heardOn`: the schedule key or the pattern.
- `waitFor` stores a subscription and cancels its firings still to be delivered; the caller counts `DueAt` first, with `onceAt` (one
  instant) or `countedFrom` (a cron from now). The `timed` pair of a subscription and its clock is gone.
- No tool sets `CatchupWindow` yet: every schedule has none (no limit), as before. The field is the seam, read by `signal.Evaluate`.
  Revisit when the owner or an agent asks for a routine that must not run late.
- `Sensor.clock` and `SensorClock` leave the wire contract (`web/src/rpc/define.ts`). The Signals page's Time source lists no clock
  sensors (`ClockSensors.vue` is gone): it shows the source's words and the scheduled firings its pattern `schedule:*` finds, and
  the routines and check-backs stay lines of the Subscriptions tab, linked to it.
- The record stage's `moves` also stores a step that moves only `DueAt` (a skipped instant).

**Step 6 as built** (no migration), where it differs from the text above:

- A watch of one exact key of a state starts at `current`, as `watchedFrom` did (what the signal says now is judged); a watch of a
  pattern or of a stream starts at `now` (`wakeStart`). The text gives a watch `now`; that would have stopped a watch of a merge
  request already merged from firing.
- `seq` names the first place judged (the cursor is one before it) and is refused past the head of the log; `time` judges from the
  first reading stored at or after it (`store.Tx.SeqBefore`) and is refused when it is still to come; `current` on a key with no
  reading yet starts at the head. A schedule takes only `now`: it judges no reading.
- `create_subscription` and `update_subscription` take an optional `start` (`now`, `seq`, `time`); an update without one keeps the
  cursor. Resuming a paused subscription to events moves its cursor to the head through the same `startSeq`.
- `subscribe` is also the path for a subscription made again over the one of its id (a watch made twice, a goal check's deadline set
  again on resume, a help's deadline); for one that keeps a schedule it stores through `waitFor`, which cancels the firings still to be
  delivered. Every other store of a subscription is a change of one that is there; fitness `TestOnlySubscribeMakesASubscription`
  lists each with why.
- `Subscription.Check` now runs on every subscription made, so `run`, `end` and `deadline` declare `Expires` (a schedule's instant
  fires them with no reading), and a plugin's action whose plugin is not enabled is checked against what every plugin's processor
  takes (`pluginTakes`): a bundled plugin's subscription is seeded before the owner enables it.
- The window is `streamWindow` (7 days, the text's `eventsKept`), and the job's function is `forgetSignals` (`Orchestrator.forget`
  is an archive's). One store method, `ForgetReadings(streamsSince, statesKept)`, replaces `ForgetEventReadings` and the per-key
  `ForgetReadings`; the stream families are `model.StreamKeyPrefixes` (`event:`, and `fact:`, whose prefix moved to
  `model.FactKeyPrefix`), and every other family is a state, including one the text does not name (`chain:<id>.stage`). A script
  sensor's readings, kept to 200 before, are kept as any state's: the newest 1000.

**Step 7 as built** (no migration: an inbox item's `subscription` is a new optional field), the server half; the Subscriptions tab's
rendering and its `web/verify` page are the web app's, where it differs from the text above:

- The dry run is a pure function of its own, `signal.Replay`: `Evaluate` at the time of each reading and at each instant the subscription
  said it was due (its schedule, a flow's release, its deadline), each evaluation applied before the next. The subscription is judged as
  it is set, active and healthy, with nothing fired before; a firing counts as done at once, so overlap holds none back. Its default start
  is the kept log from its beginning (`seq` 1), since `now` judges nothing. A schedule judges no reading: its dry run takes a time (by
  default `streamWindow` ago) and counts its instants up to now, at most `dryRunReadings` of them. A call that stops early, at the bound or
  where a flow still holds readings back, says the `seq` the next starts from.
- `explain_subscription` evaluates the subscription on the one reading with its cursor just before it, active, no hold, no last firing, no
  deadline, so a pause or a hold does not hide what its rules say; it leaves out `Advance` (the cursor it set itself). A condition holds as
  `Evaluate` judged it; `signal.Probe`'s reason is given only when it is false, since Probe reads no chain facts and refuses a condition
  that reads only `cause`. Beside it are what the Tick recorded: whether the cursor is past the reading, the firings that cover it, its near-miss.
- The Assistant's `dry_run_subscription` takes a stored subscription or a `draft` (an event subscription, as `create_subscription` takes
  it, checked as create checks it); a Chair's takes its own chain's by id.
- A held subscription waits for the owner as an inbox item of kind `held` naming it (`model.InboxItem.Subscription`), raised in the
  write that holds it with `fact:owner.asked`, so `sub_assist_owner` puts it to the Assistant first. It sits on the subscription's chain, or
  on no chain for Remit's own, and so counts in `Board.needsYou`. It is answered `retry` or `skip` (`answerHeld`), the text as why;
  lifting the hold with a tool closes it (`closeHeld`), and deleting the subscription cancels it in the store. An item with no card has no
  asker: `putAskedFact` records it as Remit's, `fireAssist` names Remit as the asker, and an overturn tells only the Assistant.
- On the tab's line `control` is absent while the subscription is active (as `hold` is while it is healthy); `hold` is the stored
  `model.Hold`; `waiting` is the next firing's `Waiting`; `nearMisses` are the newest three (`nearMissesShown`, read for every
  subscription at once by `store.RecentNearMisses`). `SignalBoard.held` counts the held lines. `held` keeps its words for the line's state.
- `startSeq` and `refuseMissing` read through interfaces orch declares (`logReader`, `namesReader`), so a dry run reads the store and
  takes no write transaction.

**Step 8 as built** (no migration), where it differs from the text above:

- `OnMessage` is the type and its field on `AgentStanding` (`onMessage` on the wire, `agentstanding.go:38`); nothing stores it, so no
  record moves. Datadog's `reading:` keyword stays.
- Rollup went whole: `rollup.go`, `SourceRollup`, `Config.RollupURL`, the silence it fed into the board, the flag and its URL check
  (`requireHTTPURL`, which only it used), `SignalSource` `rollup` in the contract, and `SignalSourceLine.Off`, whose one setter was the
  rollup source's "no telemetry store is set", together with its rendering on the Signals page. `readableBy` admits no `rollup:*` key:
  a chain watches its own records (its sensors' among them), the merge requests of a chain that serves the same objective, and for a
  coaching chain the teachings. The dev database had no subscription, reading or firing of `rollup:*` (counted 2026-11-05). On a
  database that has some, the log keeps each key's newest reading, which the Signals page lists with no source, and a watch on one
  ends at its deadline (at most 7 days); nothing writes or admits a new one.
- The tests that drove the rollup adapter (its store, its silence, its flag) went with it. The tests that used a `rollup:<name>` key as
  any state's moved to `chain:<id>.sensor.<name>`, a state too: the watch tests watch the chain's own sensor of the checkout's error
  rate, and the Signals page's seeds in `web/verify` show sensors where they showed rollups.
- `TestEveryIndexStatusIsItsRecordsStatus` (`internal/fitness/adr_test.go`) reads the state word and the records named in the first
  sentence of each record's `Status:` paragraph and of its row; `TestTheIndexCheckCatchesADriftedStatus` is its catch. ADR-0014's
  `Enforced by:` names the sensor test that holds what the deleted rollup test held: a silent source holds its watch's line.

## Threats

- **The door and every writer.** Ingest makes the pause rail hold for every writer, which closes the plugin leak; writer identity
  comes from code, so a payload cannot claim to be another writer. A flood of events is stored and judged as today; no rate cap is built.
- **A plugin processor** can no longer hold the scheduler: its deadline and its own worker bound it. It can still be slow on its own
  subscription, which the board shows as waiting or held.
- **The request id** is a key the processor checks, not a secret; a plugin that ignores it can act twice on a redelivery, which the
  SDK's documentation says a processor must not.
- **A dry run** reads the log as the agent's capability already allows (`read_signal`); it takes no lock and is bounded per call.

## Consequences

- Every outcome is a record: a firing with a status, a near-miss, a hold, a counted skip. "Why didn't it fire" is a tool call.
- One owner each: writing (`ingest`), deciding (`signal.Evaluate` and `Apply`), recording (`record.go`), doing (`Deliver`), forgetting
  (`forget`). The copied tx blocks, arity checks and `errFiredAlready`/`errNotDue` handling go, about 15 sites.
- Delivery is at-least-once with a key; a database effect is exactly once. Today some effects (`fireEnd`, `fireRun`, the plugin's) were
  already outside their firing's transaction; now that is the explicit rule, with the key.
- A subscription's firings are delivered in order; unrelated subjects on one busy subscription wait for each other (Revisit).
- A held or paused subscription keeps its keys' readings without limit; the board shows how many it holds back.
- A reading emitted inline no longer fires within `emit`'s call; it fires on the Tick the kick wakes, milliseconds later. A caller that
  needed the firing at once (`o.emit` for "callers that need the firings at once", ADR-0036) reads it after the Tick.
- Time-as-a-sensor (ADR-0049) is reversed for a subscription's own time: a private 1:1 sensor was a second record for one concept.

Stress test. The strongest counter-argument is that hold-in-place lets one poison reading stop a subscription for good, where a dead
letter would move on. It stands because the hold is visible where the owner and the Assistant look, `skip_firing` moves on with a
reason in one call, and moving on automatically is the silent loss this record exists to end; Nygard's "pause and alert after N
failures" is the same rule. Against next year's models: nothing here is scaffolding for a weaker model. Named policies, a validator and
a dry run are what a stronger agent uses to set up a rule right the first time; as models improve they use them more, and nothing has
to be removed.

## Revisit when

- **Per-key serialization** is wanted (unrelated subjects on one subscription block each other): deliver one at a time per
  (subscription, `Reading.Subject`); the blocker shows as the held firing's subject.
- **Delivery concurrency** must be limited (many subscriptions' workers at once strain a provider): a limit in `deliver.go`
  (`startDelivering`, the one place a worker starts), per processor, a field of `ProcessorSpec`. Today every subscription with a firing
  due has its worker at once.
- **A plugin's processing is long-running** past a fixed deadline: a heartbeat on `ProcessRequest` extends `Deadline` while it beats.
- **The log grows past what the disk holds** because of a forgotten held or paused subscription: an age cap per kind; a subscription
  that falls past it gets `Hold{cursor_expired}` with the gap recorded, never a silent advance.
- **Dropping readings** is asked for (a rule that wants at most one firing per period and the rest lost): `FlowPolicy` `drop`.
- **A clock several subscriptions share** is asked for: a sensor kind that writes a stored stream `clock:<name>`.
- **Built after this refactor** (2026-11-05): Grafana as a poll source (ADR-0046), the Google Analytics plugin (ADR-0066), and Grafana's HMAC
  signature at the door (ADR-0030).

Enforced by: TestAFiringIsStoredOnceWhoeverRetriesIt, TestAChairsQuestionReachesTheAssistantFirst, TestAStuckAssistantLeavesItToTheOwner,
TestTheOwnerOverturnsAnAssistantsDecision, TestProbeRunsAConditionOnARealReadingAndSaysWhyItCannot. Each step's gate tests above join
`docs/design.md` as the step lands.
