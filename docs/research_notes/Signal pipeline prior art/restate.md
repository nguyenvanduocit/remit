# Restate (restate.dev): keyed state, event handling and timers, and lessons for the Remit signal pipeline

Scope and limits of this note: researched 2026-11-05 from the current docs.restate.dev pages (fetched via their `llms.txt` index), the GitHub release notes for v1.4.0 to v1.8.1 (`gh api repos/restatedev/restate/releases`), one Restate engineering blog post and Restate's vendor comparison page. Three questions the docs left open were settled from Restate's Rust source at `main` commit `b074c52` (2026-11-03), shallow-cloned from [restatedev/restate](https://github.com/restatedev/restate/blob/b074c52fe5264947c86c1a43c782db7cd2cd72bd). Each finding below cites its file and line. The standalone Kafka ingress was read at [restatedev/ingress-integration-kafka](https://github.com/restatedev/ingress-integration-kafka) commit `4c20e3f`. About 50 tool calls in total. Release dates come from the GitHub releases API: v1.4.0 2025-06-23, v1.5.0 2025-09-16, v1.6.0 2026-01-30, v1.7.0 2026-06-18, v1.8.0 2026-10-14, v1.8.1 2026-10-21 ([GitHub releases](https://github.com/restatedev/restate/releases)).

Remit problem codes used below: (a) subscriptions fail or skip silently; (b) no start position, backfill or dry run; (c) retention ignores subscriber cursors; (d) "signal" conflates state and stream; (e) time is split between clock sensors and subscription windows, and the overlap policy is implicit; (f) a plugin processor runs under the global lock with no timeout; (g) AI agents create and debug subscriptions through tools.

## 1. Architecture: log-first design, partition processors, per-invocation journal, and the contrast with Temporal

### Takeaway
Restate is one replicated, partitioned log (Bifrost) with one leader processor per partition. The processor tails the log and keeps everything else (journals, per-key state, timers, idempotency records) in a RocksDB cache that it can rebuild from the log. Every step is appended to the log and committed before it takes effect. Temporal instead has pull-based workers that rebuild state by replaying event history. Restate pushes the invocation to the handler with its journal attached. Both models require deterministic replay of the handler code.

### Cited Findings
- "The log is the primary durability layer"; the system "treats the replicated log as ground truth"; "A write is committed when a quorum of replicas acknowledges the append." — [Architecture](https://docs.restate.dev/references/architecture.md)
- Bifrost is a "segmented virtual log": "the active segment receives appends", and reconfiguration "seals the active segment" so leadership can change "without copying data." — [Architecture](https://docs.restate.dev/references/architecture.md)
- Each partition has one processor leader and optional followers. "Only a single leader for a given partition can be active at a time, and this is the sole processor that handles invocations." Followers "keep up with the log without taking action" so they can take over. — [Architecture](https://docs.restate.dev/references/architecture.md)
- The processor "tails the log, invokes your handler code via a bidirectional stream, and maintains a materialized state cache in an embedded RocksDB". That cache holds "journals, idempotency metadata, key-scoped state for virtual objects, and timer indices" and "can always be rebuilt from the log—and is not a second source of truth." — [Architecture](https://docs.restate.dev/references/architecture.md)
- "Workflow IDs, virtual-object keys, or idempotency keys deterministically hash to a partition". "Steps, state updates, and timers for a key execute entirely within one partition—no cross-partition coordination is needed for the hot path." Cross-partition messages are "delivered exactly once to the destination partition via an internal shuffler." — [Architecture](https://docs.restate.dev/references/architecture.md)
- Snapshots: processors snapshot RocksDB to object storage, and a new processor can "download the latest snapshot and replay only the log suffix since the snapshot's sequence number". Log entries can be trimmed only after a snapshot covering them is safely persisted. — [Architecture](https://docs.restate.dev/references/architecture.md)
- Epoch fencing: "the processor rejects any events from superseded epochs" (split-brain protection). — [Architecture](https://docs.restate.dev/references/architecture.md); the blog says "Any messages carrying lower epochs than the latest epoch-bumping message will be ignored." — [Building a modern Durable Execution Engine from First Principles, 2025-02-20, Ewen/Farghal/Rohrmann](https://www.restate.dev/blog/building-a-modern-durable-execution-engine-from-first-principles)
- "Events get always added to the log first, and once they are received by the processor they are acted upon." "You can think of Bifrost as the transaction WAL, and the processor as the query engine and table storage." "Partition Processors operate independently from each other and access exclusively local data structures." — [Restate blog](https://www.restate.dev/blog/building-a-modern-durable-execution-engine-from-first-principles)
- The blog argues against the common advice "don't build a new stateful system, just use Postgres": the access patterns justify a purpose-built stack. — [Restate blog](https://www.restate.dev/blog/building-a-modern-durable-execution-engine-from-first-principles)
- Vendor claims (Restate's own page, undated, mentions "As of September 2026"):
  - Temporal "Workers poll for work, execute it, and report results", and Temporal state is "reconstructed by replaying event history".
  - Restate services "get invoked over HTTP, without a dedicated application worker process or task-polling loop". "Completed work is recovered from the journal instead of being executed again", and the invocation carries "the request, journal, and state".
  - Temporal entities are "indefinitely running workflows" that need Continue-As-New, while Restate "stores durable K/V state per object key".
  - Restate claims "<10 ms per durable action" P99 overhead, with no methodology given.
  - [Restate vs Temporal (vendor page)](https://www.restate.dev/vs/temporal)
- Independent blog comparisons say both engines use journal-and-replay, which forces deterministic code. They are secondary sources with no benchmarks. — [youngju.dev 2026 comparison](https://www.youngju.dev/blog/culture/2026-05-14-durable-execution-engines-2026-temporal-restate-inngest-trigger-dev-dbos-deep-dive-2026.en); [devstarsj 2026](https://devstarsj.github.io/2026/05/13/durable-execution-temporal-restate-distributed-systems/)

### Inferences
- Remit's shape already rhymes with Restate's: one SQLite log with a global seq plays the role of Bifrost, and orch state plays the processor's materialized cache. The difference is that Restate commits every effect (timer registration, state write, outgoing call, firing) to the log before acting, and derives everything else from it. Remit appends readings to the log, but firings, cooldown windows and plugin results are written beside it under the global mutex. (MEDIUM: based on the problem statement, not on reading Remit code.)
- Restate does not need a global lock because partitioning by key gives a single writer per partition, and the key decides the partition. In a single SQLite process the analogue is serialization per (subscription, key) or per key, not per process. → informs (f).
- "The cache can be rebuilt from the log, never a second source of truth" is a usable test for Remit: if subscription cursors, firing records and window state cannot be re-derived from the log plus subscription definitions, they are a second truth. → informs (c), (d).

### Gaps
- No primary Restate doc compares Restate with Temporal directly. The only comparison is the vendor page above, so treat it as positioning.
- I did not find a doc on how timer indices are scanned and fired inside the processor, beyond "timer indices" living in RocksDB and timers being logged like other actions.

## 2. Services, Virtual Objects and Workflows: exclusive handlers, shared handlers, and per-key K/V state

### Takeaway
A Virtual Object is a keyed entity that runs at most one writing (exclusive) handler at a time per key. Other calls to that key queue in a per-key inbox in arrival order, while different keys run in parallel. Read-only shared handlers run alongside the writer. State is a durable K/V map per key. Workflows are Virtual Objects whose `run` handler executes exactly once per ID. This per-key single writer, combined with per-partition single leadership, is what replaces a global lock.

### Cited Findings
- Basic Service: "Unlimited parallel execution", no shared state. Virtual Object: keyed, with "K/V state retained indefinitely and shared across requests". Workflow: "The `run` handler executes exactly once per workflow ID." — [Services](https://docs.restate.dev/foundations/services.md)
- "At most one handler with write access can run at a time per object key", which the docs describe as mimicking a queue per key. Shared handlers are read-only and run concurrently. — [Services](https://docs.restate.dev/foundations/services.md)
- `sys_inbox.sequence_number` "orders the queued invocations for each Virtual Object or Workflow key. Only one runs at a time per key." — [SQL introspection reference](https://docs.restate.dev/references/sql-introspection.md)
- To find what blocks a key, each key's lock is named `service/key`: `select acquired_by from sys_locks where lock_name = 'MyServiceName/myKey' and scope is null`. "The `acquired_by` value is the blocking invocation ID, which you can cancel." — [Introspection](https://docs.restate.dev/services/introspection.md). `sys_locks` replaced `sys_keyed_service_status` in v1.8.0 — [v1.8.0 release notes](https://github.com/restatedev/restate/releases/tag/v1.8.0)
- Sleeping in an exclusive handler "queues all other calls to that object". The docs prefer delayed messages over sleep-then-send so "other requests to the same Virtual Object keep processing". — [Durable timers (TS)](https://docs.restate.dev/develop/ts/durable-timers.md)
- Eager state access (the default) sends a full snapshot of the object's K/V entries with each invocation. Lazy access loads each entry on `ctx.get`. — [Service configuration](https://docs.restate.dev/services/configuration.md)
- New in 2026 (v1.7 opt-in, v1.8 default): scoped Virtual Objects. "Cart 'a' under scope bob differs from Cart 'a' under scope joe", and unscoped, `bob` and `joe` are "three separate objects with separate state, queues, and locks." — [Flow control](https://docs.restate.dev/services/flow-control); [v1.8.0 release notes](https://github.com/restatedev/restate/releases/tag/v1.8.0)
- Agent sessions are modeled as Virtual Objects keyed by session ID: "Restate queues concurrent requests to the same session key." — [AI sessions pattern](https://docs.restate.dev/ai/patterns/sessions.md)

### Inferences
- For Remit (f): the unit of serialization should be the entity a firing mutates, such as a (subscription, key) pair or a chain, not the whole process. A plugin processor that hangs then blocks only its own key, as a sleeping exclusive handler blocks only its object, and the blockage is visible as a lock holder with an ID the owner or an agent can cancel. (MEDIUM)
- The `sys_locks` / `sys_inbox` pair is the model for "why didn't my subscription fire?": show who holds the key and what is queued behind it. → informs (a), (g).
- The docs warn that long work inside an exclusive handler starves the key, and the fix they give is to schedule a delayed message instead of sleeping. Remit's cooldown and deadline windows could follow the same rule: scheduled future entries in the log, never held locks or in-memory waits. → informs (e).

### Gaps
- The Services page does not discuss deadlocks, such as two Virtual Objects calling each other with exclusive handlers. I found no doc on it in this pass.

## 3. Durable timers, delayed calls, awakeables, durable promises and signals: how an external event resumes a waiting handler

### Takeaway
Time and outside events use the same journal mechanism. `ctx.sleep` and delayed sends are durable timers stored by the partition, and a restart resumes with only the remaining time. Awakeables give an ID that an outside system resolves over HTTP, and the handler suspends (uses no compute) until then. Workflow promises and per-invocation named signals cover in-entity events. Restate has no native cron: recurring jobs are a Virtual Object that reschedules itself with a delayed message, and the docs say plainly that overlapping runs can happen.

### Cited Findings
- `ctx.sleep(...)`: "There is no hard limit on how long you can sleep." "Restate tracks and manages timers, ensuring they survive failures and restarts". A service that sleeps 12 hours and fails after 8 sleeps only 4 more. — [Durable timers (TS)](https://docs.restate.dev/develop/ts/durable-timers.md)
- Clock caveat: the SDK computes the wake-up time from the delay and the server uses it, so "If the Restate Server and the SDK have different system clocks, the sleep duration might not be accurate." — [Durable timers (TS)](https://docs.restate.dev/develop/ts/durable-timers.md)
- `RestatePromise.orTimeout({ seconds: 5 })` throws `TimeoutError`. — [Durable timers (TS)](https://docs.restate.dev/develop/ts/durable-timers.md)
- Delayed invocation over HTTP: `/restate/send/...?delay=10s` (humantime) or `?delay=PT10S` (ISO 8601). The response returns an invocation ID usable to cancel or kill it. — [HTTP invocation](https://docs.restate.dev/services/invocation/http.md)
- Delayed sends are detached from the caller: "One-way and delayed calls are detached from the call tree and are not killed by either action" (cancel or kill). — [Managing invocations](https://docs.restate.dev/services/invocation/managing-invocations.md)
- Awakeables: `ctx.awakeable<T>()` returns `{ id, promise }`, and the "Handler suspends here until external completion". They are resolved from another handler with `ctx.resolveAwakeable`/`rejectAwakeable`, or from outside via `/restate/awakeables/<id>/resolve` or `/reject`. — [External events (TS)](https://docs.restate.dev/develop/ts/external-events.md)
- Workflow promises: `ctx.promise<string>("review")`, resolvable from any handler of the same workflow. Results are readable many times during retention (default 24 h after `run` completes). — [External events (TS)](https://docs.restate.dev/develop/ts/external-events.md)
- Signals: identified by target invocation ID plus a name (`ctx.signal<boolean>("approval")`). They "Can be resolved multiple times. Each `await` receives the next resolution", and resolutions "are stored durably until received, and can arrive before or after the waiter starts." — [External events (TS)](https://docs.restate.dev/develop/ts/external-events.md)
- `sys_promise` (workflow promises) was added in v1.7.0. — [SQL introspection reference](https://docs.restate.dev/references/sql-introspection.md)
- Inactivity timeout (default 1 min): if no new journal entries arrive in that window, Restate asks the service to suspend. Abort timeout (default 10 min): Restate forcibly aborts after it. — [Service configuration](https://docs.restate.dev/services/configuration.md)
- Cron: "Restate does not yet include native cron support." — [Durable timers (TS)](https://docs.restate.dev/develop/ts/durable-timers.md)
  - The cron guide has a `CronJob` Virtual Object whose `execute` sends the task and then a delayed `execute` to itself ("Jobs automatically reschedule themselves after each execution").
  - It stores the next invocation ID in state so `cancel` can cancel it.
  - It warns that failures can cause "concurrent executions of the same cron job (one retrying and the other starting up)", and its mitigation is to attach to the previous run with a zero timeout and cancel it if it is still running.
  - It does not address missed runs or catch-up.
  - [Cron guide](https://docs.restate.dev/guides/cron.md)

### Inferences
- → (e): Restate has one notion of time: a timer is a journal or log entry with a wake-up timestamp, owned by the partition. Remit splits time between clock sensors (readings) and subscription windows (cooldown and deadline). The Restate lesson is to make "wake at T" a scheduled entry in the same log or store that the scheduler fires, so the subscription window and the clock are one mechanism. (MEDIUM)
- → (e): Restate's cron guide shows that the overlap policy is the user's job and is easy to get wrong. A self-rescheduling job plus retries gives two concurrent runs unless code checks for it. Remit should make the overlap policy an explicit field of a timed subscription (skip, queue, or cancel the previous run) rather than an implicit effect of cooldown. No Restate primitive offers this, so it is a gap Remit can close. (MEDIUM)
- → (d): Restate keeps three things apart that Remit's "signal" merges:
  - current value per key: the K/V `state` table;
  - one-shot completion: an awakeable or promise, resolved once and read many times;
  - an ordered stream of occurrences: a signal resolved many times with each await taking the next one, or Kafka records that each become an invocation.
  (HIGH that Restate separates them, per the cited docs. MEDIUM that the split maps onto Remit.)
- An awakeable is the closest analogue to Remit's "a chain waits on an outside event". The ID is handed out first and the result is stored durably even if it arrives before the waiter (true of signals as documented), so the order of arrival does not matter.

### Gaps
- Timer catch-up after downtime (from source, HIGH):
  - Timers are kept in wake-up-time order. `TimerQueue::await_timer` sleeps only when `sleep_until.duration_since(now)` is positive, so a timer already in the past fires at once, with no delay ([crates/timer-queue/src/lib.rs:72-80](https://github.com/restatedev/restate/blob/b074c52fe5264947c86c1a43c782db7cd2cd72bd/crates/timer-queue/src/lib.rs#L72-L80)).
  - The timer service reads stored timers in batches, capped by `num_timers_in_memory_limit` ([crates/timer/src/service/mod.rs:110-233](https://github.com/restatedev/restate/blob/b074c52fe5264947c86c1a43c782db7cd2cd72bd/crates/timer/src/service/mod.rs#L110-L233)).
  - So after downtime, every overdue timer fires once, oldest first, as fast as the batches load.
  - I found no coalescing or skipping of overdue timers. Because Restate has no native cron, a missed recurrence is not replayed either: a self-rescheduling job fires its one overdue timer and then computes its next run.
- Delayed invocations under v1.8 vqueues (from source, HIGH):
  - When an invocation has a `vqueue_id`, the state machine "skips the rest of this logic and jumps straight to vqueues' implementation" ([crates/worker/src/partition/state_machine/mod.rs:907-915](https://github.com/restatedev/restate/blob/b074c52fe5264947c86c1a43c782db7cd2cd72bd/crates/worker/src/partition/state_machine/mod.rs#L907-L915)).
  - `vqueue_enqueue` puts the entry in the queue with `run_at = execution_time` and marks it `Scheduled` only "if it's actually in the future" ([mod.rs:1020-1040](https://github.com/restatedev/restate/blob/b074c52fe5264947c86c1a43c782db7cd2cd72bd/crates/worker/src/partition/state_machine/mod.rs#L1020-L1040)). No timer is registered.
  - Only the legacy, non-vqueue path registers a `TimerKeyValue::neo_invoke` timer ([mod.rs:1180-1210](https://github.com/restatedev/restate/blob/b074c52fe5264947c86c1a43c782db7cd2cd72bd/crates/worker/src/partition/state_machine/mod.rs#L1180-L1210)).
  - So since v1.8, a delayed call is a queue entry with a run-at time, decided by the same scheduler that applies the key lock and the concurrency limits. `ctx.sleep` timers remain in the timer service.
  - Lesson for Remit (e): a time-delayed action is an entry in the same per-key queue with a not-before time, not a separate clock mechanism. (MEDIUM for the mapping.)
- The TS external-events page does not mark signals as new. I could not confirm which release introduced named signals.

## 4. Kafka subscriptions and event ingress: keying, per-key ordering, offsets, and handler failure

### Takeaway
A subscription maps a topic to one handler. Each record becomes its own invocation, with its own ID and lifecycle. The record key becomes the Virtual Object or Workflow key, so ordering is per key for exclusive handlers and absent for shared or Service handlers. Records are deduplicated on (consumer group, topic, partition, offset), which Restate calls exactly-once. A failing record is a failing invocation: it retries with backoff, then pauses. It does not stall the Kafka partition, because the offset advances once the invocation is durably accepted. For an exclusive handler it does block later records with the same key until it is resumed, cancelled or restarted (verified in source, see Inferences). As of v1.8.1 (2026-10-21) the in-server consumer is deprecated in favor of a standalone Kafka ingress container.

### Cited Findings
- Old in-server subscriptions:
  - Created with `restate subscriptions create kafka://<cluster>/<topic> service://<Service>/<handler> [key=value...]`. The options are any librdkafka parameter, e.g. `auto.offset.reset=earliest`.
  - "Each event creates an invocation with its own unique ID."
  - Deleting a subscription stops its consumer group, and records already enqueued are still processed.
  - The docs say they "plan to remove [this] in future releases."
  - [Kafka (old support)](https://docs.restate.dev/services/invocation/kafka.md)
- Dynamic Kafka cluster and subscription management through the Admin API and CLI is new in v1.7.0 (2026-06-18) (`restate kc create ...`, `restate sub create ... group.id=g1`). — [v1.7.0 release notes](https://github.com/restatedev/restate/releases/tag/v1.7.0)
- Keying and ordering:
  - For Virtual Object and Workflow handlers, the record key is the object key and must be UTF-8.
  - For exclusive handlers, "Records are delivered to the subscribed handler in the order in which they arrived on the topic partition."
  - For shared and Service handlers, "Records are delivered in parallel without ordering guarantees."
  - [Kafka integration](https://docs.restate.dev/services/integrations/kafka.md); [Kafka (old support)](https://docs.restate.dev/services/invocation/kafka.md)
- Event metadata reaches the handler as headers: `restate.subscription.id`, `kafka.offset`, `kafka.partition`, `kafka.timestamp`. — [Kafka (old support)](https://docs.restate.dev/services/invocation/kafka.md)
- Standalone Kafka ingress (needs Restate ≥ v1.8.1):
  - It "runs as its own container and scales independently of the Restate server".
  - It is "the recommended way to consume Kafka events and replaces the old Kafka support".
  - "Restate deduplicates based on consumer group id, topic, partition and offset", which yields "exactly-once processing", and "restarts and rebalances never cause duplicate or lost invocations".
  - An optional `RESTATE_RECORD_MAPPER_IDEMPOTENCYKEY_POINTER` takes the idempotency key from the record body (e.g. `/eventId`).
  - [Kafka integration](https://docs.restate.dev/services/integrations/kafka.md)
- Migration caveats:
  - On delete, the old subscription drains: "Restate stops reading new records and waits up to 5 minutes for the records it already read to be ingested."
  - "Draining is best effort": after a crash or a drain timeout, records may be re-delivered or skipped depending on the restart offset.
  - Reusing a `group.id` resumes from the committed offsets.
  - Running the old and new consumers on the same topic and group splits partitions between them.
  - [Kafka integration](https://docs.restate.dev/services/integrations/kafka.md)
- `sys_invocation.invoked_by` takes the values `ingress`, `service`, `subscription`, `restart_as_new` and `ingress_ingestion`. — [SQL introspection reference](https://docs.restate.dev/references/sql-introspection.md)
- Event-processing positioning: Restate offers "Queue-per-key with ordering guarantees", while Kafka offers "Queue-per-partition". "Failed handlers are retried with exponential backoff until they succeed." Kafka is described as "Coarse-grained offset commits with potential reprocessing." — [Event processing use case](https://docs.restate.dev/use-cases/event-processing.md)
- Private services: when ingress is private, a service "rejects HTTP and Kafka requests but still accepts calls from other Restate services." — [Service configuration](https://docs.restate.dev/services/configuration.md)

### Inferences
- → (a): The central Restate move is that consuming an event creates a durable, addressable invocation. It is never processed inline. The consumer's offset can advance as soon as the invocation is durably accepted, because dedup on (group, topic, partition, offset) makes re-ingestion harmless. Retries, pauses and errors are then per-invocation state, visible in `sys_invocation`. For Remit, a firing is that entity: one row per (subscription, key, revision) with a status (pending, running, backing-off, paused, done, failed), last error and attempt count. A failure becomes a row an agent can query, never a log line. Remit already has the idempotency key; the missing part is the lifecycle. (MEDIUM)
- → (b): Restate's start position is Kafka's (`auto.offset.reset`, or reuse of a consumer group's committed offsets). Remit's equivalent is setting a new subscription's initial SeenSeq: "from now", "from seq N" or "from time T". Restate's documented drain-on-delete and "best effort" warnings show that moving a cursor between consumers is where loss or duplication happens, so the cursor handoff should be one transactional step. (MEDIUM)
- Ordering only holds for exclusive keyed handlers, and parallel handlers give it up explicitly. A Remit subscription could declare whether it needs per-key order, as Restate makes the choice by handler type. (LOW)
- Per-key head-of-line blocking (from source, HIGH): a paused or suspended exclusive invocation keeps its Virtual Object lock.
  - `pause_entry` and `suspend_entry` both call `park_entry`, which moves the entry to `Stage::Paused` or `Stage::Suspended` and never releases the lock ([crates/vqueues/src/lib.rs:686-790](https://github.com/restatedev/restate/blob/b074c52fe5264947c86c1a43c782db7cd2cd72bd/crates/vqueues/src/lib.rs#L686-L790)).
  - The only place the lock is released is `end()`, when the entry moves to `Stage::Finished` (`self.storage.release_lock(...)`, followed by a `LockReleased` event) ([crates/vqueues/src/lib.rs:908-990](https://github.com/restatedev/restate/blob/b074c52fe5264947c86c1a43c782db7cd2cd72bd/crates/vqueues/src/lib.rs#L908-L990)).
  - The v1.8 migration also files paused invocations under their lock name ([crates/vqueues/src/migrations.rs:671-706](https://github.com/restatedev/restate/blob/b074c52fe5264947c86c1a43c782db7cd2cd72bd/crates/vqueues/src/migrations.rs#L671-L706)).
  - So a Kafka record whose handler exhausts its retries and pauses blocks every later record for the same key until someone resumes, cancels, kills or restarts it. Other keys are unaffected.
  - Lesson for Remit (a)/(f): when a failure parks per-key work, it must be visible as the thing blocking that key (Restate shows it in `sys_locks.acquired_by`). Otherwise per-key serialization turns into silent per-key starvation. (MEDIUM for the Remit mapping.)
- Offset commit timing (from source, HIGH, in-server consumer `crates/ingress-kafka`):
  - The consumer sets `enable.auto.commit=true` and `enable.auto.offset.store=false` ([crates/ingress-kafka/src/subscription_controller.rs:117-120](https://github.com/restatedev/restate/blob/b074c52fe5264947c86c1a43c782db7cd2cd72bd/crates/ingress-kafka/src/subscription_controller.rs#L117-L120)).
  - Each record is sent to the partition processor with `ingestion.ingest(...)`, and its commit token waits in an in-order `inflight` queue. The offset is stored (`consumer.store_offset`) only when the record at the head of that queue resolves as committed. librdkafka's auto-commit then commits the stored offsets ([crates/ingress-kafka/src/consumer_task.rs:702-775](https://github.com/restatedev/restate/blob/b074c52fe5264947c86c1a43c782db7cd2cd72bd/crates/ingress-kafka/src/consumer_task.rs#L702-L775)).
  - "Committed" is the partition processor's `CommitCallback` answering `ResponseStatus::Ack` after the forwarded records are proposed and committed ([crates/worker/src/partition/mod.rs:1046-1065](https://github.com/restatedev/restate/blob/b074c52fe5264947c86c1a43c782db7cd2cd72bd/crates/worker/src/partition/mod.rs#L1046-L1065)).
  - So the offset advances when the record is durably accepted as an invocation, not when the handler succeeds. Handler failure never holds back the Kafka offset. It holds back only that key (see above).
  - Dedup uses a producer ID hashed from (subscription, consumer group, topic, partition), with the offset as the sequence number ([consumer_task.rs:685-692, 792-805](https://github.com/restatedev/restate/blob/b074c52fe5264947c86c1a43c782db7cd2cd72bd/crates/ingress-kafka/src/consumer_task.rs#L685-L805)). Redelivery after a crash between ingest and offset commit is therefore dropped by the partition.
  - Lesson for Remit (a)/(b): advance a subscription's SeenSeq when the firing row is durably written, keyed for dedup, and run the action separately against that row. (MEDIUM for the mapping.)

### Gaps
- Standalone Kafka ingress ([restatedev/ingress-integration-kafka](https://github.com/restatedev/ingress-integration-kafka/blob/4c20e3f), Kotlin, commit `4c20e3f` 2026-11-02; from source, HIGH):
  - It forces `enable.auto.commit=false`: "offsets are committed manually, after Restate" acks ([RestateKafkaIntegrationConfiguration.kt:48-50](https://github.com/restatedev/ingress-integration-kafka/blob/4c20e3f/src/main/kotlin/dev/restate/integration/kafka/RestateKafkaIntegrationConfiguration.kt#L48-L50)).
  - On each durable ack it commits `lastCommitted + 1` ([ConsumerVerticle.kt:176-180](https://github.com/restatedev/ingress-integration-kafka/blob/4c20e3f/src/main/kotlin/dev/restate/integration/kafka/ConsumerVerticle.kt#L176-L180)).
  - On a retryable stream failure it rewinds to the first offset not yet committed and reopens a producer with the same `producerId`, "so the server's offset-based dedup drops anything already committed" ([ingestion/PartitionSession.kt:30-44](https://github.com/restatedev/ingress-integration-kafka/blob/4c20e3f/src/main/kotlin/dev/restate/integration/kafka/ingestion/PartitionSession.kt#L30-L44)).
  - This is the same rule as the in-server consumer: the offset follows durable acceptance, not handler success.
  - Silent-skip caveat, relevant to (a): a record the server rejects at ingest (a record-scoped error) is dropped. The consumer moves past it, and the only trace is a `LOG.warn("skipping rejected record ...")` ([PartitionSession.kt:101-105, 186-207](https://github.com/restatedev/ingress-integration-kafka/blob/4c20e3f/src/main/kotlin/dev/restate/integration/kafka/ingestion/PartitionSession.kt#L101-L207)).
  - A record the mapper filters out (null mapping) is skipped without being sent ([ConsumerVerticle.kt:98](https://github.com/restatedev/ingress-integration-kafka/blob/4c20e3f/src/main/kotlin/dev/restate/integration/kafka/ConsumerVerticle.kt#L98)).
  - So Restate's guarantee covers records it accepts. Records rejected before acceptance become log lines, not invocation rows. Remit should record such rejections as firing outcomes, which closes a gap Restate leaves open. (MEDIUM for the mapping.)
- Offset-commit details above for the in-server consumer apply to it alone. Backpressure: the in-flight ingest window is bounded by a semaphore permit per record (`RecordCommitResolver { _permit: OwnedSemaphorePermit }`, `crates/ingestion-client/src/session.rs:127-130`), but I did not trace its size.
- No doc states what a brand-new standalone consumer group does without committed offsets, beyond librdkafka defaults.

## 5. Idempotency keys, exactly-once claims, and retention

### Takeaway
An `Idempotency-Key` header makes a call deduplicate: a duplicate within retention gets the first result, or attaches to the run still in progress. Workflows deduplicate by ID ("Previously accepted"). Records are kept 24 h by default (idempotency, journal and workflow retention), configurable per service or handler, with cluster-wide `default-*` and `max-*` knobs since v1.7. Since 2026, `controlled-idempotent-sharding` gives calls without a key an auto key that only collapses ingress retries. Restate's exactly-once claim rests on log dedup and epoch fencing, and the docs scope it to within Restate.

### Cited Findings
- "Add an idempotency key to your request header to let Restate deduplicate retries." "On retry, Restate returns the first invocation's result or lets you attach to it if still running." — [Invocations](https://docs.restate.dev/foundations/invocations.md)
- Restate "persists the response for a retention period of one day (24 hours)", and a re-invocation with the same key gets "the same response and doesn't re-execute the request". — [HTTP invocation](https://docs.restate.dev/services/invocation/http.md)
- `POST /restate/attach` and `/restate/output` accept a target: a workflow, an idempotent invocation (service or object + handler + idempotency key), or an invocation ID. `POST /restate/lookup` resolves a workflow or idempotency target to its invocation ID. — [HTTP invocation](https://docs.restate.dev/services/invocation/http.md)
- A workflow can be submitted only once, and a resubmission fails with "Previously accepted". — [HTTP invocation](https://docs.restate.dev/services/invocation/http.md)
- Idempotency is per scope: the same key deduplicates only within one scope (flow-control scopes, v1.7+). — [Flow control](https://docs.restate.dev/services/flow-control)
- Retention defaults:
  - Journal retention is 24 h. It is capped by idempotency or workflow retention, and `0` keeps the status without journal entries.
  - Idempotency retention is 24 h.
  - Workflow retention is 24 h after `run` completes; after it, the K/V state is cleared and the promises are discarded.
  - [Service configuration](https://docs.restate.dev/services/configuration.md)
- `controlled-idempotent-sharding` is "the default for new clusters". The HTTP ingress then assigns a random idempotency key to service and virtual object calls without one, which "only collapses retries of the same in-flight request". Completed invocations are then kept by idempotency retention. — [Service configuration](https://docs.restate.dev/services/configuration.md); [Managing invocations](https://docs.restate.dev/services/invocation/managing-invocations.md). The HTTP docs say these injected keys "don't deduplicate client requests". — [HTTP invocation](https://docs.restate.dev/services/invocation/http.md)
- The v1.7.0 release added "Configurable idempotency & workflow retention — new `default-*` / `max-*` retention knobs". — [v1.7.0](https://github.com/restatedev/restate/releases/tag/v1.7.0). The v1.5.0 release kept invocation progress (journals) for 24 h after completion by default and added `max-journal-retention`. — [v1.5.0](https://github.com/restatedev/restate/releases/tag/v1.5.0)
- Duplicate calls with the same idempotency key "resolve to the same invocation", and the processor "returns the already-committed result from the journal without re-executing steps." — [Architecture](https://docs.restate.dev/references/architecture.md)
- v1.8.0 propagates scope, limit key and idempotency key end to end (service protocol v7). — [v1.8.0](https://github.com/restatedev/restate/releases/tag/v1.8.0)

### Inferences
- → (c): Restate keeps two kinds of retention apart:
  - log trimming, which happens only after a snapshot covering the entries is durable (so the trim point follows the slowest durable consumer of the log);
  - result and dedup retention (24 h), which is a separate, explicit per-service knob.

  Remit's log retention should likewise trim only below min(SeenSeq) over active subscriptions, or explicitly mark a subscription "lagged past retention", and keep idempotency-record retention as a separate setting. Restate does not track external consumer cursors itself: it hands that job to Kafka consumer groups. (MEDIUM)
- Remit's (subscription, key, revision) firing key is the analogue of Restate's (target, idempotency key). Restate adds two things Remit may lack: an attach/lookup path that resolves a dedup key to the run it already produced, and a retention window stated next to the key. → (a), (g). (MEDIUM)

### Gaps
- No doc found on what happens when the same idempotency key arrives with a different payload, beyond returning the first result.
- No primary doc gives an end-to-end exactly-once proof. The claims are as quoted above.

## 6. Observability and failure handling: SQL introspection, the UI and CLI, retry policy, pause on max attempts

### Takeaway
Every invocation is a queryable row with status, retry count, last failure (error code, message, the command that failed) and next retry time. Its journal and error events are queryable too, as are the per-key lock holder and inbox, and the K/V state. Since v1.5 and v1.6 the default retry policy pauses an invocation after max attempts (70 by default) instead of killing it. An operator then resumes it (optionally on a new deployment), restarts it as new, or restarts it from a journal prefix. v1.8 (2026-10-14) rebuilt the CLI for scripts and AI agents with `--json`, structured errors and `--dry-run`/`--yes`.

### Cited Findings
- `sys_invocation` columns include `status` (`pending`, `scheduled`, `ready`, `running`, `paused`, `backing-off`, `suspended`, `completed`), `completion_result`, `completion_failure`, `retry_count`, `last_failure`, `last_failure_error_code`, `last_failure_related_command_*`, `last_attempt_deployment_id`, `next_retry_at`, `scheduled_start_at`, `completion_retention`, `journal_retention`, `invoked_by` and `suspended_waiting_future_json`. — [SQL introspection reference](https://docs.restate.dev/references/sql-introspection.md)
- `retry_count` counts attempts since the current leader started the invocation. It is not a global counter. — [SQL introspection reference](https://docs.restate.dev/references/sql-introspection.md)
- `sys_journal` (one row per entry: type, name, completed, `sleep_wakeup_at`, `promise_name`, `entry_json`, `appended_at`) and `sys_journal_events` (transient errors, pauses, suspensions). — [SQL introspection reference](https://docs.restate.dev/references/sql-introspection.md)
- New flow-control tables (v1.7/1.8): `sys_vqueues`, `sys_vqueue_entry_status` (stage `inbox`, `running`, `paused`, `suspended` or `finished`; `num_attempts`, `num_errors`, `num_pauses`), `sys_vqueue_meta` (moving averages of wait and run times), `sys_scheduler` (what the head entry is `blocked_on`), `sys_locks`, `sys_rules` and `sys_user_limits`. — [SQL introspection reference](https://docs.restate.dev/references/sql-introspection.md); [Flow control](https://docs.restate.dev/services/flow-control)
- Docs queries:
  - Retrying invocations: `select entry_id from sys_vqueues where stage = 'inbox' and status = 'backing-off'`. The docs note "A retrying invocation shows as `ready` in `sys_invocation`, so check `sys_vqueues` for `backing-off`."
  - Stuck invocations: `select * from sys_invocation where modified_at <= now() - interval '1' hour`.
  - Who triggered an invocation: `invoked_by, invoked_by_service_name, invoked_by_id`.
  - Zombie invocations pinned to a removed deployment.
  - CLI equivalents: `restate invocations list --status backing-off`, `--zombie`, `restate invocations describe <id>` (status, retry count, last failure) and `restate invocations journal <id>`. SQL runs via `restate sql` or `POST :9070/query`.
  - [Introspection](https://docs.restate.dev/services/introspection.md)
- The v1.6.0 release (2026-01-30) enabled "Error Events" by default: "When invocations encounter transient failures, Restate now automatically records detailed error information in the journal — visible in the UI invocation timeline and queryable via SQL... including error codes, messages, stacktraces, and the related command that caused the failure." — [v1.6.0](https://github.com/restatedev/restate/releases/tag/v1.6.0)
- Retry policy defaults: initial interval 50 ms, factor 2.0, max interval 60 s, max attempts 70, on max attempts `pause` (the other option is `kill`, which fails the caller with a terminal error and does no compensation). It is set at service or handler level in the SDK, globally under `[invocation.default-retry-policy]`, and through runtime UI/CLI overrides that last until the next service revision is registered. — [Service configuration](https://docs.restate.dev/services/configuration.md)
- History of the pause default:
  - v1.5.0 (2025-09-16) introduced `default-retry-policy`, where pausing on max attempts was opt-in, so invocations "avoid unnecessary retries" and can be resumed with a click.
  - v1.6.0 made pause the default and added manual pause and resume, "even on a different deployment".
  - [v1.5.0](https://github.com/restatedev/restate/releases/tag/v1.5.0); [v1.6.0](https://github.com/restatedev/restate/releases/tag/v1.6.0)
- Recovery actions:
  - Cancel: graceful, runs compensation, "non-blocking", and needs the deployment reachable.
  - Kill: "immediately stops all calls in the invocation tree without executing compensation logic."
  - Pause and resume: resume works on the original deployment, `--deployment latest` or a specific ID. The docs warn that changed logic can cause non-determinism errors.
  - Restart as new: a new ID with the same input; "no partial progress will be kept".
  - Restart from prefix: keeps part of the journal (v1.6).
  - Purge.
  - REST: `PATCH :9070/invocations/<id>/<action>`, with bulk targets by service, handler or `Object/key/handler`.
  - [Managing invocations](https://docs.restate.dev/services/invocation/managing-invocations.md); [v1.6.0](https://github.com/restatedev/restate/releases/tag/v1.6.0)
- v1.5.0 "Restart as new" was framed as a dead-letter-queue replacement: "No need to build your own dead letter queue logic anymore, just restart the failed invocations when you need to." v1.5.0 also added "Retry now", which skips the backoff timer. — [v1.5.0](https://github.com/restatedev/restate/releases/tag/v1.5.0)
- v1.8.0 CLI "Built for Scripts, CI and AI Agents":
  - a global `--json` flag; "failures come as structured error objects with distinct exit codes";
  - "State-changing commands share a `--dry-run` / `--yes` preview-then-apply flow";
  - new commands `invocations journal`, `sql describe` (table schemas), `search` ("free-text command discovery, e.g. `restate search pause a stuck invocation`") and `openapi`.
  - [v1.8.0](https://github.com/restatedev/restate/releases/tag/v1.8.0)
- Restate UI 1.0 shipped with v1.7.0: "a complete new layout, better observability features". — [v1.7.0](https://github.com/restatedev/restate/releases/tag/v1.7.0)

### Inferences
- → (a): Restate's answer to silent failure has three parts:
  1. every unit of work is a row with a status enum that includes `backing-off` and `paused`;
  2. the last error is attached to that row, with the failing step;
  3. exhausting retries parks the work (paused) instead of dropping it, and keeps an explicit resume, restart or kill path.

  Remit firings can take the same shape. "Skipped" should be a recorded outcome with a reason: condition false, cooldown, deadline passed, deduplicated. Today it is an absence. (MEDIUM)
- → (g): The v1.8 CLI is direct prior art for AI agents operating such a system:
  - structured JSON output with distinct error codes;
  - preview-then-apply on every changing command;
  - schema discovery (`sql describe`) and command discovery (`search`);
  - one query surface (SQL) over status, journal, locks, inbox and state.

  Remit's subscription tools could offer the same. `create_subscription --dry-run` would evaluate the CEL condition over the last N readings from a chosen start seq and return what would have fired, which also covers (b). A `why_not_fired(subscription, key)` tool would return the cursor, the last evaluated reading, the condition result, the cooldown state and the last error. (MEDIUM)
- → (f): Restate bounds handler execution with an inactivity timeout (1 min) and an abort timeout (10 min), and since v1.7 a bounded invoker memory pool (1.5 GiB) and per-scope concurrency limits. A Remit plugin processor needs at least the timeout and must run outside the global mutex. The firing row's status would then show `running` and `backing-off` and carry the error when it times out. (MEDIUM)

### Gaps
- I did not inspect the Restate UI screens directly. The UI 1.0 claims come from release notes.
- The exact `event_type` values in `sys_journal_events` are not listed on the docs page.

## 7. Restate for AI agents (waiting on events, sessions, flow control)

### Takeaway
Restate's AI guidance builds agents from the same primitives. A session is a Virtual Object keyed by session ID, so messages queue per session. A human approval is an awakeable raced against a durable timer. Every LLM call and tool call is a journaled step. Cost is capped with scope-based concurrency limits, which Restate presents as made for AI agents. An agent's wait is a suspended invocation that uses no compute, and the outside world resumes it through an HTTP callback.

### Cited Findings
- Human-in-the-loop:
  - The agent creates `ctx.awakeable<boolean>()`, sends the ID to the approver through a durable `ctx.run(...)` step, then awaits.
  - The agent "suspends, freeing compute resources", and the approver resolves it with `curl .../restate/awakeables/<id>/resolve`.
  - The agent "resumes from the exact point it paused".
  - The timeout is `approval.promise.orTimeout({ hours: 3 })` in TS, or `restate.select(approval=..., timeout=ctx.sleep(...))` in Python. On restart, "Restate persists both the timer and the approval promise."
  - [AI human-in-the-loop](https://docs.restate.dev/ai/patterns/human-in-the-loop.md)
- Sessions are Virtual Objects. "Restate queues concurrent requests to the same session key". History lives in K/V state, and a shared handler can read it while a turn runs. The Agent Reference Architecture separates the controller from turn execution so you can "steer a running turn or respond to approvals while it works." Integrations: Vercel AI SDK, OpenAI Agents SDK, Google ADK, Pydantic AI, LangChain. — [AI sessions](https://docs.restate.dev/ai/patterns/sessions.md)
- Observability for agents: "Every LLM call, tool execution, routing decision, and state change is recorded and inspectable". OpenTelemetry traces, with each durable step a span, export to LangFuse or Jaeger. Cancel exists for "long-running agents that may get stuck in loops or consume excessive resources." — [AI observability & control](https://docs.restate.dev/ai/patterns/observability-control.md)
- Flow control (v1.7 2026-06-18, on by default in v1.8 2026-10-14):
  - "each concurrent invocation can translate directly into model or API spend".
  - Scopes and two-level limit keys apply concurrency rules (`restate rules set "checkout" --concurrency 50`).
  - Excess invocations are "held in their queue until a slot frees up", never rejected.
  - Limits are per scope, never global.
  - Rate limits, priorities and backlog limits are planned, not built.
  - A scope is a sharding key: "All work for a scope is processed by one partition".
  - [Flow control](https://docs.restate.dev/services/flow-control); [v1.7.0](https://github.com/restatedev/restate/releases/tag/v1.7.0); [v1.8.0](https://github.com/restatedev/restate/releases/tag/v1.8.0)
- Ordering under limits: "Calls to the same object with the same limit key run in arrival order". Across limit keys, a held-back key can let a later invocation run first. — [Flow control](https://docs.restate.dev/services/flow-control)

### Inferences
- → (g) and the agent-centric rule in Remit: Restate's agent primitives are all addressable IDs (invocation ID, awakeable ID, session key), and every one has a status query and a control verb (pause, resume, cancel, kill, restart). An agent that debugs a subscription needs the same: the firing ID, its status, its journal of steps (condition evaluated, action dispatched, result) and the verbs retry-now, resume, skip and restart-as-new. (MEDIUM)
- Restate's queue-at-limit semantics (hold, never reject) and per-limit-key ordering show how Remit's cooldown could be expressed: as admission control on a per-(subscription, key) queue, with the overlap policy explicit. → (e). (LOW-MEDIUM)
- Restate states its limits per scope and warns that `*` is not a global cap. Remit's policies should name what they apply to with the same precision. (LOW)

### Gaps
- No Restate doc found on agents creating or modifying subscriptions or event routes themselves. Agents call handlers. The routing (subscriptions, rules) is operator-configured via CLI or Admin API, and v1.8's agent-friendly CLI is the closest feature.
- No Restate MCP server for operating the runtime was found in the docs pages fetched.

## 8. Mechanism-to-problem map (summary of the inferences above)

### Takeaway
Restate's strongest lessons for Remit:
- a firing as a first-class, queryable invocation with a lifecycle and pause-on-exhaustion → (a);
- per-key single-writer queues in place of the global lock, with timeouts and a lock-holder view → (f);
- K/V state, one-shot promises and multi-shot signals or streams kept apart → (d);
- timers as entries in the same log, with the overlap policy made explicit (Restate leaves it to the user) → (e);
- log trimming gated by durable progress, with dedup retention as its own knob → (c);
- an agent-grade CLI (`--json`, `--dry-run`/`--yes`, schema discovery, SQL over status, locks and journals) → (b), (g).

### Cited Findings
- (a) Invocation status and last-failure columns — [SQL introspection](https://docs.restate.dev/references/sql-introspection.md). Pause on max attempts by default — [Service configuration](https://docs.restate.dev/services/configuration.md), [v1.6.0](https://github.com/restatedev/restate/releases/tag/v1.6.0). Error events in the journal — [v1.6.0](https://github.com/restatedev/restate/releases/tag/v1.6.0). Restart-as-new replaces a DLQ — [v1.5.0](https://github.com/restatedev/restate/releases/tag/v1.5.0).
- (b) `auto.offset.reset` and consumer-group resume — [Kafka (old)](https://docs.restate.dev/services/invocation/kafka.md), [Kafka integration](https://docs.restate.dev/services/integrations/kafka.md). CLI `--dry-run`/`--yes` — [v1.8.0](https://github.com/restatedev/restate/releases/tag/v1.8.0).
- (c) Trim only after a snapshot persists; 24 h idempotency and journal retention; `max-*` knobs — [Architecture](https://docs.restate.dev/references/architecture.md), [Service configuration](https://docs.restate.dev/services/configuration.md), [v1.7.0](https://github.com/restatedev/restate/releases/tag/v1.7.0).
- (d) `state` table vs awakeables and promises vs multi-resolution signals vs per-record invocations — [External events](https://docs.restate.dev/develop/ts/external-events.md), [Introspection](https://docs.restate.dev/services/introspection.md).
- (e) Durable sleep, delayed send, `orTimeout`, no native cron, overlap acknowledged — [Durable timers](https://docs.restate.dev/develop/ts/durable-timers.md), [Cron guide](https://docs.restate.dev/guides/cron.md).
- (f) One leader per partition, one writer per key, `sys_locks`, inactivity and abort timeouts, concurrency rules — [Architecture](https://docs.restate.dev/references/architecture.md), [Services](https://docs.restate.dev/foundations/services.md), [Service configuration](https://docs.restate.dev/services/configuration.md), [Flow control](https://docs.restate.dev/services/flow-control).
- (g) Agent-oriented CLI and SQL introspection — [v1.8.0](https://github.com/restatedev/restate/releases/tag/v1.8.0), [Introspection](https://docs.restate.dev/services/introspection.md).

### Inferences
- Restate's architecture does not fit a single-node SQLite design whole: Bifrost replication, partitions and epochs solve distribution problems Remit does not have. The transferable parts are semantic:
  - commit to the log, then act;
  - per-key single writer;
  - the unit of work as an addressable entity with a lifecycle;
  - pause as the default end state of failure;
  - timers as log entries;
  - structured, previewable operator and agent tooling.

  (MEDIUM)
- Two things Restate does not provide that Remit needs, so no prior art here:
  - a dry run that evaluates a subscription's condition against history before it goes live (Restate's `--dry-run` previews operator commands, not routing logic);
  - an explicit overlap policy for timed jobs.

  (HIGH that they are absent from the docs read. Coverage is limited to the pages listed above.)

### Gaps
- Source reading covered five questions at `restatedev/restate` commit `b074c52` and `ingress-integration-kafka` commit `4c20e3f`: pause and lock, offset commit (both consumers), overdue timers, delayed invocations under vqueues, and rejected records. Anything else in this note rests on docs and release notes.
