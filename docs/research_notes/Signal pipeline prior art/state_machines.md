# State machines and statecharts for event-driven systems, plus Home Assistant's automation engine

Scope: how explicit state machines model entity lifecycles and event reactions (Harel/SCXML, XState v5, AWS Step Functions,
Azure Durable Entities, Akka event-sourced behaviors, pure reducers), Home Assistant automations as a practical rule engine,
when state machines are overkill, and what each informs for the Remit signal pipeline open problems:
(a) silent fail/skip, (b) no start position/backfill/dry-run, (c) retention ignores cursors, (d) "signal" conflates state and
stream, (e) time split between clock sensors and subscription windows, overlap policy implicit, (f) plugin processor under the
global lock with no timeout, (g) AI agents create and debug subscriptions through tools.

Sources fetched 2026-11-05. Research budget: about 40 tool calls (over the 15-call guide, because the brief spans seven areas).
Harel 1987 was read from page images of a scanned copy (pp. 231-234, 252-262).

## 1. Harel statecharts and SCXML: what hierarchy, parallel states, guards, timed transitions and history solve over flat FSMs

### Takeaway
Statecharts add hierarchy (compound states), orthogonal/parallel regions, guards, history and timed events to flat FSMs, chiefly
to stop the multiplicative state explosion that happens when independent aspects are modelled in one flat machine; SCXML 1.0
(W3C Rec, 2015) gives these an exact execution semantics (run-to-completion macrosteps, internal vs external queues, error
events, cancellable delayed sends).

