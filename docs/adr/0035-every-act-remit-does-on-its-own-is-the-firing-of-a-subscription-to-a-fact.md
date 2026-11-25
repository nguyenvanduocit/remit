# 0035. Every act Remit does on its own is the firing of a subscription to a fact it recorded

Status: accepted, 2026-11-02; partly superseded by ADR-0049: a subscription's cursor is `SeenSeq` over one log, not `SeenRev` of one key; and by ADR-0064: a goal check's deadline and a periodic job are a subscription's own `Schedule`, not a clock subscription; its waits on the owner, but a secret act, are an agent's (ADR-0087). Source: the owner's requests of 2026-11-02 (the goal judge is hired automatically; list everything
Remit does automatically like that and move it onto the signal mechanism; design it long term, in one go, as much as is right),
`docs/design.md`, ADR-0013, ADR-0014, ADR-0024, ADR-0032. Extends ADR-0014 (its actions gain `hire`, `end` and `run`; its sources
gain `fact`) and supersedes none of it.

## Context

ADR-0014 made the wake, the start and the placement of an event the firing of a subscription. Everything else Remit does on its
own is still written as a call at the place where the cause happens, or as a timer in `Tick`:

- **Hires:** the goal judge (`RequestFinish` → `startGoalCheck`, `goal.go:458`), the judge of a key result's check (`StateCheck` →
  `startCheckReview`, `check_review.go:72`), the intake of a queued chain (`runReady`, `schedule.go:180`).
- **Deadlines:** the goal check's limit (`expireGoalChecks`, `goal.go:660`).
- **Periodic jobs:** the merge request sync, the worktree collection, the database snapshot and the event packing,
  each its own `if now.Sub(o.lastX) >= everyX` in `Tick` (`schedule.go:120-135`).

The cost shows in four places. The owner cannot see, route, pause or change any of these on the Subscriptions tab, though he can
for a routine; "have the QA employee judge every finish" is a code change. Each has its own dedup, its own restart story and its own
admission call. A new automatic act (release notes when a chain finishes, a second judge for a risky change) is a new call site in
the orchestrator, never a sentence to the Assistant. And ADR-0032 makes the judge a hired employee the owner names, which is a
policy: the code that picks who judges, when and from what is the thing that should be data.

The mechanism to carry them exists. A step stores its change, the readings it records and the firings it is the effect of in one
transaction (`records.readings`, `records.firings`, `apply.go:128-136`), so a fact and the act that follows it are stored once, and
a restart acts once. That is a transactional outbox already; what is missing is the facts, and the actions to answer them.

## Options

1. Keep the calls and add subscriptions only for new automations. Two ways to do the same kind of thing; the existing ones stay
   invisible to the owner.
2. Move every automatic act, the replies of a command's own transaction and the liveness rails included. Maximal uniformity, and it
   puts a subscription between a tool call and its answer, and lets the owner pause the rail that ends a stalled chain.
3. **Move each act that is a policy, and keep inline what is a reply or a rail.** The test is one question: *would the owner want to
   see, route, pause or change this act, or is it the answer to the command that caused it?* Chosen; the sorting is below.

## Decision

### The fact

- **A fact is a reading of `fact:<noun>.<past-tense verb>`** (`fact:chain.queued`, `fact:chain.finish-requested`,
  `fact:check.stated`; a fact joins the list when something answers it). The value names what it is about (`{chain, card, …}`), never
  a payload of the work. One key for all chains, with the chain in the value, so a subscription's `SeenRev` stays one cursor over one
  key and no wildcard key exists. The Chair's own `chain:<id>.mr.<repo>` and `chain:<id>.live` keep their shape: they are what a
  chain watches, and a fact is what Remit answers.
