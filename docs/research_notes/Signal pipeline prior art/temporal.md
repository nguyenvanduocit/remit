# Temporal: durable execution, signals, timers and schedules (prior art for the Remit signal pipeline)

Scope: Temporal's model as documented on docs.temporal.io and in the `temporalio/api` protos (fetched 2026-11-05), and what it
teaches for Remit's reading log + subscription pipeline. Facts carry a source; lessons for Remit are under
*Inferences* and are this researcher's reasoning, not Temporal's claims. Problem letters refer to the brief:
(a) silent fail/skip, (b) no start position/backfill/dry-run, (c) retention ignores cursors, (d) "signal" conflates state
and stream, (e) time split between clock sensors and subscription windows / implicit overlap policy, (f) plugin processor
under the global lock with no timeout, (g) AI agents create and debug subscriptions through tools.

## Event History as the source of truth; determinism, replay, and what a workflow waits on

### Takeaway
Every workflow has one append-only, durable Event History; workflow code never acts directly but emits Commands that the
service turns into persisted Events, so a crash anywhere is recovered by replaying the history. A wait (timer, activity,
message) is durable because the thing being waited on is itself an event the service records and later delivers.

### Cited Findings
- Event History is "a complete and durable log of everything that has happened in the lifecycle of a Workflow Execution." — [Temporal: Event History](https://docs.temporal.io/encyclopedia/event-history)
- Workflow code "sends a Command to the Temporal Service"; a Command is "a requested action issued by a Worker to the Temporal Service after a Workflow Task Execution completes"; the service acts on Commands (schedule an activity, start a timer) and the resulting Events are persisted. — [Event History](https://docs.temporal.io/encyclopedia/event-history)
- After a Worker crash the Worker "uses the Event History to replay the code and recreate the state of the Workflow Execution" and "resumes progress from the point of failure as if the failure never occurred." — [Event History](https://docs.temporal.io/encyclopedia/event-history)
- "A Workflow Definition must be deterministic to support replay." Workflow code must "make the same Workflow API calls in the same sequence, given the same input." "When the Workflow's code replays, the Commands that are emitted are compared with the existing Event History"; a mismatch fails the execution with a non-determinism error. — [Workflow Definition](https://docs.temporal.io/workflow-definition)
- Branching "based off a local time setting or a random number" is forbidden inline; SDK APIs supply time/random/unreliable data; "API calls, LLM/AI invocations, database queries" belong in Activities, outside the replay path. — [Workflow Definition](https://docs.temporal.io/workflow-definition)
- Long-lived executions need a versioning strategy (patching or the recommended Worker Versioning). — [Workflow Definition](https://docs.temporal.io/workflow-definition)
- Examples of what a workflow awaits: "Start and wait on a Timer/sleep", "Spawn and wait on an Activity Execution"; message handlers can block on a Wait Condition until the workflow reaches a state. — [Workflow Definition](https://docs.temporal.io/workflow-definition); [Handling messages](https://docs.temporal.io/handling-messages)
- Continue-As-New "allows you to checkpoint your Workflow's state and start a fresh Workflow"; the new run "has the same Workflow Id, but a different Run Id, and starts its own Event History." Use it when history grows toward the limits ("Temporal will tell your Workflow when it's approaching performance or scalability problems" via Continue-As-New Suggested) and periodically so long-lived workflows pick up new code. State moves forward only as the new run's arguments. — [Continue-As-New](https://docs.temporal.io/workflow-execution/continue-as-new)
- Waits are recorded as named events: `TimerStarted` ("The Workflow started a Timer, such as with a call to sleep"), `TimerFired` ("The Timer fired."), `TimerCanceled` ("The Workflow canceled the Timer before it fired"), `WorkflowExecutionSignaled` ("The Workflow Execution received a Signal"), `WorkflowExecutionUpdateAccepted` ("the Update passed its validator, if one is defined"), `ActivityTaskScheduled`. — [Events reference](https://docs.temporal.io/references/events)
- `ActivityTaskStarted` ("A Worker picked up the Activity Task") is not written when the worker starts; the service writes it only "until the Activity Execution closes, for example with ActivityTaskCompleted or ActivityTaskFailed," and on retries it reflects the last attempt. So retries happen without adding history events. — [Events reference](https://docs.temporal.io/references/events)
- History is bounded: 51,200 events or 50 MB per execution (warning at 10,240 events / 10 MB); 4 MB per history transaction; 2 MB per payload. — [Temporal Cloud limits](https://docs.temporal.io/cloud/limits)

### Inferences
- Remit's reading log with a global sequence number plays the role of Event History, but at the system level rather than per execution. The transferable part is the Command/Event split: a subscription's decision ("fire action X for key K at rev R") should be a recorded event, and the side effect (tell agent, start chain, run plugin) should be a separate step that consumes that record. Then a crash between decision and effect is recovered by re-reading the record, not by re-evaluating the CEL condition against a log that may have moved. Informs (a), (f).
- Determinism matters if Remit ever re-evaluates conditions (dry-run, backfill, debug "why did this not fire"): a CEL condition that reads wall-clock time or mutable state outside the reading cannot be replayed. Passing "now" in as an input (as Temporal's SDK time APIs do) keeps evaluation pure and replayable. Informs (b), (g). MEDIUM.
- Temporal's hard per-history caps show that an unbounded per-entity log needs an explicit compaction story (continue-as-new). For Remit, the analogous pressure is on the firing table and per-subscription state, not only readings. Informs (c). MEDIUM.

- Temporal keeps retry attempts out of the history (only the final attempt's started/closed events are written) and holds per-attempt state in mutable "pending activity" info instead. That is the split Remit needs too: the firing log records decisions and final outcomes, while attempt counters and next-retry times live in one mutable row per pending action. That keeps the log from filling up with retries. Informs (a), (c). MEDIUM.

### Gaps
- The Continue-As-New page does not say what carries over besides state passed as arguments (search attributes, memo); the search-attribute page says keys carry over on Continue-As-New. Signal draining before Continue-As-New is covered in the message-passing section.

## Signals vs Updates vs Queries: semantics, ordering, delivery, signal-with-start, buffering

### Takeaway
Temporal splits "talk to a running workflow" into three verbs: Query (read, no history, never blocks), Signal (async write,
fire-and-forget, accepted once the server persists it), Update (sync tracked write with optional validator that can reject
before anything is written). Messages arriving before a handler exists are buffered by the SDK; ordering across senders is
not something the docs promise.

### Cited Findings
- Queries "can read the current state of the Workflow but cannot block in doing so"; they "never add Event History entries" and can run on completed workflows. — [Workflow message passing](https://docs.temporal.io/encyclopedia/workflow-message-passing)
- Signals are "asynchronous write requests" that "cause changes in the running Workflow, but you cannot await any response or error"; use them when clients want to move on immediately and don't depend on Worker availability. — [Workflow message passing](https://docs.temporal.io/encyclopedia/workflow-message-passing)
- Updates are "synchronous, tracked write requests"; they "must wait for the Worker running the Workflow to acknowledge the request"; use them to validate before the Update is accepted into history. Pattern for async read-after-write: send a Signal, then poll with a Query. — [Workflow message passing](https://docs.temporal.io/encyclopedia/workflow-message-passing)
- A client's Signal call returns when the server accepts the Signal, not when the workflow receives it; Signals can be sent only to executions "that haven't closed." — [Go message passing](https://docs.temporal.io/develop/go/message-passing)
- Buffering: "the SDK buffers signals when there are no channels created for them"; "the SDK buffers messages when there are no registered handlers for them," and registering a handler immediately processes matching buffered Updates, which can reorder processing across message types. — [Go message passing](https://docs.temporal.io/develop/go/message-passing)
- Loss risk: before completing or Continue-As-New, drain the signal channel, otherwise "the Signals will be lost." — [Go message passing](https://docs.temporal.io/develop/go/message-passing)
- Order within one wake-up: "it will process messages in the order they were received, followed by making progress in the Workflow's main method." — [Handling messages](https://docs.temporal.io/handling-messages)
- No official cross-sender ordering guarantee found; community answers say confirmed-earlier signals from different senders can arrive later, and recommend one workflow per key with sequence numbers in payloads. — [Community: Functional requirement definition](https://community.temporal.io/t/functional-requirement-definition/12554); [Community: Ordering guarantee](https://community.temporal.io/t/ordering-guarantee-of-messages-sequencing/9206?page=2). (Forum, not official; LOW-MEDIUM.)
- Official ordering statement (Workflow Streams, Public Preview): "Ordering is guaranteed within a single publisher"; across publishers, the workflow's receipt order of Signals sets the interleaving; "if event A must precede event B, publish them from the same publisher." — [Workflow Streams](https://docs.temporal.io/workflow-streams)
- Workflow Streams are "a durable event channel hosted inside a Workflow". Publishers batch events into one Signal per flush (default 2 s); each subscriber poll is an Update that waits for events past the subscriber's own offset; a Query reports the head offset. Every entry has a "global, monotonically increasing offset" that never changes once written; subscribers keep their own offset and resume without coordination. Growth is bounded by truncation (drops entries below an offset) and Continue-As-New; "A subscriber behind the new base is advanced silently, so it may re-receive items." Aimed at tens of publishers/subscribers, not thousands. — [Workflow Streams](https://docs.temporal.io/workflow-streams)
- Signal-With-Start: if a workflow with that id is running it is signaled; otherwise a new one starts and is immediately signaled. Update-With-Start requires a Workflow Id Conflict Policy (recommended "Use Existing" with an idempotent handler). — [Go message passing](https://docs.temporal.io/develop/go/message-passing)
- Update validators run before the Update is written to history; a validator error rejects it and `WorkflowExecutionUpdateAccepted` is not recorded. — [Go message passing](https://docs.temporal.io/develop/go/message-passing)
- Failure semantics differ: a non-Application exception in a Signal handler causes a Workflow Task Failure, so "the Workflow gets stuck" and retries the handler until fixed; an Application Failure fails the whole workflow. For Updates, validator errors / Application Failures fail only the Update and "the Workflow will keep going." — [Handling messages](https://docs.temporal.io/handling-messages)
- Handlers should run before the main method is ready only after state is initialized ("Initialize your Workflow's state before handling messages"); a handler can block on an init flag. Finish all handlers before completion/continue-as-new (await AllHandlersFinished); by default the worker logs a warning if a run ends with unfinished handlers. — [Handling messages](https://docs.temporal.io/handling-messages); [Go message passing](https://docs.temporal.io/develop/go/message-passing)
- Limits: 10,000 Signals per execution, after which "no more Signals are processed"; 10 in-flight Updates and 2,000 total Updates per execution; 2,000 pending activities/signals/child workflows/cancels combined by type (500 recommended). — [Temporal Cloud limits](https://docs.temporal.io/cloud/limits)

### Inferences
- Temporal names state reads (Query) and stream writes (Signal) as different verbs with different storage (Query writes nothing; Signal is a history event). Remit's "signal" covers both a current value (the latest reading of a key) and an event stream (each reading). Splitting into "current state of key K" (read, like Query) and "readings of K after seq N" (stream, like Signal) would give each its own retention and semantics. Informs (d). MEDIUM.
- The Update validator is a concrete pattern for (g): an agent-facing "create/change subscription" tool should validate (compile CEL, resolve key pattern against known keys, check action target exists) and reject *before* anything is stored, returning the reason synchronously. Unvalidated writes that fail later are how silent failures (a) are born. HIGH that the pattern exists; MEDIUM on fit.
- Temporal's own docs call out two silent-loss paths (undrained signals at completion; Signal-handler panics leaving the workflow "stuck") and answer them with a default warning log and a "Task Failures" view (below). Remit's equivalent: a subscription whose action errors must enter a visible failed state, not just be skipped. Informs (a).
- Signal-With-Start / Update-With-Start + Conflict Policy is the direct analogue of "wake or start a work chain": one atomic call that either delivers to the running chain or starts one and delivers, with an explicit policy for the conflict. Remit's "one run at a time" routine is Conflict Policy = Use Existing (or Fail); making it an explicit enum rather than implicit behavior addresses (e).
- Buffering before handler registration shows a delivery target can be "not ready yet" without losing messages; for Remit, the subscription cursor (SeenSeq) already gives this for free if the action is not acknowledged until the target accepts it.

### Gaps
- Same-publisher order is documented only for Workflow Streams. For plain Signals the docs give only the per-wake-up order rule. Temporal's documentation or support would have to state anything stronger; a forum staff reply saying signals are "guaranteed to be delivered in the same order" ([forum](https://community.temporal.io/t/signalling-system-human-driven-workflows/160)) is not documentation.
- No official per-workflow signal rate guidance found on the handling-messages page.

## Durable timers and the timer + signal race

### Takeaway
A timer is a server-side persisted record, not a sleeping thread: workers hold nothing while waiting, a worker or service
outage only delays the firing until both are back, and timers are cancellable via a cancellable context. Racing a timer
against a signal is done in workflow code (selector / await-with-timeout), and both outcomes are history events, so the
race is replayable.

### Cited Findings
- Timers are persisted; if the Worker or Temporal Service is down when the period ends, the call "will resolve, causing execution to proceed" once they are available again. — [Timers and delays](https://docs.temporal.io/workflow-execution/timers-delays)
- "Workers consume no additional resources while waiting for a Timer to fire," and "a single Worker can await millions of Timers concurrently." — [Timers and delays](https://docs.temporal.io/workflow-execution/timers-delays)
- Duration: one second to several years; workflows "should not rely on sub-second accuracy for Timers"; treat the duration as a minimum. — [Timers and delays](https://docs.temporal.io/workflow-execution/timers-delays)
- Go: `timerCtx, cancelTimer := workflow.WithCancel(ctx); timer := workflow.NewTimer(timerCtx, duration)` — a cancellable context "so the Workflow can cancel the Timer early." `workflow.Sleep` with zero/negative duration returns immediately. — [Go timers](https://docs.temporal.io/develop/go/timers)
- Timer vs other events race (Go): a Selector combines `selector.AddFuture(workflow.NewTimer(childCtx, threshold), ...)` with `selector.AddReceive(workflow.GetSignalChannel(ctx, name), func(c workflow.ReceiveChannel, more bool) { c.Receive(ctx, &signalVal) ... })` and `selector.Select(ctx)`. "Merely matching on the channel doesn't consume the message; it has to be explicitly consumed." Selectors "keep the Workflow Definition deterministic"; a native Go `select` cannot be used because it chooses randomly among ready cases. (The page races a timer against an Activity and separately receives a Signal; the timer+signal combination uses the same `Select`.) — [Go selectors](https://docs.temporal.io/develop/go/selectors)
- Condition-with-timeout (Go SDK): `func AwaitWithTimeout(ctx Context, timeout time.Duration, condition func() bool) (ok bool, err error)` "blocks the calling thread until condition() returns true or blocking time exceeds the passed timeout value. Returns ok=false if timed out"; `Await` doc: "Do not mutate values or trigger side effects inside condition." — [pkg.go.dev: go.temporal.io/sdk/workflow](https://pkg.go.dev/go.temporal.io/sdk/workflow#AwaitWithTimeout)
- Start Delay postpones a workflow's first execution; Signal-With-Start or Update-With-Start during the delay ends it early ("the remaining delay is bypassed"), but a plain Signal "does not interrupt the delay"; incompatible with Schedules and Cron Jobs. — [Timers and delays](https://docs.temporal.io/workflow-execution/timers-delays)

### Inferences
- Remit's cooldown/deadline windows on a subscription are timers in Temporal's sense. Today time lives in two places (clock sensors emitting readings, and subscription windows). Temporal keeps exactly one durable timer primitive and composes everything else (schedule, deadline, cooldown, start delay) from it. A single "timer due at T for owner X" record, fired by the scheduler tick, would let both routines and subscription deadlines share one mechanism and one view. Informs (e). MEDIUM.
- "Deadline OR reading, whichever first" is the timer+signal race. In Temporal both branches resolve into recorded events and the losing timer is cancelled. Remit should record which branch won (reading at seq N vs deadline at T) in the firing, so "why did this fire / not fire" is answerable. Informs (a), (g).
- "Treat the duration as a minimum" and "fires after outage" mean timers never fire early and never silently vanish; a missed fire during downtime fires late. Whether late is acceptable is a separate policy (the Schedule catchup window, below).

- The Selector's deterministic choice among ready cases is the rule Remit needs when a reading and a deadline become due in the same tick: a fixed precedence (e.g. the reading at seq ≤ the tick wins over the deadline), not whichever goroutine gets there first. Informs (e). MEDIUM.

### Gaps
- No official doc page shows a timer and a signal combined in one Selector; the combination is assembled from the two cited snippets.
- Internal storage of timers (timer queue in the history service) was not documented on the fetched pages.

## Schedules: spec, overlap, catchup, pause, backfill, trigger, jitter; why they replaced cron workflows

### Takeaway
A Schedule is its own object with its own id, separate from the workflows it starts. Its spec is a union of
intervals/calendars with bounds, exclusions, jitter and timezone; its policies make overlap, lateness and failure explicit
enums; and its describe output exposes counters for every way an action can be skipped. This is exactly the set of knobs
Remit's routines keep implicit.

### Cited Findings
- A Schedule "contains instructions for starting a Workflow Execution at specific times"; it has its own identity and is independent of any workflow execution, unlike a Temporal Cron Job, where the cron schedule is a property of the workflow execution. For a one-time future start, use Start Delay instead. — [Schedule](https://docs.temporal.io/schedule)
- Action: workflow type, task queue, args, timeouts. Scheduled runs get the action timestamp appended to the workflow id plus `TemporalScheduledStartTime` and `TemporalScheduledById` search attributes. `keep_original_workflow_id` suppresses the timestamp suffix. — [Schedule](https://docs.temporal.io/schedule); [schedule/v1/message.proto](https://raw.githubusercontent.com/temporalio/api/master/temporal/api/schedule/v1/message.proto)
- Spec: interval (epoch-aligned, with phase offset), calendar (cron strings of 5/6/7 fields or structured fields with `comment`), multiple specs combined, start/end time bounds, exclusion calendars, jitter ("a random value from 0 to this amount of jitter. Default: 0", bounded by the time to next action), timezone (UTC recommended). Actions generally start within about one second of the spec time. — [Schedule](https://docs.temporal.io/schedule); [message.proto](https://raw.githubusercontent.com/temporalio/api/master/temporal/api/schedule/v1/message.proto)
- Overlap policies when an action is due while a previous run is still open: **Skip** (default, "Nothing happens; the Workflow Execution is not started"), **BufferOne** (start once the current completes; buffer of one, extras dropped), **BufferAll** (sequential, subject to catchup window and buffer limits), **CancelOther** (cancel running, start new after cancellation completes), **TerminateOther** (terminate without cooperation, start new), **AllowAll** (only policy allowing concurrent runs). — [Schedule](https://docs.temporal.io/schedule)
- A paused workflow (pre-release feature) still counts as running for overlap: under Skip new actions are skipped while it stays paused. — [Schedule](https://docs.temporal.io/schedule)
- Catchup window: "The default is one year, meaning Actions will be taken unless over one year late"; minimum 10 s; measured from the jitter-adjusted time; manual triggers and backfills are not restricted by it. — [Schedule](https://docs.temporal.io/schedule)
- `pause_on_failure`: if a run fails or times out (not cancel/terminate) the schedule pauses itself ("turn on paused"), applying after the full retry chain. — [Schedule](https://docs.temporal.io/schedule); [message.proto](https://raw.githubusercontent.com/temporalio/api/master/temporal/api/schedule/v1/message.proto)
- Pause: "When a Schedule is Paused, the Spec has no effect"; manual trigger still works; `notes` is an "Informative human-readable message with contextual notes, e.g. the reason a schedule is paused"; pausing does not affect already started executions. — [Schedule](https://docs.temporal.io/schedule); [message.proto](https://raw.githubusercontent.com/temporalio/api/master/temporal/api/schedule/v1/message.proto)
- Backfill runs every action that would have occurred in a range, "taken now"; range "exclusive on start_time and inclusive on end_time"; per-request overlap override (AllowAll = parallel, BufferAll = sequential); useful for gaps from a pause or before the schedule existed. — [Schedule](https://docs.temporal.io/schedule); [message.proto](https://raw.githubusercontent.com/temporalio/api/master/temporal/api/schedule/v1/message.proto)
- TriggerImmediately: per-request overlap override; `scheduled_time` is the "Timestamp used for the identity of the target workflow," default now. — [message.proto](https://raw.githubusercontent.com/temporalio/api/master/temporal/api/schedule/v1/message.proto)
- Limited actions: `limited_actions` decrements `remaining_actions` per action; at zero it "will act as if it were paused"; manual triggers and overlap-skipped actions don't count. — [Schedule](https://docs.temporal.io/schedule); [message.proto](https://raw.githubusercontent.com/temporalio/api/master/temporal/api/schedule/v1/message.proto)
- ScheduleInfo (describe output): `action_count`, `missed_catchup_window` ("Number of times a scheduled action was skipped due to missing the catchup window"), `overlap_skipped`, `buffer_dropped`, `buffer_size`, `running_workflows`, `recent_actions` ("Most recent ten actual action times (including manual triggers)"), `future_action_times` ("Next ten scheduled action times"). Each `ScheduleActionResult` carries `schedule_time` (spec time incl. jitter), `actual_time`, and an eventually consistent `start_workflow_status`. — [message.proto](https://raw.githubusercontent.com/temporalio/api/master/temporal/api/schedule/v1/message.proto)
- Runs can read the last successful run's result and the last failure (last failure reset by a success). — [Schedule](https://docs.temporal.io/schedule)
- Schedules are implemented internally as a workflow (hidden from normal Elasticsearch views); List/Count are eventually consistent and use the Visibility rate limit. — [Schedule](https://docs.temporal.io/schedule)
- Scheduler v2 (server PR #10503, merged 2026-06-05): v1 "runs as a single workflow"; v2 "splits work across independent CHASM tasks": a generator task buffers actions, and an invoker process-buffer task resolves and executes them. The scheduler learns about completions through Nexus callbacks rather than polling. In v2, "Actions that survive overlap policy evaluation are checked against the catchup window a second time" (new `buffer_expired` reason: "buffered in time but expired before execution"). v2 resolves the overlap policy before buffering and stores it on each `BufferedStart`, so a later `UpdateSchedule` does not change already-buffered actions, whereas v1 applies the new policy to them. The PR adds a `schedule_overlap_skipped` metric tagged by policy. — [temporalio/temporal#10503](https://github.com/temporalio/temporal/pull/10503)
- Community report: missed actions inside the catchup window ran only when the overlap policy allowed it; under default Skip with a still-open run, nothing progresses. — [Community: Simple scheduling in Java SDK](https://community.temporal.io/t/simple-scheduling-in-java-sdk/8692) (forum, via search summary; LOW-MEDIUM)
- Cloud limits: 10 schedule requests/s per namespace; use jitter in specs to spread launches. — [Temporal Cloud limits](https://docs.temporal.io/cloud/limits)
- Why Schedules over cron: Temporal's blog says traditional cron lacks "job pausing and detailed monitoring for long-running tasks," and that Schedules support "start, backfill, delete, describe, list, pause, trigger, and update," each with "a unique id" and managed independently. — [Temporal blog: Schedules vs cron](https://temporal.io/blog/temporal-schedules-reliable-scalable-and-more-flexible-than-cron-jobs). Migration advice: "wrapping your current code in an Activity, which is then placed within a single-activity Workflow"; "It's essential to ensure the activity is idempotent or to disable the retry policy to prevent any potential issues"; Schedules are "similar to Kubernetes Cron but far more powerful", with "configuring overlap policies to enabling pause-on-failure" as the management features. — [Temporal blog: converting job scheduling](https://temporal.io/blog/how-to-convert-your-job-scheduling-system-to-temporal-schedules)

### Inferences
- The structural reason Schedules replaced cron workflows (stated in the docs as "a separate object" vs "a property of the Workflow Execution") is that a schedule attached to a run cannot be paused, edited, described or backfilled independently of that run. Remit's routine = clock sensor + subscription is closer to Temporal Schedules (separate object) but splits the one concept across two records, which is where (e) comes from. A routine as one entity holding spec + overlap policy + catchup window + pause/notes + counters is the Schedule shape. Informs (e).
- Overlap policy should be an explicit enum on the routine/subscription, with Skip as default to match today's "one run at a time"; BufferOne is the likely second need ("run again once after the current finishes"). Every Skip must increment a visible counter (`overlap_skipped`), never vanish. Informs (a), (e).
- `missed_catchup_window`, `overlap_skipped`, `buffer_dropped` are the direct answer to (a): every reason an action did not happen is a named counter on the object, alongside `recent_actions` with both scheduled and actual time. A Remit subscription should record, per evaluation that did not fire, *why* (condition false, cooldown, overlap, outside window, action error), at least as counters plus the last N outcomes.
- Backfill with an explicit half-open range and per-request overlap override, and `future_action_times` (next ten), are the shape for (b): a start position / backfill on a subscription ("evaluate from seq N" or "from time T") and a dry-run preview ("these 10 readings would fire"). Temporal's `scheduled_time` as workflow identity shows how backfilled firings stay idempotent: the identity is the *scheduled* time/key, not wall clock. For Remit, (subscription, key, revision) already plays that role.
- `pause_on_failure` + `notes` is a ready-made pattern for (a) and (g): a subscription whose action fails N times pauses itself with a machine-written note, which an agent can read through a tool and resume.
- Catchup window gives "how late may a missed tick still fire" a name and a default; Remit's clock sensor needs the same after a server restart, or downtime ticks are either silently lost or all fired at once. Informs (e).
- Temporal's own v1→v2 rewrite (#10503) points to two design choices. First, generating due actions and executing them are separate tasks, so a slow start does not block generation; this is (f) again. Second, the policy is resolved and frozen on each buffered action when it is generated, so editing a routine does not retroactively change work already queued. For Remit: a firing record should carry the policy and parameters it was decided under, which also makes "why did this fire" answerable after the subscription is edited (g). The added `buffer_expired` reason and per-policy skip metric show Temporal still finding unnamed skip paths in 2026, so (a) needs every drop path enumerated, not just the obvious ones. MEDIUM.

### Gaps
- A standalone schedules design proposal was not found: `github.com/temporalio/proposals/.../schedules.md` returned 404 and search found none. The v2 PR above is the best primary design source captured.
- The blog did not explicitly say Cron Workflows are deprecated; "replaced" is the framing of the docs (separate object vs property) and blog, not a fetched deprecation notice.
- Internal buffer limits for BufferAll were not stated numerically.

## Activities: timeouts, retry policies, non-retryable errors; how a hung external call does not block everything

### Takeaway
All I/O runs in Activities, executed by workers outside the coordinator, each attempt bounded by Start-To-Close (required in
effect) and optionally Heartbeat; the server never waits on a worker, it waits on a durable timer for the timeout. Retries
are on by default with exponential backoff; non-retryable errors are typed.

### Cited Findings
- Start-To-Close: max time for a single attempt; "no default value of its own"; if neither it nor Schedule-To-Close is set "the Temporal Service rejects the Activity"; strongly recommended to always set. On timeout another attempt is scheduled if the policy allows. — [Detecting activity failures](https://docs.temporal.io/encyclopedia/detecting-activity-failures). That page also says an ActivityTaskTimedOut event is recorded on each timeout; the [Events reference](https://docs.temporal.io/references/events) contradicts this, saying the event means the service "won't retry it". The events reference is the more specific source.
- Schedule-To-Close: total across retries, default ∞; recommended way to bound total retry duration (rather than Maximum Attempts). — [Detecting activity failures](https://docs.temporal.io/encyclopedia/detecting-activity-failures); [Retry policies](https://docs.temporal.io/encyclopedia/retry-policies)
- Schedule-To-Start: queued-to-picked-up, default ∞, "non-retryable by design" (a retry would land in the same queue); monitoring `temporal_activity_schedule_to_start_latency` is recommended instead. — [Detecting activity failures](https://docs.temporal.io/encyclopedia/detecting-activity-failures)
- Heartbeat timeout: "the maximum time between Activity Heartbeats"; 0 = disabled; capped at Start-To-Close; heartbeats throttled (default 30 s, max 60 s). "The Temporal Server doesn't detect when a Worker loses communication or crashes. It relies on the Start-To-Close Timeout to force retries"; a short Heartbeat Timeout detects a crashed or silently failed worker sooner. — [Detecting activity failures](https://docs.temporal.io/encyclopedia/detecting-activity-failures)
- Retry defaults: initial 1 s, backoff 2.0, max interval 100 x initial, max attempts unlimited, no non-retryable types by default; interval = min(initial x coeff^n, max interval). Activities retry by default; "Unlike Activities, Workflow Executions do not retry by default." — [Retry policies](https://docs.temporal.io/encyclopedia/retry-policies)
- Non-retryable: an `ApplicationError` with `non_retryable` is never retried regardless of policy, or match by error `type` in the policy; use sparingly (e.g. bad input). A failure can set a next-retry delay that overrides the policy interval, still capped by attempts and timeouts. — [Retry policies](https://docs.temporal.io/encyclopedia/retry-policies)
- `ActivityTaskTimedOut` per the events reference: "The Activity Execution hit an Activity timeout, and the Temporal Service won't retry it." Read with the timeouts page, a timed-out attempt that will be retried is not written as a history event; the event appears only when retries end. — [Events reference](https://docs.temporal.io/references/events); [Detecting activity failures](https://docs.temporal.io/encyclopedia/detecting-activity-failures)
- Workflow-level timeouts: Execution Timeout covers the whole execution "including retries and any usage of Continue As New", default ∞; Run Timeout covers a single run, default = Execution Timeout; Workflow Task Timeout is the time a worker has to process one workflow task, default 10 s, max 120 s, "not recommended to move beyond the default value"; its purpose is detecting a downed worker. The docs "generally do not recommend setting Workflow Timeouts"; use a Timer for time-based actions. A failed Workflow Task does not fail the workflow; affected workflows are found via the `TemporalReportedProblems` search attribute or the Task Failures view. — [Detecting workflow failures](https://docs.temporal.io/encyclopedia/detecting-workflow-failures)
- Pending-activity state exposed per activity: `attempt`, `maximum_attempts`, `last_failure`, `last_heartbeat_time`, `last_started_time`, `last_worker_identity`, `current_retry_interval` ("The time activity will wait until the next retry"), `next_attempt_schedule_time`, `last_attempt_complete_time`, `paused` and `pause_info` (paused manually or "by the rule"). — [workflow/v1/message.proto](https://raw.githubusercontent.com/temporalio/api/master/temporal/api/workflow/v1/message.proto)

### Inferences
- The key move for (f): Temporal never runs user I/O in the coordinator. The coordinator records "activity scheduled" and a timeout timer; the worker runs it elsewhere; completion or timeout is a new event. Remit running a plugin processor under the global write mutex inverts this. The Temporal-shaped fix: under the lock, only record "processor job scheduled for (subscription, key, rev)" and release; run the plugin outside the lock with a start-to-close deadline (context timeout); take the lock again only to record the result. A hung plugin then costs one job slot, not the whole scheduler. HIGH that this is Temporal's design; MEDIUM on direct fit.
- Required timeout by default (Temporal rejects an activity with neither timeout) argues for Remit making a processor timeout mandatory with a default, so an unbounded call is unrepresentable. Informs (f).
- Retry + typed non-retryable + `last_failure` + `next_attempt_schedule_time` gives (a) a vocabulary: a firing's action is "retrying (attempt 3, next at T, last error E)" or "failed permanently (non-retryable: bad input)", never silently gone. Activity pause "by the rule" is the analogue of a subscription auto-pausing after repeated failure.

- Temporal bounds each unit of execution with a short default (10 s workflow task, required activity start-to-close), but advises against a timeout on the whole long-lived workflow. The analogue for Remit: bound every processor/action attempt, but do not put a deadline on a subscription's lifetime. Expiry belongs to an explicit timer or deadline window. Informs (e), (f). MEDIUM.

### Gaps
- Activity pause/reset API details (newer feature) were only seen as proto fields, not documented semantics.

## Task queues and workers: decoupling work from the coordinator

### Takeaway
Work is handed off through persisted task queues that workers pull from when they have capacity; the service never pushes
into a worker or waits synchronously on one. This is the opposite of a single global lock around both decisions and
execution.

### Cited Findings
- A task queue is a "lightweight, dynamically allocated queue that one or more Worker Entities poll for Tasks"; "Workers poll for Tasks in Task Queues via synchronous RPC." — [Task Queue](https://docs.temporal.io/task-queue)
- Pull model gives flow control: a worker asks only when it has spare capacity. Workflow and Activity tasks persist in the queue if a worker is down; Query and Nexus tasks are sync-matched only. — [Task Queue](https://docs.temporal.io/task-queue)
- 4 partitions by default; one partition is almost FIFO but limits throughput. Task queues allow routing to specific worker processes. Activity task queues support server-side dispatch rate limiting. — [Task Queue](https://docs.temporal.io/task-queue)

### Inferences
- For Remit (single SQLite, single process), the useful lesson is not partitions but the separation: decisions (append reading, evaluate subscriptions, record firing) are short serialized writes; effects (start chain, run job, run plugin) are queued records consumed by workers outside the lock with bounded concurrency. Each effect kind can have its own queue/concurrency limit, so a slow plugin queue does not delay "tell an agent". Informs (f). MEDIUM.
- Persisted tasks mean a restart resumes pending effects; an in-memory goroutine fan-out would not. Informs (a).

### Gaps
- Sticky execution, worker-side concurrency limits and exact rate-limit options were not on the fetched page.

## Visibility, search attributes, Web UI: seeing why something is stuck

### Takeaway
Temporal makes "what is it waiting on" a first-class view: a live call stack showing each await point, pending activities
with attempt/last failure/next retry, an auto-flag for workflows whose tasks keep failing, schedules with recent and
upcoming runs, and human-readable notes/summaries.

### Cited Findings
- Call Stack tab shows the `__stack_trace` Query result; "The call stack shows each location where Workflow code is waiting"; works only if a worker is running. — [Web UI](https://docs.temporal.io/web-ui)
- Pending Activities tab: "Displays a summary of recently active and/or pending Activity Executions"; per-activity fields include attempt, last_failure, next_attempt_schedule_time, last_worker_identity. — [Web UI](https://docs.temporal.io/web-ui); [workflow/v1/message.proto](https://raw.githubusercontent.com/temporalio/api/master/temporal/api/workflow/v1/message.proto)
- Workers tab lists workers polling the workflow's task queue; "If no Workers are polling, an error displays." — [Web UI](https://docs.temporal.io/web-ui)
- User Metadata: static Summary/Details and dynamic "Current Details", plus events with user metadata as "a readable log of what the Workflow is doing." — [Web UI](https://docs.temporal.io/web-ui)
- Task Failures saved view: workflows flagged after five consecutive workflow task failures or timeouts (cleared on next success); configurable via `system.numConsecutiveWorkflowTaskProblemsToTriggerSearchAttribute`. Such a workflow "stays Running without making progress." — [Web UI](https://docs.temporal.io/web-ui)
- Workflow list filters by status, id, type, times and any search attribute; Schedules page shows frequency, bounds, and recent and upcoming runs. — [Web UI](https://docs.temporal.io/web-ui)
- Search attributes are indexed key-value metadata in the Visibility store, used in List Filters. Defaults are reserved and read-only (`ExecutionStatus`, `WorkflowType`, `StartTime`, `CloseTime`, `TaskQueue`, `WorkflowId`, `TemporalScheduledById` = "The ID of the Schedule that started the Workflow"). Custom types: Bool, Datetime, Double, Int, Keyword, KeywordList, Text, set at start or updated from workflow code. Example filter: `WorkflowType = '...' and ExecutionStatus != 'Running' and (StartTime > '...' or CloseTime < '...')`. — [Search attributes](https://docs.temporal.io/search-attribute)
- Custom attribute limits per namespace on SQL stores (incl. SQLite): 3 each of Bool/Datetime/Double/Int, 10 Keyword, 3 each KeywordList/Text (Cloud: 20/40/5); value up to 2 KB, 40 KB total. "Do not use sensitive data or PII in Search Attributes" (stored unencrypted). — [Search attributes](https://docs.temporal.io/search-attribute)
- Visibility lags state: "You might experience lag due to time passing between the Workflow's state change and the Activity updating the Visibility store"; for low-latency logic, use a Query or Activities instead. — [Search attributes](https://docs.temporal.io/search-attribute)
- List Filter grammar: `=`, `!=`, `>`, `>=`, `<`, `<=`, `AND`, `OR`, parentheses, `BETWEEN ... AND`, `IN (...)`, `STARTS_WITH` (Keyword only), `IS NULL`/`IS NOT NULL`, `ORDER BY` ("currently not supported in Temporal Cloud"); scoped to one namespace; use `CountWorkflow` for large result sets. — [List Filter](https://docs.temporal.io/list-filter)
- Stuck detection without failing the workflow: the `TemporalReportedProblems` search attribute marks workflows with repeated workflow-task failures. — [Detecting workflow failures](https://docs.temporal.io/encyclopedia/detecting-workflow-failures)
- Workflow pause info records `identity` ("The identity of the client who paused"), `paused_time`, `reason`. — [workflow/v1/message.proto](https://raw.githubusercontent.com/temporalio/api/master/temporal/api/workflow/v1/message.proto)

### Inferences
- Answer to (a) and (g) in one: each subscription should expose a "what am I waiting on" view, analogous to the call stack: cursor seq vs log head, condition last result and why, window/cooldown state with next due time, last firing, last action failure with attempt and next retry, paused + reason + who. The same data served to an agent through a describe tool lets it debug a subscription without the UI.
- The Task Failures flag (N consecutive failures → searchable attribute) is a cheap, deterministic way to surface repeated silent errors; Remit can derive "needs attention" from consecutive failed firings and show it on the subscription's own screen and the owner's waiting count. Informs (a).
- Pause records carry who and why; matches AGENTS.md's rule that pulling a rail is recorded with who and why.
- Temporal lists executions through a separate, lagging, typed index (search attributes), distinct from the authoritative history. Remit on one SQLite file does not need a second store. The lesson it does need is a small set of typed, filterable attributes on subscriptions and firings (status, origin schedule/subscription id, problem flag), so an agent's list tool can answer "which subscriptions are failing / paused / behind" with one filter. `TemporalScheduledById` is the provenance link Remit needs on a chain started by a routine or subscription. Informs (a), (g). MEDIUM.

### Gaps
- Per-SDK upsert API names were not captured; they are SDK detail and do not bear on Remit's design.

## Idempotency: workflow id reuse/conflict policies, request ids

### Takeaway
Temporal enforces at most one open execution per workflow id, and makes both "id is open" and "id existed and closed"
explicit policies; client calls carry request ids so retries do not act twice; Update ids are deduplicated server-side,
Signals are not.

### Cited Findings
- "Temporal guarantees that only one Workflow Execution with a given Workflow Id can be in an Open state at any given time," per namespace. — [Workflow Id and Run Id](https://docs.temporal.io/workflow-execution/workflowid-runid)
- Conflict Policy (id currently open): Fail (default, "Workflow execution already started"), Use Existing (returns the open run's id), Terminate Existing. — [Workflow Id and Run Id](https://docs.temporal.io/workflow-execution/workflowid-runid)
- Reuse Policy (id held by closed run): Allow Duplicate (default), Allow Duplicate Failed Only, Reject Duplicate, Terminate If Running. Reuse checks only see closed runs still within retention (default 30 days; Cloud range 1–90 days). — [Workflow Id and Run Id](https://docs.temporal.io/workflow-execution/workflowid-runid); [Temporal Cloud limits](https://docs.temporal.io/cloud/limits)
- "both Signals and Updates automatically use request IDs to deduplicate retried client calls"; Updates are deduplicated by Update ID, per run; for Signals use "a custom idempotency key that you send as part of your own signal inputs." — [Handling messages](https://docs.temporal.io/handling-messages)
- Proto: `StartWorkflowExecutionRequest.request_id` is "A unique identifier for this start request. Typically UUIDv4"; on `SignalWorkflowExecutionRequest` it is "Used to de-dupe sent signals", on Signal-With-Start "Used to de-dupe signal w/ start requests". Reuse policy default `WORKFLOW_ID_REUSE_POLICY_ALLOW_DUPLICATE` (closed runs); conflict policy default `WORKFLOW_ID_CONFLICT_POLICY_FAIL` (running runs). — [workflowservice/v1/request_response.proto](https://raw.githubusercontent.com/temporalio/api/master/temporal/api/workflowservice/v1/request_response.proto)
- Scheduled runs get the scheduled timestamp appended to the workflow id, so each schedule slot is its own id. — [Schedule](https://docs.temporal.io/schedule)

### Inferences
- Remit's firing key (subscription, key, revision) is the analogue of a deterministic workflow id; "routine runs one at a time" is Conflict Policy = Fail/Use Existing. Making Conflict and Reuse policies explicit fields of the action (start chain) clarifies (e): e.g. "if a chain for this key is open, deliver to it (Use Existing); if one finished, start a new one (Allow Duplicate)".
- Workflow Streams is the closest Temporal analogue to Remit's reading log: a global offset (like the global sequence), subscribers each keeping a cursor (like SeenSeq), a Query for the head (cursor lag = head − cursor), and Update-based long-polls past the cursor. Its truncation rule, a lagging subscriber "advanced silently", is exactly failure (c) combined with (a), now documented as accepted behavior in a preview feature. Remit should do the opposite: never advance a cursor silently; bound truncation by live cursors, or mark the subscription as having skipped from seq X to Y and show it. HIGH (fact) / MEDIUM (fit).
- Temporal's retention caveat (reuse checks only see retained runs) is the same hazard as (c): if Remit prunes firings or readings older than a subscriber's cursor, idempotency and replay break silently. Retention must be bounded below by min(SeenSeq) of live subscriptions, or a subscription whose cursor falls behind retention must be marked broken visibly rather than skipping ahead. Temporal's model does not solve this for you (it simply forgets after retention), so this is a lesson by contrast. MEDIUM.

### Gaps
- No source found on how long the server remembers a request_id for dedup (the proto comments do not say). Only Temporal's server source or support can answer it. It does not change the lesson: Remit's dedup key (subscription, key, revision) is stored, not time-windowed.

## For each mechanism: which open problem (a)-(g) it informs, and how

### Takeaway
The strongest transfers are: explicit skip counters and pause-on-failure from Schedules for (a); backfill ranges, scheduled
identity and next-N preview for (b); Activity-style off-lock execution with mandatory timeouts for (f); one timer primitive
plus explicit overlap/conflict enums for (e); Query vs Signal split for (d); Update validators and a describe/"what am I
waiting on" view for (g). Retention (c) is a lesson by contrast.

### Cited Findings
- Schedule describe counters (`missed_catchup_window`, `overlap_skipped`, `buffer_dropped`) and `recent_actions`/`future_action_times` — [schedule/v1/message.proto](https://raw.githubusercontent.com/temporalio/api/master/temporal/api/schedule/v1/message.proto)
- Backfill and trigger with per-request overlap override and `scheduled_time` identity — [message.proto](https://raw.githubusercontent.com/temporalio/api/master/temporal/api/schedule/v1/message.proto)
- Start-To-Close required in effect; server relies on it to detect crashed workers — [Detecting activity failures](https://docs.temporal.io/encyclopedia/detecting-activity-failures)
- Overlap policies and catchup window — [Schedule](https://docs.temporal.io/schedule)
- Query writes no history; Signal is an async history write — [Workflow message passing](https://docs.temporal.io/encyclopedia/workflow-message-passing)
- Update validator rejects before history — [Go message passing](https://docs.temporal.io/develop/go/message-passing)
- Reuse checks bounded by retention — [Workflow Id and Run Id](https://docs.temporal.io/workflow-execution/workflowid-runid)

### Inferences
| Problem | Temporal mechanism | Lesson for Remit | Confidence |
|---|---|---|---|
| (a) silent fail/skip | Schedule counters; `pause_on_failure` + `notes`; pending-activity `last_failure`/`attempt`/`next_attempt_schedule_time`; Task Failures flag after 5 consecutive failures; worker warning on unfinished handlers | Every non-firing has a named reason recorded (counter + last N outcomes); repeated action failure auto-pauses the subscription with a written note and raises "needs attention" | HIGH (mechanism) / MEDIUM (fit) |
| (b) start position, backfill, dry-run | Backfill (half-open range, overlap override, runs "taken now"); `future_action_times`; deterministic replay against history | Subscription created with an explicit start (head, seq N, time T); backfill = evaluate a past range with idempotent keys; dry-run = evaluate condition purely over a range and return would-fire list, writing nothing (Query-like) | MEDIUM |
| (c) retention vs cursors | Retention 1–90 days; reuse checks only see retained runs; history caps force continue-as-new; Workflow Streams truncation advances lagging subscribers silently | Temporal forgets after retention; Remit must instead bound pruning by live cursors or mark a lagging subscription broken visibly | MEDIUM |
| (d) state vs stream | Query (read state, no history) vs Signal (stream event in history) vs Update (validated write) | Separate "latest value of key" from "readings after seq"; different retention and semantics; conditions over state vs over events named differently | MEDIUM |
| (e) time split, implicit overlap | Schedule as one object (spec + overlap + catchup + jitter + pause); one durable timer primitive; Start Delay; Conflict/Reuse policies | Routine as one entity, not sensor + subscription; overlap policy an enum (Skip default, BufferOne, AllowAll...); catchup window for downtime; cooldown/deadline built on the same timer record | MEDIUM |
| (f) plugin under global lock, no timeout | Activities run on workers via task queues; Start-To-Close mandatory; heartbeat; retries with typed non-retryable | Lock only to record "scheduled" and "result"; run plugin outside the lock with a mandatory deadline; retries and permanent failure recorded | HIGH (mechanism) / MEDIUM (fit) |
| (g) agents create/debug via tools | Update validators reject before write; Describe Schedule; call stack "where it is waiting"; pause records who/why; notes | Create/change tools validate synchronously and return the reason; a describe tool returns the "waiting on" state and recent outcomes; pause/resume tools carry reason and actor | MEDIUM |

### Gaps
- No Temporal mechanism matches content-based CEL conditions or key patterns (Workflow Streams filters only by topic). Those lessons belong to the researchers covering event-matching systems; (d) and the key-pattern matching parts of the pipeline have no direct Temporal analogue, so lessons there are by analogy only.
- No source checked on how Temporal Nexus or newer "Workflow pause" (pre-release) behave in detail.