### Cited Findings
- Harel, "Statecharts: A Visual Formalism for Complex Systems", *Science of Computer Programming* 8 (1987) 231-274; it targets
  discrete-event systems (multi-computer real-time systems, communication protocols, digital control units) and extends state
  diagrams with hierarchy and concurrency to address state explosion — [Papers We Love entry](https://paperswelove.org/papers/statecharts-a-visual-formalism-for-complex-systems-46a2275a/); [Recurse Center summary](https://www.recurse.com/blog/59-paper-of-the-week-statecharts-a-visual-formalism-for-complex-systems)
- Harel's own statement of the problem: a complex system "cannot be beneficially described in this naive fashion, because of
  the unmanageable, exponentially growing multitude of states, all of which have to be arranged in a 'flat' unstratified fashion,
  resulting in an unstructured, unrealistic, and chaotic state diagram"; a usable approach "must be modular, hierarchical and
  well-structured" and "must also solve the exponential blow-up problem by somehow relaxing the requirement that all combinations
  of states have to be represented explicitly" (§1, pp. 232-233) — [Harel 1987, scanned copy](https://www.state-machine.com/doc/Harel87.pdf)
- "In a nutshell, one can say: statecharts = state-diagrams + depth + orthogonality + broadcast-communication"; the kernel is
  "AND/OR decomposition of states together with inter-level transitions, and a broadcast mechanism for communication between
  concurrent components" (§1, p. 233); transitions are labelled with an event and an optional parenthesized condition, e.g.
  γ(P) (§2, p. 234) — [Harel 1987](https://www.state-machine.com/doc/Harel87.pdf)
- Timeouts (§4.2 "Delays and timeouts", pp. 253-255): time is handled "using implicit timers", formally via "the event expression
  timeout(event, number), which represents the event that occurs precisely when the specified number of time units have elapsed
  from the occurrence of the specified event", e.g. timeout(entered date, 120). Because "the need to limit the system's lingering
  in a state is something that occurs repeatedly", a state can carry bounds Δt1 < Δt2; for the lower bound, "if they are to cause
  exits, events do not apply in the state until the lower bound is reached" — [Harel 1987](https://www.state-machine.com/doc/Harel87.pdf)
- Actions and activities (§5, pp. 256-257): transitions are labelled α(P)/S (event, guard, action); actions are instantaneous and
  can themselves be events sensed in orthogonal components; activities "take a nonzero amount of time" and are controlled with
  start(X), stop(X) and the condition active(X); actions can be attached to entry, exit and "throughout" a state — [Harel 1987](https://www.state-machine.com/doc/Harel87.pdf)
- Harel, Pnueli, Schmidt and Sherman (LICS 1987) formalise the semantics, including multi-level concurrency with a broadcast
  mechanism that can produce chain-reaction effects — [LICS 1987 archive](https://lics.siglog.org/archive/1987/HarelPnueliSchmidtS-OntheFormalSemantic.html)
- State explosion: "state machines often end up with a large number of states, a lot of them with identical transitions";
  "adding a new aspect to the state machine multiplies the number of states that need to be modeled" and "creates a
  disproportionately high number of transitions" — [statecharts.dev: State explosion](https://statecharts.dev/state-machine-state-explosion.html)
- Parallel regions fix it for independent aspects: "Adding more regions does not cause an explosion of states; ... the
  statechart grows more or less linearly as more features are added"; when an aspect only matters inside another state, nest it:
  "By leveraging hierarchies, the state explosion problem goes away too" — [statecharts.dev: State explosion](https://statecharts.dev/state-machine-state-explosion.html)
- SCXML 1.0 is a W3C Recommendation dated 1 September 2015 — [W3C SCXML](https://www.w3.org/TR/scxml/)
- Compound states: "When a compound state is active, one and only one of its child states is active" (§3.1.2). Parallel: "When a
  <parallel> element is active, all of its children are active", and when all children reach final states it raises
  done.state.id (§3.1.3) — [W3C SCXML](https://www.w3.org/TR/scxml/)
- History: shallow history "remembers only which immediate child was active"; deep history records the full set of active atomic
  descendants (§3.1.4) — [W3C SCXML](https://www.w3.org/TR/scxml/)
- Guards: transitions carry a `cond`; "If the 'event' attribute is missing, the transition is taken whenever the 'cond' evaluates
  to true" (eventless transitions, §3.1.1); eventless transitions are checked on entering a state before event-driven ones
  (§3.12.1) — [W3C SCXML](https://www.w3.org/TR/scxml/)
- Run-to-completion: a microstep executes an optimal enabled transition set; a macrostep is "a series of one or more microsteps
  ending in a configuration where the internal event queue is empty"; only then does the processor take events from the external
  queue (§3.13); generated events go to the rear of the internal queue (§4.2) — [W3C SCXML](https://www.w3.org/TR/scxml/)
- Errors are events: errors are signalled by raising events named `error.*` (`error.communication`, `error.execution`) on the
  queue (§3.12.2); a failing element stops the rest of its executable block (§4.9); a condition expression that errors is treated
  as false and `error.execution` is queued (§5.9.1) — [W3C SCXML](https://www.w3.org/TR/scxml/)
- Timers: "The 'delay' attribute allows for deferred event delivery and can be used to implement a timer" (§6.1); `<send>`
  arguments are evaluated when `<send>` runs, not at dispatch, and the message is dispatched only once the delay elapses; if the
  session terminates first the message is discarded (§6.2.3) — [W3C SCXML](https://www.w3.org/TR/scxml/)
- `<cancel sendid>` cancels a delayed send, best effort: it "SHOULD make its best attempt"; it cannot succeed if the event was
  already delivered; all delayed events sharing the sendid are cancelled; it cannot affect another session's events (§6.3) —
  [W3C SCXML](https://www.w3.org/TR/scxml/)

### Inferences
- SCXML's "a guard that errors is false AND an `error.execution` event is raised" is the exact pattern Remit lacks for
  problem (a): a CEL condition that errors must not silently read as "no match"; it should both not fire and leave a recorded,
  queryable error event against the subscription.
- Parallel regions map onto the subscription lifecycle: "enabled/paused/ended" (owner control), "held(reason)/healthy" (failure
  state) and "idle/cooling/due" (time window) are independent aspects. Flattened, they multiply (3x2x3 = 18 combinations), which is
  why today's implicit field soup is hard to reason about. Modelled as orthogonal regions (or three separate small enums with
  explicit transitions), each stays linear.
- Named, cancellable delayed events (`<send delay id>` + `<cancel sendid>`) are the right primitive for (e): a cooldown or
  deadline is a timer owned by the subscription state, entered on transition and cancelled on exit, rather than a separate clock
  sensor computing elapsed time.
- Macrostep semantics (drain internal events before the next external reading) is a precedent for processing one reading
  to completion against all subscriptions under one write, which the global mutex already approximates.

- Harel's state bound (a state that may be left only after Δt1 and must be left by Δt2) is a cooldown (lower bound) and a deadline
  (upper bound) as properties of a state, which is exactly the subscription "window" in (e). Expressed this way, cooldown and
  deadline belong to the subscription's state, not to a separate clock sensor.
- Harel's activity model (start(X)/stop(X)/active(X), "throughout" a state) fits a running processor or job: entering an
  "under way" state starts it, leaving stops it, and a guard can ask whether it is still active, which is the hook for an
  overlap policy and a timeout.

### Gaps
- Harel's paper was read from page images of a scanned copy (no text layer); quotes were transcribed from those images.
- SCXML does not specify persistence of delayed sends across process restarts; it only says they are discarded when the session
  terminates.

## 2. XState v5 / Stately: actors, persisted snapshots, delayed events, inspection, machine-as-data

### Takeaway
XState v5 treats every machine as an actor whose state can be persisted deeply (`getPersistedSnapshot`) and restored
(`createActor(logic, { snapshot })`) without re-running actions; it also exposes pure `transition(machine, state, event) ->
[nextState, actions]` functions and an inspection event stream (actor, event, snapshot, microstep) that tools build on. Its docs
are silent on durable timers and offer no versioning story beyond "restored state may be incompatible".

### Cited Findings
- Persist with `actor.getPersistedSnapshot()`; "persisted state represents the internal state of the actor, while snapshots
  represent the actor's last emitted value"; any actor can be persisted, not only machines — [Stately: Persistence](https://stately.ai/docs/persistence)
- Restore with `createActor(logic, { snapshot: restoredState }).start()`; "Actions from machine actors will not be re-executed,
  because they are assumed to have been already executed"; "invocations will be restarted, and spawned actors will be restored
  recursively"; persistence "is deep" (children included) — [Stately: Persistence](https://stately.ai/docs/persistence)
- Event sourcing as the alternative: replaying events "can be more reliable than persisting state, because it is less prone to"
  incompatible state, and it lets actions be replayed; one method is recording `@xstate.event` via inspection and re-sending — [Stately: Persistence](https://stately.ai/docs/persistence)
- Caveats: "if the machine or actor logic changes, the restored state may be incompatible with the new logic"; state must be
  JSON-serializable; the page gives no versioning or migration guidance — [Stately: Persistence](https://stately.ai/docs/persistence)
- Delayed transitions: defined on a state's `after` property (ms or a named delay); "If another event occurs before the end of
  the timer, the transition doesn't complete"; delays can be functions of context (e.g., `context.attempts * 1000`); "Delayed
  transition timers are canceled when the state is exited"; testing uses a simulated clock. The page says nothing about timer
  durability across restore — [Stately: Delayed transitions](https://stately.ai/docs/delayed-transitions)
- Pure functions: `initialTransition(machine, input?)` returns `[initialState, initialActions]`; `transition(machine, state, event)`
  "Computes the next state and actions given a current state and event" and returns `[nextState, actions]`; actions are objects
  with `type` and `params`; "The pure functions capture these actions but don't execute them"; `getMicrosteps` returns each
  intermediate `[snapshot, actions]`; restore a persisted snapshot via `machine.resolveState()` then call `transition`; pitched as
  "Perfect for server-side workflows and API endpoints" and easy to test — [Stately: Pure transitions](https://stately.ai/docs/pure-transitions)
- Inspection: an `inspect` option on `createActor` receives events for every actor in the system: `@xstate.actor` (created),
  `@xstate.event` (target, `sourceRef`, event), `@xstate.snapshot` (new snapshot + causing event), `@xstate.microstep` (value after
  the microstep, transitions taken). Microsteps expose transient states that `subscribe()` never shows. Stately Inspector is built
  on this API — [Stately: Inspection](https://stately.ai/docs/inspection)

### Inferences
- The pure `transition(state, event) -> (state, actions-as-data)` signature is the shape Remit wants for subscription
  evaluation: `(subscription state, reading | tick) -> (new state, [fire action | hold(reason) | record skip(reason)])`. The shell
  executes the actions; the core runs in tests without a clock or DB.
- "Actions are not re-executed on restore" plus "invocations restart" is the exact durability contract an idempotent firing table
  (keyed subscription, key, revision) already gives: persisted state + idempotency key means a restart re-derives, never re-fires.
- XState's silence on durable timers is a warning: in-memory `after` timers are lost on restart. Remit should persist a
  timer as data (a `due_at` column the scheduler tick scans), the same as Step Functions' Wait and Durable Entities' scheduled
  operations, never as a goroutine timer.
- The inspection stream (event in, snapshot out, transitions taken) is the model for (g): if every subscription evaluation emits
  a record of reading seen, guard result, transition taken and action emitted, an AI agent debugging a subscription reads that
  record through a tool instead of reasoning over code.
- Machine definitions in XState are plain config objects, which is why visual editors and inspectors can read them; a
  subscription whose lifecycle is a declared table (states, events, guards) is likewise something an agent tool can render and
  validate before Apply (dry run, (b)).

- The next major version addresses durable timers explicitly: xstate@6.0.0-alpha.63 adds an experimental
  `execution.restore(persistedSnapshot)` in `xstate/durable` "for resuming checkpoints without sending a synthetic event or
  replaying entry actions"; callers "Execute the returned effects to resume active embedded children and pending timers", and
  "Timers with a persisted wall-clock start keep their original deadline" (example: `const [snapshot, effects] =
  execution.restore(checkpoint.snapshot); await execution.executeEffects(effects);`) — [xstate@6.0.0-alpha.63 release](https://github.com/statelyai/xstate/releases/tag/xstate@6.0.0-alpha.63)
- XState v5 source on `main` (package version 5.33.3, the current npm `latest`; line numbers below are for `main`): delayed events
  live in the actor system's scheduler as `ScheduledEvent { startedAt, delay, id, event, source, target }` (system.ts:16-17),
  with `startedAt: Date.now()` set when scheduled (system.ts:120) and armed with `clock.setTimeout`; `createActor` builds the system
  with only `{ clock, logger }` (createActor.ts:159-162), so `createSystem` seeds `_scheduledEvents` from `options.snapshot.scheduler`
  (system.ts:172-173) with no snapshot to read; `getSnapshot()` writes the key `_scheduledEvents` (system.ts:230); `start()`
  re-schedules each stored event with its original `delay`, not the remaining time from `startedAt` (system.ts:239). Tag
  xstate@5.19.0 has the same code a few lines earlier — [xstate main system.ts](https://github.com/statelyai/xstate/blob/main/packages/core/src/system.ts); [xstate main createActor.ts](https://github.com/statelyai/xstate/blob/main/packages/core/src/createActor.ts); [xstate@5.19.0 system.ts](https://github.com/statelyai/xstate/blob/xstate%405.19.0/packages/core/src/system.ts)
- Reproduction run 2026-11-05 with xstate 5.33.3 from npm (Node): a machine `cooling --after 300ms--> due`, started, persisted with
  `getPersistedSnapshot()` while in `cooling`, JSON round-tripped, restored with `createActor(m, { snapshot }).start()`. The persisted
  snapshot's keys were `status, output, error, value, historyValue, context, children` (no scheduler or scheduled events). After
  1000 ms the restored actor was still `cooling`; a fresh control actor had moved to `due`. Output: `after 1000ms restored: cooling
  | fresh control: due`. Source: own run of the script described here against [xstate 5.33.3 on npm](https://www.npmjs.com/package/xstate)
- A user in a v4-to-v5 migration discussion reports it is not possible to start a restored but not-yet-started invoked actor or
  timer (user report) — [statelyai/xstate discussion #4856](https://github-redirect.dependabot.com/statelyai/xstate/discussions/4856)

### Inferences (durable timers)
- A v5 actor restored from a persisted snapshot comes back without its pending `after` timers and stays in the waiting state
  indefinitely (HIGH: reproduced above). Even if the scheduler were seeded, each timer would wait its full delay again because
  `startedAt` is never read (from source). Any durable use of XState v5 timers needs the host to persist deadlines itself.
- Stately's own direction (v6 `xstate/durable`) converges on what Durable Entities and Step Functions already do: a timer is
  persisted with its wall-clock start/deadline, and restore returns effects (re-arm timers) as data for the host to execute. That
  is the design to copy for subscription cooldowns and deadlines: persist `due_at`, and on startup the scheduler re-derives what
  is due from stored rows rather than from in-memory timers.

### Gaps
- The v5 docs (persistence, delayed transitions) do not mention the timer loss shown by the reproduction. `xstate/durable` is an alpha of v6 and its API may change; the
  release page did not show the year of the release.
- No official Stately guidance found on migrating persisted snapshots when the machine definition changes.

## 3. AWS Step Functions (Amazon States Language) as a durable state machine

### Takeaway
Step Functions Standard workflows persist execution state between every transition (exactly-once, up to one year, 90-day
history, 25,000-event cap), express time as Wait states (relative or absolute, up to one year), bound every task with
TimeoutSeconds/HeartbeatSeconds, handle errors with ordered Retry/Catch lists keyed by named error types, and give users a
per-execution graph, step input/output, retry tab and full event history; Express trades all of that for at-least-once and no
stored history.

### Cited Findings
- Standard: up to one year, "durable, and auditable"; exactly-once ("tasks and states are never run more than once, unless you
  have specified Retry"); full history via API for 90 days; billed per state transition. Express: up to five minutes,
  at-least-once (async) or at-most-once (sync), billed per execution/duration/memory; workflow type cannot be changed after
  creation — [AWS: Choosing workflow type](https://docs.aws.amazon.com/step-functions/latest/dg/choosing-workflow-type.html)
- Standard "Execution state internally persists between state transitions" and starting an execution with the same name as a
  running one returns an idempotent response; Express does not persist between transitions and does not manage idempotency
  ("Can result in loss of internal workflow state if state machine logic is not idempotent"); Express history is not captured,
  only CloudWatch Logs — [AWS: Choosing workflow type](https://docs.aws.amazon.com/step-functions/latest/dg/choosing-workflow-type.html)
- Wait state: exactly one of `Seconds` (0-99,999,999), `Timestamp` (RFC3339, truncated to seconds), `SecondsPath`,
  `TimestampPath`; maximum wait is one year (Standard) and five minutes (Express) — [AWS: Wait state](https://docs.aws.amazon.com/step-functions/latest/dg/state-wait.html)
- Task `TimeoutSeconds`: max time before `States.Timeout`; default depends on task type, and "Activities, and the wait for a
  .sync job or a .waitForTaskToken callback, have no default timeout. They can last until the execution reaches its maximum
  duration." `HeartbeatSeconds` must be less than TimeoutSeconds, default 99,999,999; missing a heartbeat fails the task with
  States.Timeout. AWS: "It's a good practice to set a timeout value and a heartbeat interval for long-running activities." — [AWS: Task state](https://docs.aws.amazon.com/step-functions/latest/dg/state-task.html)
- Default on error: "Step Functions defaults to failing the entire state machine execution." All states except Pass and Wait can
  error — [AWS: Error handling](https://docs.aws.amazon.com/step-functions/latest/dg/concepts-error-handling.html)
- Named errors: `States.ALL` (wildcard, must be alone and last; cannot catch `States.DataLimitExceeded` or `States.Runtime`),
  `States.Timeout`, `States.HeartbeatTimeout`, `States.TaskFailed` (wildcard except Timeout), `States.Permissions`,
  `States.Runtime` (not retriable, always fails the execution) — [AWS: Error handling](https://docs.aws.amazon.com/step-functions/latest/dg/concepts-error-handling.html)
- Retrier fields: `ErrorEquals` (required), `IntervalSeconds` (default 1), `MaxAttempts` (default 3; 0 = never retry),
  `BackoffRate` (default 2.0), `MaxDelaySeconds` (cap; otherwise unbounded), `JitterStrategy` (`FULL`|`NONE`, default `NONE`);
  retriers are scanned in order; "Retries are treated as state transitions"; a retrier's count applies across all visits within one
  state execution — [AWS: Error handling](https://docs.aws.amazon.com/step-functions/latest/dg/concepts-error-handling.html)
- Catchers: `ErrorEquals`, `Next` (fallback state), `ResultPath` (where the error output goes); retriers run first, then catchers;
  error output carries a human-readable `Cause`. Catchers do not cover top-level execution failure; AWS suggests listening for
  `TIMED_OUT` events via EventBridge — [AWS: Error handling](https://docs.aws.amazon.com/step-functions/latest/dg/concepts-error-handling.html)
- Debugging: execution page shows status, definition, state-transition count, input/output; a failed execution shows an error
  banner linking to the failing step; Graph and Table views (with a Timeline column showing retries in red and backoff in grey);
  Step details tabs Input, Output (with "Advanced view" of the data path), Details (including configured TimeoutSeconds and
  HeartbeatSeconds), Definition, Retry ("Retries & redrives"), Events; full Events table with a 25,000-event maximum history;
  Export to JSON; Redrive of unsuccessful Standard executions within 14 days (retry counts reset to 0 on redrive) — [AWS: Viewing execution details](https://docs.aws.amazon.com/step-functions/latest/dg/concepts-view-execution-details.html); [AWS: Error handling](https://docs.aws.amazon.com/step-functions/latest/dg/concepts-error-handling.html)
- Versioning hazard in Express: definitions for past executions are not stored, so after a definition change the Definition,
  Graph and Table views are unavailable for older executions — [AWS: Viewing execution details](https://docs.aws.amazon.com/step-functions/latest/dg/concepts-view-execution-details.html)

### Inferences
- (f): Step Functions' default is "no timeout" only for human-paced waits, and AWS still recommends setting one. A plugin
  processor is not a human wait: Remit should give every processor a mandatory timeout (and optionally a heartbeat), surface
  a typed `timeout` error, and never run it inside the global write lock; the lock should cover only the commit of its result.
- (a): Step Functions' rule "an unhandled error fails the execution, visibly" plus named error classes and ordered Retry/Catch is
  the cure for silent skips: each subscription firing should end in an explicit terminal outcome (fired, skipped(reason),
  failed(error class), held) and an unhandled error should move the subscription to a visible `held` state, never be dropped.
- Retry as policy-as-data (`ErrorEquals`, `IntervalSeconds`, `MaxAttempts`, `BackoffRate`, `MaxDelaySeconds`, `JitterStrategy`)
  is a ready schema for a subscription's failure handling instead of per-action ad hoc code.
- Storing the definition with each execution (Standard) and losing it (Express) is a direct lesson for (g) and versioning: record
  the subscription revision on every firing (Remit already keys firings by revision) so a trace always renders against the
  definition that produced it.
- Redrive (resume an unsuccessful execution from the failed step with fresh retry counts) is precedent for "replay this
  subscription from cursor X" as an explicit owner/agent action, relevant to (b).

- TestState API (dry run of one state): it "accepts the definition of a state and executes it" without creating or updating a
  state machine; it takes a single state definition or a full definition plus `stateName`; inspection levels INFO (output or
  error), DEBUG (adds `inspectionData`: `input`, `afterInputPath`, `afterParameters`, `result`, `afterResultSelector`,
  `afterResultPath`, `output`) and TRACE (HTTP request/response, HTTP Task only); every level returns `status` (`SUCCEEDED`,
  `FAILED`, `RETRIABLE`, `CAUGHT_ERROR`) and `nextState`. Since November 2025 it can mock service results or errors (`--mock`
  with `result` or `errorOutput`, validated STRICT/PRESENT/NONE against the service API model), set context and
  `retrierRetryCount`, and report which retrier/catcher applied (`retryIndex`, `retryBackoffIntervalSeconds`, `catchIndex`);
  tests can be chained by feeding `output` and `nextState` into the next call. Without a mock it calls real services under an IAM
  role — [AWS: Testing state machines with TestState API](https://docs.aws.amazon.com/step-functions/latest/dg/test-state-isolation.html)

### Inferences (dry run)
- TestState is the closest primary-source precedent for (b) and (g): give the agent one call that takes a subscription
  definition (or a revision of it) plus an input reading, runs the real evaluator with actions mocked, and returns the outcome,
  the next lifecycle state, and the intermediate values (key match, CEL result, cooldown check), the same shape as
  `status` + `nextState` + `inspectionData`. Running it over a stored range of readings gives the backfill preview.

### Gaps
- None remaining for this section within scope.

## 4. Durable entity state machines: Azure Durable Entities, Akka event-sourced behaviors

### Takeaway
Durable entities (Azure) and event-sourced behaviors (Akka) both give each entity an identity, serialize its operations, persist
state after each operation, and separate a pure state transition from side effects; Akka makes the state machine explicit by
choosing the command handler by current state, and both treat changes to in-flight logic as the main versioning risk.

### Cited Findings
- Durable Entities: each entity has a unique identity and optional state; "To prevent conflicts, a single entity runs operations
  serially, one after another"; messages travel through reliable queues; "The runtime always persists the entity state in
  storage" — [Microsoft Learn: Durable entities](https://learn.microsoft.com/en-us/azure/azure-functions/durable/durable-functions-entities)
- Entity ID is (entity name, entity key), e.g. `@Counter@Game1`; an operation has entity ID, operation name, optional input and an
  optional **scheduled time** ("schedule an operation to run several days later") — [Microsoft Learn: Durable entities](https://learn.microsoft.com/en-us/azure/azure-functions/durable/durable-functions-entities)
- Signal = one-way: "The runtime guarantees delivery, but the sender can't observe result values or errors"; call = two-way, only
  from orchestrations — [Microsoft Learn: Durable entities](https://learn.microsoft.com/en-us/azure/azure-functions/durable/durable-functions-entities)
- Queried entity state "is always a 'committed' state, so it's never a temporary intermediate state ... But this state can be stale"
  — [Microsoft Learn: Durable entities](https://learn.microsoft.com/en-us/azure/azure-functions/durable/durable-functions-entities)
- Critical sections: `LockAsync` on a set of entities queues all other operations on them until release; "Locks on entities are
  durable"; critical sections "don't automatically roll back changes when errors occur" by design — [Microsoft Learn: Durable entities](https://learn.microsoft.com/en-us/azure/azure-functions/durable/durable-functions-entities)
- Akka `EventSourcedBehavior` = `persistenceId`, `emptyState`, `commandHandler`, `eventHandler`; "The event handler must only
  update the state and never perform side effects"; command handlers return effect directives (`persist`, `none`, `unhandled`,
  `stop`, `stash`, `reply`, `async`); side effects chained with `thenRun` run "on an at-most-once basis and will not be executed if
  the persist fails" — [Akka: Event Sourcing](https://doc.akka.io/libraries/akka-core/current/typed/persistence.html)
- Recovery replays journaled events through the same event handler; snapshots optimise recovery; selecting command handlers per
  state type (`forStateType`) "is useful when implementing finite state machine (FSM) like entities"; schema evolution requires
  that "it must be possible to read old events when the application has evolved"; event adapters convert events on the way to the
  journal — [Akka: Event Sourcing](https://doc.akka.io/libraries/akka-core/current/typed/persistence.html)
- Durable Functions versioning: breaking changes include changing an activity/entity function's name, input or output, or adding,
  removing or reordering calls/timers/external events in orchestrator code; replay then raises a non-deterministic orchestration
  error; "Deploying breaking changes without a mitigation strategy ... can cause orchestrations to fail ..., get stuck indefinitely
  in a Running status"; recommended mitigation is orchestration versioning ("Each orchestration instance gets a version
  permanently associated with it when created"; newer workers run older instances, older workers refuse newer ones); alternatives
  are side-by-side deployment or stopping all in-flight instances — [Microsoft Learn: Durable Functions versioning](https://learn.microsoft.com/en-us/azure/azure-functions/durable/durable-functions-versioning)

### Inferences
- A subscription is a natural durable entity: identity = subscription id (or subscription x key), operations = `reading`, `tick`,
  `pause`, `resume`, `edit`; serial processing per entity is what the global mutex provides today, at coarser grain. Per-entity
  serialization is the seam that would let a slow processor stop blocking unrelated subscriptions (f).
- Akka's split (command handler decides, returns events/effects; pure event handler applies; side effects after persist,
  at-most-once) is the same functional-core shape as section 5, and its "side effect only after successful persist" rule matches
  Remit recording the firing row before (or atomically with) dispatching the action.
- Durable Entities' "scheduled time" on an operation is one primitive covering cooldown expiry, deadlines and routine due times,
  which argues for unifying (e): a timer is a scheduled operation on the subscription/routine entity.
- Versioning lesson: stamp each subscription firing and each in-flight wait with the definition revision that created it, and
  decide explicitly whether an edit re-evaluates in-flight cooldowns/deadlines or lets them finish on the old revision.

- Akka's dedicated FSM persistence class is retired: `PersistentFSM` is "Deprecated and removed except for the event and snapshot
  types which are needed for migrating existing PersistentFSM actors to EventSourcedBehavior" via
  `akka.persistence.typed.scaladsl.PersistentFSMMigration` / `javadsl.PersistentFSMMigration`; `StateChangeEvent` is kept
  ("Persisted on state change Not deprecated as used for users migrating from PersistentFSM to EventSourcedBehavior") — [Akka API: akka.persistence.fsm](https://doc.akka.io/japi/akka/current/akka/persistence/fsm/package-summary.html); deprecated since Akka 2.6.0 per [Akka 2.8 API: PersistentFSM$](https://doc.akka.io/api/akka-core/2.8/akka/persistence/fsm/PersistentFSM$.html)

### Inferences (Akka)
- Akka dropped a special FSM abstraction in favour of a plain event-sourced entity whose command handler is chosen by state. That
  supports modelling Remit's lifecycle as ordinary typed state plus a transition function, not a separate FSM framework.
- Keeping old event and snapshot types alive only so stored data can migrate forward is the same rule as Remit's numbered
  migrations: stored shapes migrate once; runtime code reads one shape.

- `PersistentFSMMigration` ("Helper functions for migration from PersistentFSM to Persistence Typed") offers `snapshotAdapter(adapt)`,
  which will "adapt snapshots created by a PersistentFSM into the correct State type of a EventSourcedBehavior"; `adapt` "Takes in
  the state identifier, snapshot persisted by the PersistentFSM and the state timeout and returns the State that should be given
  to the EventSourcedBehavior" (Scala signature `Function3[String, Any, Option[FiniteDuration], State]`) — [Akka API: PersistentFSMMigration$](https://doc.akka.io/japi/akka-core/current/akka/persistence/typed/scaladsl/PersistentFSMMigration$.html)
- Worked migration (Apache Pekko, the Akka fork, same API): the snapshot adapter maps each old state identifier plus data to the
  new state (e.g. `"Shopping"` to `Shopping(cart)`, throwing on unknown identifiers); an event adapter drops `StateChangeEvent`
  (returns `EventSeq.empty`) because "the state transitions can be inferred from the events", or converts it to a private event if
  the state identifier or timeout is needed; "there is no explicit support for state timeout as with PersistentFSM", so timeouts
  become timers, and "Care must also be taken to restart timers on recovery in the signal handler": on `RecoveryCompleted`, for
  states `Shopping` and `Inactive`, call `timers.startSingleTimer(StateTimeout, Timeout, 1.second)` — [Pekko: Persistent FSM migration](https://pekko.apache.org/docs/pekko/current/persistence-fsm.html)
- The typed FSM pattern: "EventSourcedBehavior can be used to represent a persistent FSM"; the command handler matches on current
  state then command and turns commands into events, the event handler maps (state, event) to the next state, and a timer is
  started after persist, e.g. `Effect.persist(ItemAdded(item)).thenRun(_ => timers.startSingleTimer(StateTimeout, Timeout, 1.second))`
  — [Pekko: EventSourced behaviors as finite state machines](https://pekko.apache.org/docs/pekko/current/typed/persistence-fsm.html)

### Inferences (Akka timers)
- Akka/Pekko and XState v5 share one trap: timers are in-memory and must be re-armed by hand on recovery, and Pekko's example
  re-arms with a fresh full delay rather than the remaining time. Remit avoids the whole class by storing `due_at` and letting
  the scheduler tick compare it to now, so recovery needs no re-arm step. MEDIUM.

### Gaps
- None remaining for the Akka migration within scope.
- No primary source found on Durable Entities' storage format (snapshot vs event history) for entity state.

## 5. Pure functional core: `(state, event) -> (state, effects)` and effect-as-data

### Takeaway
Several mature systems converge on one shape: a pure transition returns the next state plus effects described as data, and a
runtime executes those effects (XState `transition` returning action objects, Akka command handlers returning `Effect`
directives, Elm programs returning `Cmd`/`Sub` to the runtime); this makes rule evaluation testable without I/O and makes
"what would happen" (dry run) the same code path as "what happens".

### Cited Findings
- XState `transition(machine, state, event)` returns `[nextState, actions]` and "The pure functions capture these actions but don't
  execute them" — [Stately: Pure transitions](https://stately.ai/docs/pure-transitions)
- Akka command handlers return effect directives; the event handler "must only update the state and never perform side effects";
  `thenRun` side effects run after a successful persist — [Akka: Event Sourcing](https://doc.akka.io/libraries/akka-core/current/typed/persistence.html)
- Elm: "In addition to producing Html values, our programs will also send Cmd and Sub values to the runtime system"; programs
  "command the runtime system to make an HTTP request or to generate a random number" and "subscribe to the current time"; the
  runtime turns input into a `Msg` and feeds it back in — [Elm guide: Effects](https://guide.elm-lang.org/effects/)
- SCXML expresses timers as data too: a delayed `<send>` with an id, cancellable by `<cancel sendid>` — [W3C SCXML §6.2-6.3](https://www.w3.org/TR/scxml/)

### Inferences
- For Remit, the core could be `Evaluate(sub Subscription, now time.Time, reading *Reading) (Subscription, []Effect)` with
  `Effect` a closed sum: `Fire{action, key, revision}`, `Skip{reason}`, `Hold{reason}`, `ScheduleTimer{due}`, `AdvanceCursor{seq}`.
  Every `Skip` and `Hold` becomes a recorded row, closing (a).
- Dry run (b) falls out for free: run the same pure function over a historical range of readings from a chosen start seq and
  return the effects without executing them. Backfill is the same loop with the effects executed.
- Time as an input (`now`) rather than a read of the clock makes cooldown/deadline/overlap rules testable and unifies (e): the
  clock sensor becomes just another event source feeding `tick` events.
- Effects as data are also what an AI agent can read: a tool returning "for reading #1234 your subscription would Skip(cooldown
  until 10:05)" answers (g) without the agent reading Go.

### Gaps
- Did not fetch Gary Bernhardt's "Functional core, imperative shell" talk or the Redux style guide; claims about those are not
  cited here.

## 6. Home Assistant automations: trigger/condition/action, modes, traces, blueprints

### Takeaway
Home Assistant automations are a widely used rule engine with an explicit trigger -> condition -> action split, edge-triggered
semantics ("a trigger reacts at a specific moment, not while something is true"), an explicit overlap policy per automation
(single/restart/queued/parallel with max and a log level for overflow), and per-run traces (last 5 by default, plus 5 "did not
trigger" traces) that record trigger, condition results, actions, changed variables, timeline, related activity and the config at
run time. Its timers (`for`, delays, waits) do not survive restart.

### Cited Findings
- Edge semantics: "A trigger reacts at a specific moment, not while something is true"; a door already open when the automation is
  saved does not trigger until it closes and reopens — [HA: Triggers](https://www.home-assistant.io/docs/automation/trigger/)
- Trigger types include State changed, Numeric state crossed threshold, Template, Time, Time pattern, Webhook, MQTT, Event, Sun,
  Zone, Tag, Sentence, Home Assistant; most entity triggers do not react to transitions from `unavailable`/`unknown`; trigger IDs
  default to list position and need not be unique; trigger variables are either set at fire time or at attach time (limited
  templates, for blueprint inputs) — [HA: Triggers](https://www.home-assistant.io/docs/automation/trigger/)
- State trigger: `from`/`to` take a state, a list or `null`; `null` matches any state while ignoring attribute-only changes;
  `not_from`/`not_to` exclude; `attribute` watches an attribute; with neither `from` nor `to` it "fires on all state changes. It also
  fires when only an attribute changes"; `for` requires the new state to hold for the duration, and "a change from one of the
  states to another starts the time again"; "the timer resets if Home Assistant restarts or automations reload" — [HA: State changed trigger](https://www.home-assistant.io/triggers/state/)
- Modes (default `single`): `single` "Doesn't start a new run while the automation is running, and logs a warning"; `restart`
  "Stops the current run, including a delay or a wait that is running, and starts a new run"; `queued` "Starts the new run after
  all previous runs have finished" (in trigger order; joins the queue only if conditions pass); `parallel` "Starts a new,
  independent run right away". `max` (queued/parallel only) default 10, minimum 2. `max_exceeded` (YAML only) sets the log level
  of the overflow message, default `warning`, `silent` suppresses it. Messages: "Already running", "Maximum number of runs
  exceeded" — [HA: Automation modes](https://www.home-assistant.io/docs/automation/modes/)
- Durability: "Restarting Home Assistant stops all runs. Any delay, wait, or trigger duration in progress is lost"; a timer that
  expires while HA is off does not fire on start — [HA: Automation modes](https://www.home-assistant.io/docs/automation/modes/)
- Community reports confirm `for` cannot persist across restart and workarounds use a restorable timer helper or a stored "since"
  timestamp checked by a template trigger — [HA Community: for through restart](https://community.home-assistant.io/t/how-to-make-trigger-with-for-work-through-ha-restart/988889); [HA Community: not triggering after restart](https://community.home-assistant.io/t/automation-not-triggering-after-ha-restart-when-using-for-duration-in-state-trigger/945359) (anecdotal)
- Traces: a trace is "a step-by-step record of what happened": the trigger, whether each condition passed, what each action did,
  which variables changed; views: Step details (Step config, Changed variables, Related activity), Trace timeline ("The steps that
  ran, and when they ran"), Automation config ("The configuration of the automation at the time it ran"), Blueprint config — [HA: Testing automations](https://www.home-assistant.io/docs/automation/testing/)
- Trace retention: "the last 5 traces of each automation"; "Did not trigger" traces are stored separately and don't replace real
  runs; `trace: stored_traces: N` (YAML only) keeps N runs plus N did-not-trigger traces; traces "are kept when Home Assistant
  restarts" but are lost on an unexpected stop; an automation without an `id` has no traces; download as JSON; traces may contain
  secrets and should be redacted before sharing — [HA: Testing automations](https://www.home-assistant.io/docs/automation/testing/)
- Run outcomes shown in the trace: "Stopped because a condition failed", "Stopped on error", "Stopped because only a single
  execution is allowed"; "If the list of traces has no run for that moment, the trigger didn't react"; conditions are checked only
  once right after the trigger; a "Wait for a trigger" never finishes if the awaited state already holds — [HA: Troubleshooting automations](https://www.home-assistant.io/docs/automation/troubleshooting/)
- Test tools: test a single condition in isolation (without trigger data, so may be unreliable), run a single action, "Run actions"
  skipping triggers and conditions, trigger manually with or without conditions, simulate a trigger by setting a state or firing an
  event — [HA: Testing automations](https://www.home-assistant.io/docs/automation/testing/)
- Entity activity records what caused a change and links back to the trace of the automation that caused it — [HA: Troubleshooting automations](https://www.home-assistant.io/docs/automation/troubleshooting/)
- Blueprints: "ready-made automations shared by the Home Assistant community" with configurable inputs; re-importing overwrites
  the blueprint and "If the re-imported blueprint is not compatible, it can break your automations"; after reload, blueprint changes
  apply to existing automations; "Take control" converts an instance into a regular automation, one-way — [HA: Using blueprints](https://www.home-assistant.io/docs/automation/using_blueprints/)

### Inferences
- (e) overlap policy: HA makes overlap a single named field with four values and a bound (`max`) plus an explicit, logged overflow
  outcome. Remit's implicit overlap (what happens when a subscription matches while its previous action is still running or
  its cooldown is active) should become an enum on the subscription (e.g. skip/restart/queue/parallel with max) whose overflow is
  a recorded `Skip{reason: already_running | max_exceeded}`, never silent.
- (a) and (g): HA's split between "run traces" and "did not trigger" traces is the key idea: record why something did NOT happen,
  in a separate bounded buffer so noise does not evict real runs. A subscription tool for agents should answer both "last N
  firings with guard results" and "last N near-misses (key matched, condition false / cooldown / held)".
- Storing "the configuration at the time it ran" with each trace pairs with the (subscription, key, revision) firing key: the trace
  should render against that revision.
- HA's lost-on-restart `for` timers are a counter-example: Remit should keep durations as persisted `due_at` values so a
  restart neither loses nor double-fires them.
- Edge vs level: HA triggers are edge-only and its docs repeatedly warn about the consequence (already-true states never fire;
  wait-for-trigger hangs if already true). This is (d) in miniature: a "signal" must say whether it is a state (level, has a
  current value) or a stream (edges/events), and a subscription on a state needs an explicit "evaluate current value on start"
  option, which is also the start-position question in (b).
- Blueprints map to playbook/subscription templates an agent instantiates; HA's "re-import can break your automations" and
  one-way "Take control" show the versioning trade-off between linked templates and forked copies.

- Template trigger: "The trigger reacts when the template changes from false to true" (`true`, non-zero numbers, `yes`, `on`,
  `enable` count as true); the template is re-rendered "when one of the entities in the template changes", templates using
  `now()`/`utcnow()` re-render at the start of every minute, and templates with neither are rendered once; with `for`, "the
  template must stay true for the entire time", and a numeric result that changes while still true (e.g. 5 to 6) stops the
  timer; "the timer resets when Home Assistant restarts or automations reload", and the page suggests a date-and-time helper
  plus a Time trigger as the durable alternative; a template already true at setup does not fire; render errors cause no
  reaction and a logged warning — [HA: Template trigger](https://www.home-assistant.io/triggers/template/)

### Inferences (template trigger)
- HA's own remedy for lost `for` timers is to store an absolute time (a date-and-time helper) and trigger on it, the same
  persisted `due_at` design recommended above for (e).
- "A render error causes no reaction and only a log warning" is HA's version of problem (a); in HA it is visible only in logs, not
  in the automation's trace list. Remit should record a CEL evaluation error against the subscription itself.

### Gaps
- HA does not document a "start position" or backfill: automations only react to live changes.

## 7. When explicit state machines are overkill vs worth it; pitfalls

### Takeaway
State machines pay off when several independent aspects interact (state explosion) and when lifecycle state must be persisted,
inspected and debugged; for a few booleans an enum suffices. The main practical pitfalls are flat-FSM explosion (solved by
hierarchy/parallel regions), developer unfamiliarity/YAGNI, and versioning persisted state when the definition changes, which every
durable engine studied handles explicitly (Durable Functions orchestration versioning, Akka event adapters, Step Functions storing
definitions) and XState leaves to the user.

### Cited Findings
- "Anecdotal evidence suggests that the main reason for statecharts not being used is the YAGNI mindset"; "the easiest thing that
  could possibly work will always be a set of implicit booleans"; "It's as if you don't need statecharts until it's too late";
  enum-based states or the State pattern "can sometimes be enough"; when state explosion does not apply, "a simpler solutions might
  be considered to be good enough" — [statecharts.dev: Why statecharts are not used](https://statecharts.dev/faq/why-statecharts-are-not-used.html)
- Flat FSMs multiply states and transitions with each independent aspect — [statecharts.dev: State explosion](https://statecharts.dev/state-machine-state-explosion.html)
- Practitioner criticism (anecdotal): a state machine for large-project UI produced "clusters of states which were interconnected
  in every possible way"; others replied that hierarchical states address this — [dev.to comment thread](https://dev.to/yuriykulikov/comment/hm9o)
- Persisted-state incompatibility: XState warns restored state "may be incompatible with the new logic" and recommends event
  sourcing as more robust — [Stately: Persistence](https://stately.ai/docs/persistence)
- Replay-based engines break on logic changes (non-deterministic orchestration, stuck Running instances) and need version stamping
  per instance — [Microsoft Learn: Durable Functions versioning](https://learn.microsoft.com/en-us/azure/azure-functions/durable/durable-functions-versioning)
- Event-sourced entities must read old events forever; event adapters translate — [Akka: Event Sourcing](https://doc.akka.io/libraries/akka-core/current/typed/persistence.html)
- Express workflows lose the ability to render old executions once the definition changes because definitions are not stored — [AWS: Viewing execution details](https://docs.aws.amazon.com/step-functions/latest/dg/concepts-view-execution-details.html)

### Inferences
- Remit is past the "few booleans" threshold for subscriptions: the brief lists six lifecycle values (active, paused, held,
  cooling down, due, ended) across at least three independent aspects, and routines add four more. That is the state-explosion
  condition statecharts.dev names; an explicit model is warranted.
- The cheapest robust form is not a statechart library but explicit Go types: one small enum per orthogonal aspect (control:
  active/paused/ended; health: ok/held(reason); window: idle/cooling(until)/due(at)), a pure transition function, and a closed
  effect type. This captures the parallel-region benefit without a runtime dependency.
- Versioning: Remit's lifecycle state is a snapshot (current row), not a replayed history, so it avoids the Durable Functions
  non-determinism trap; the remaining risk is an enum value or field meaning changing, handled by the existing rule of migrating
  stored records forward once in a numbered migration.

### Gaps
- No primary source found quantifying state-machine adoption costs or failure rates; criticism found is anecdotal.

## 8. Mapping each mechanism to Remit open problems (a)-(g)

### Takeaway
Every mechanism studied answers at least one open problem; the strongest transfers are SCXML/Step Functions error-as-event and
explicit terminal outcomes for (a), pure transition + effects-as-data for (b) dry run and testability, HA modes for (e) overlap,
Step Functions task timeouts for (f), and HA traces plus XState inspection for (g).

### Cited Findings
- (a) Silent fail/skip: SCXML turns guard errors into `error.execution` events (§5.9.1) — [W3C SCXML](https://www.w3.org/TR/scxml/); Step Functions fails the execution by default on unhandled error and names error classes — [AWS: Error handling](https://docs.aws.amazon.com/step-functions/latest/dg/concepts-error-handling.html); HA records "Stopped because a condition failed / on error / only a single execution is allowed" and keeps separate "Did not trigger" traces — [HA: Troubleshooting](https://www.home-assistant.io/docs/automation/troubleshooting/); [HA: Testing](https://www.home-assistant.io/docs/automation/testing/)
- (b) Start position, backfill, dry run: XState pure `transition` computes next state and actions without executing them — [Stately: Pure transitions](https://stately.ai/docs/pure-transitions); Akka recovery replays journaled events through the event handler — [Akka: Event Sourcing](https://doc.akka.io/libraries/akka-core/current/typed/persistence.html); Step Functions Redrive resumes unsuccessful executions — [AWS: Viewing execution details](https://docs.aws.amazon.com/step-functions/latest/dg/concepts-view-execution-details.html); HA edge-only triggers never fire for an already-true state — [HA: Triggers](https://www.home-assistant.io/docs/automation/trigger/)
- (c) Retention vs cursors: Step Functions keeps history 90 days and caps at 25,000 events per execution; Express keeps none — [AWS: Choosing workflow type](https://docs.aws.amazon.com/step-functions/latest/dg/choosing-workflow-type.html); [AWS: Viewing execution details](https://docs.aws.amazon.com/step-functions/latest/dg/concepts-view-execution-details.html); Akka must be able to read old events after evolution, with snapshots to shorten recovery — [Akka: Event Sourcing](https://doc.akka.io/libraries/akka-core/current/typed/persistence.html)
- (d) State vs stream: HA distinguishes entity state (with `for` duration on a held state, attribute-only changes) from fired events, and triggers are edge-only — [HA: State changed trigger](https://www.home-assistant.io/triggers/state/); [HA: Triggers](https://www.home-assistant.io/docs/automation/trigger/); Durable Entities expose a committed current state separately from the operation messages that change it — [Microsoft Learn: Durable entities](https://learn.microsoft.com/en-us/azure/azure-functions/durable/durable-functions-entities)
- (e) Time and overlap: SCXML delayed `<send>` + `<cancel>` — [W3C SCXML](https://www.w3.org/TR/scxml/); XState `after`, cancelled on state exit — [Stately: Delayed transitions](https://stately.ai/docs/delayed-transitions); Step Functions Wait (relative or absolute) — [AWS: Wait state](https://docs.aws.amazon.com/step-functions/latest/dg/state-wait.html); Durable Entities scheduled operations — [Microsoft Learn: Durable entities](https://learn.microsoft.com/en-us/azure/azure-functions/durable/durable-functions-entities); HA modes with `max`/`max_exceeded` — [HA: Automation modes](https://www.home-assistant.io/docs/automation/modes/)
- (f) Processor under global lock, no timeout: Step Functions TimeoutSeconds/HeartbeatSeconds and AWS's advice to always set them for long-running work — [AWS: Task state](https://docs.aws.amazon.com/step-functions/latest/dg/state-task.html); Durable Entities serialize per entity, not globally — [Microsoft Learn: Durable entities](https://learn.microsoft.com/en-us/azure/azure-functions/durable/durable-functions-entities); Akka runs side effects after persist — [Akka: Event Sourcing](https://doc.akka.io/libraries/akka-core/current/typed/persistence.html)
- (g) Agents create/debug subscriptions: XState inspection stream (`@xstate.event`, `@xstate.snapshot`, `@xstate.microstep`) — [Stately: Inspection](https://stately.ai/docs/inspection); HA trace contents (trigger, condition results, changed variables, config at run time, JSON download) and isolated condition/action test tools — [HA: Testing](https://www.home-assistant.io/docs/automation/testing/); Step Functions step details with input/output, retries and definition per execution — [AWS: Viewing execution details](https://docs.aws.amazon.com/step-functions/latest/dg/concepts-view-execution-details.html)

### Inferences
- (a) Make every evaluation end in a recorded outcome from a closed set (fired, skipped(reason), held(error), not_matched for
  near-misses), and move a subscription to `held(reason)` on an unhandled error. MEDIUM: inferred from SCXML/Step Functions/HA.
- (b) Implement start position as an explicit cursor choice at creation (from now, from seq N, from oldest retained, current value
  for state signals); dry run = the pure evaluator over that range returning effects unexecuted; backfill = same loop executed,
  idempotent via the (subscription, key, revision) firing key. MEDIUM.
- (c) Retention should compute its floor as min(SeenSeq) over active subscriptions (bounded by a max age, after which a lagging
  subscription is moved to `held(cursor_expired)` visibly rather than silently skipping). Precedent is indirect: the sources show
  bounded history (Step Functions 90 days / 25,000 events) and snapshots replacing history (Akka), not cursor-aware retention.
  LOW-MEDIUM; cursor-aware retention prior art (Kafka consumer groups, Postgres replication slots) is outside this note's scope.
- (d) Split the concept: a state signal has a current value per key (level; supports `for` durations and "evaluate current value
  on subscribe"); a stream signal has only events (edge). HA and Durable Entities both keep these apart. MEDIUM.
- (e) Put all time in the subscription/routine state as persisted `due_at` timers fired by the scheduler tick (Durable Entities
  scheduled operation, SCXML delayed send), and make overlap an explicit mode enum with `max` and a recorded overflow outcome
  (HA). Avoid HA's and XState's in-memory timers that are lost on restart. MEDIUM-HIGH.
- (f) Run processors outside the global mutex with a mandatory timeout, then commit their result under the lock in a short
  transaction; a timeout is a typed error feeding the (a) outcome path. Per-subscription serialization (Durable Entities) replaces
  global serialization as the long-term seam. MEDIUM.
- (g) Expose to agents: the lifecycle as data (current state per aspect and allowed transitions), a trace per evaluation (reading
  seq, guard result, transition, effects, revision), separate near-miss buffer, and a dry-run tool over a seq range. HIGH that the
  pattern exists in HA and Step Functions; MEDIUM that it fits Remit's tool surface.

### Gaps
- Not covered here and likely in other researchers' scope: cursor-aware log retention (Kafka, NATS JetStream, Postgres slots),
  CEL-specific error semantics, and workflow engines such as Temporal.