- **The fact and the decision of a step are written together.** A decision (`model.Decision`, the owner's history) and a fact are
  the same event told to two readers; the step that changes the state stores both in its transaction, and the writer of each is the
  one helper of its site (`requestGoalCheckCmd`, `StateCheck`, `putQueuedFact`), never a call beside it.
- **Written by the step that changes the state**, in its transaction, so the fact exists exactly when the state does. After the
  commit `react` has the subscriptions to its key judge it at once, so a fact is answered in milliseconds, as an `emit` is. Nothing polls the store for
  a state to find what changed.

### The actions

ADR-0014's actions are wake, start, tell, dispatch, skip. Three are added; each is closed, named, and takes one of ADR-0014's
admission rows:

| Action | Does | Admission |
|---|---|---|
| `hire` | Creates one card, from a role the subscription names (`goal-judge`, `check-reviewer`, `intake`), in the chain the fact names, with a brief Remit builds from the fact. The card, its brief and the firing are one transaction | a provider that can run agents; the intake pool for `intake`; no slot |
| `end` | Ends the goal check of the chain the subscription's `Subject` names, saying why: the judge stops, or, when none was hired, the chain finishes without the check. The effect is a state change that holds when done twice, so it comes before the firing is stored. Only a Remit-owned subscription may hold it (`fire` refuses any other) | none |
| `run` | Runs one job from a closed registry Remit ships (`sync-merge-requests`, `collect-worktrees`, `snapshot`, `pack-events`); a subscription names a job, never code. A job at work is not started again; its next time comes round. The firing is stored, then the job starts in the background: a restart between the two loses that one run, which the job's next period or its own next look repeats | none |

None merges, signs in, pauses, resumes, stops a chain, or touches a credential. `end` reaches into a chain's life only as the
deadline of a goal check does today, and it is the one action whose reach ADR-0014 did not list, so this record lists it
(*Threats*).

### What moves and what stays

| Act | Becomes | Why |
|---|---|---|
| Goal judge on a finish | `fact:chain.finish-requested` → `hire` goal-judge. `RequestFinish` keeps every refusal (open watch, unanswered owner, unaccepted reports, open plan steps, the limit of checks), sets `Chain.Finishing`, writes the fact and returns | Who judges, and whether, is policy (ADR-0032) |
| Check review | `fact:check.stated` → `hire` check-reviewer | Same |
| Intake | `fact:chain.queued` → `hire` intake | Same; `admit` keeps the intake pool and the order (`readyToRun`) |
| Goal check deadline | A one-time clock subscription of Remit for the chain (`sub_check_<chain>`, `Next` = the request's time + the limit, `end`), stored with the request and deleted in the step that ends the check or closes the chain out | A deadline is time; it ends a check no judge was hired for as well as one that gave no verdict |
| Periodic jobs | Builtin clock subscriptions `@every …` with `run` | One scheduler concept; `lastMRSync`, `lastCollect`, `lastSnapshotCheck`, `lastPack` go; the owner sees the period and may change it; a missed time fires once (ADR-0014) |
| Owner-made automations | A fact is subscribable like any signal: a subscription on `fact:chain.finish-requested` or any later fact starts a chain or tells a Chair. The Assistant's subscription tools that make one are the seam (*Revisit when*) | The payoff: no form, no code, no new call site |

**Stays as code.**

- **Admission of a queued chain** (`startChain`, slots, queue order): it is the rail of ADR-0001, ordered across chains.
- **Finish gating** (`RequestFinish`'s refusals) and **the transition of `Chain.Finishing`** (verdict claimed, refusals counted, check
  ended): one atomic state machine in `step`; it writes `fact:chain.goal-checked` for whoever listens.
- **The replies of a command's own transaction:** the Chair told a submit's result, a permission asked, a verification's result
  delivered, a retirement told. Nobody would pause these, and a subscription between a call and its answer only adds delay.
- **Liveness rails of absence:** nudges and the finish of a stalled chain, the silent-turn cut, the idle watchdog, session recovery.
  A subscription fires on a fact, and these fire on none; what is not there is not a reading. They stay in `Tick` until a source of
  absence exists (*Revisit when*).
- **The wake of cards waiting for a provider** (`wakeWaiting`, `failover.go`) and **the chair-permission timeout** (`permission.go`): the first probes a provider's health and keeps its state in memory, the second is a wait with no reading to hear. Both stay in code until a source of health and of absence exists (*Revisit when*).
- **Door rules** (skip, tell, live) and the pollers of rollups and pulls: they are already subscriptions and source adapters.
- **Key-result owner wake** (`objective_open.go`): its policy is the key result's owner, which is data; a subscription would
  carry a start spec the producer computed. Left until a second producer needs it.

### The shipped subscriptions

The facts above and the subscriptions that answer them ship as `Builtin` subscriptions, as the built-in event signals do
(`event_signal_builtin.go`): the code gives their settings, a change by the owner is kept as his (`Edited`) until he resets it, and
none is deleted. He reads them on the Subscriptions tab and pauses or resumes one by telling the Assistant (`pause_automation`,
`resume_automation`); their other settings are the code's. **A paused judge subscription is the owner's
choice not to have a goal check:** `Chain.Finishing` then runs out at its deadline, and the chain finishes without a check and says
why, which is the path `endGoalCheck` takes when a judge gives no verdict.

### One change, one start-up

All of it lands together:

- `SeedSubscriptions` runs at each start, before `Recover`: it makes the shipped subscriptions that are missing (a fact subscription begins at the last reading of its key, so history is not replayed; a clock one at the next time of its cron), brings the non-edited ones up to what ships and leaves the owner's;
- `Recover` writes `fact:chain.queued` again for each queued chain with no intake and stores the deadline of each chain with a goal check no one has ended;
- `startGoalCheck`, `startCheckReview`, the intake call in `runReady`, `expireGoalChecks` and the four timers are deleted, with their tests; the brief of each hired role is built by its `hire*` function, one owner;
- `hireNeeds` says once what each role takes from admission (the judge's and the check reviewer's: a provider that can run agents, as read from the code);
- `signalKeyOf` and `signalSourceSpecs` gain the `fact` source, the contract (`web/src/rpc/define.ts`) the new action kinds, source and subscription kind, and the Subscriptions tab shows them with their firings.

## Threats

| Threat | Control |
|---|---|
| A subscription on a fact starts agents without end (a chain finishes, a subscription starts a chain, which finishes) | The seam for a damping window is `Subscription.Window` (cooldown, longest) and for a depth limit `Cause.Depth`; neither is built (*Revisit when*) |
| The Assistant, injected, adds a subscription that hires or starts on a fact to run work the owner never meant | Making or changing a subscription of Remit whose action is `hire`, `start` or `end` waits for the owner's Apply when anyone but he sets it, as a rail-moving write does (ADR-0024 `holdEventSignal`'s pattern) |
| A paused or deleted judge subscription lets a chain finish unchecked | It is the owner's right, and it is shown: the finish records "no goal check" with the reason (`goal_failed`); a chain never hangs on `Finishing` because the deadline subscription ends it |
| `end` becomes a way to kill chains | `fire` refuses `hire`, `end` and `run` for a subscription of any owner but Remit; `end` reaches only a goal check that is under way, and the firing records why |
| A fact's value steers a hired agent (OWASP LLM01) | A fact carries ids, never the Chair's words; the brief of a hired role is built by Remit from records it reads |
| A restart double-fires or drops a hire | The card, its brief, the firing and the subscription's progress are one transaction; a firing stored already does nothing twice (ADR-0014) |

Controls not built in this change, named for *Revisit when*: the depth limit (`Cause.Depth`), the Apply gate on owner-set
`hire`/`end` by the Assistant, and the per-day cap for owner-made subscriptions. The Assistant's tools that make a subscription on a fact, or change a shipped one beyond pausing it, wait for the same revisit. The seams are the field, the hold in the
subscription tools and the cap constant; threat modelling (`aio-threat-models`) runs when the owner asks for hardening.

## Consequences

- One path for every automatic act: fact, subscription, firing, effect. What Remit does on its own is listed on one page, the
  Subscriptions tab, with who made it and what it did last.
- The judge's policy (who, when, how many) is data the owner changes by talking; ADR-0032's hired judge needs no new call site.
- Latency of a hire is one pass of `judge` after the step commits, held by admission when no provider answers, instead of a call inside the
  Chair's tool. `finish_chain` returns "check requested" and the verdict reaches the Chair as a message, as it does now.
- `Tick` shrinks to admission, the liveness rails and the pollers; five hand-rolled timers and three direct hire calls go.
- The fact table grows with the product. A fact no subscription listens to costs one stored reading.
- Tests to write first, one for each behaviour: a finish requested hires exactly one goal judge and a replayed firing hires none;
  a paused judge subscription finishes the chain unchecked at its deadline and says why; a verdict before the deadline deletes the
  deadline; a queued chain gets one intake and a restart does not give it a second; each shipped job runs at its period and a time
  missed while the server was down runs once; `end`, `hire` and `run` are refused to a chain-owned subscription (`automation_test.go`).

## Revisit when

A source of absence is wanted (a chain that moved nothing for a time as a reading), which would carry the stall rail, the silent-turn
cut and the watchdog; a second producer needs the key-result owner wake as a subscription; a fact needs a field only one kind of it
reads (a flag in disguise); the volume of facts shows in the tick or the store; or the owner wants `hire` or `end` set by the
Assistant without his Apply.
