# 0014. Signal is the backbone: every wake, start and placement is the firing of a subscription

Status: accepted, 2026-10-30; partly superseded by ADR-0049: a subscription listens to a key pattern (`Match`) with one cursor over the log (`SeenSeq`), and time is a clock sensor, not a clock subscription (`Cron`, `Next` and the clock branch of the evaluator are gone), and GitLab's event signal and its rules are shipped by the gitlab plugin; the shipped routine `rt_scout` it names is gone (ADR-0023); superseded in part by ADR-0024: the `delivery` source is the `event` source, and by ADR-0036: an event signal's rules are subscriptions of their own, with no placement or burst; ADR-0064 puts time back on the subscription, as its `Schedule` and `DueAt`, in place of ADR-0049's clock sensor. Source: the owner's requests of 2026-10-29 (one generic signal-and-subscribe system like Vue's
reactivity; inputs handled differently per source; the scheduler, `check_back`, routines and webhooks refactored onto it in
one go; a chain does not wake early on news, it subscribes to a time), `docs/design.md`, ADR-0001, ADR-0008, ADR-0011,
ADR-0012, ADR-0013. The implementation plan is `docs/superpowers/plans/2026-10-29-signal-backbone.md`. This record generalises
ADR-0008, ADR-0011 and ADR-0012 and supersedes none of them: bursts, preconditions and the shipped routine keep their rules,
and their owner becomes this system.

## Context

Four mechanisms share one spine, *something happens or becomes true, a condition is judged, admission and quotas are
applied, an action follows*, and each implements it alone:

- the scheduler wakes a waiting chain at its check-back and starts queued chains (`schedule.go`, `checkback.go`);
- a routine starts a chain at a time, and the Scout runs on a period of its own (`routine.go`, `schedule.go`);
- the webhook door stores a delivery, places a burst of posts about one chain together, judges preconditions in order and
  hands the rest to a Dispatcher (`webhook.go`, `burst.go`, `door.go`, `precondition.go`);
- an agent that waits on a condition runs its own until-loop in its own session, and its news wakes the chain early
  (`background.go`, `newsCmd`).

Each has its own dedup (a sender's retry key, a burst's quiet time, the own-account skip, the stall rail), its own storage and
its own way of surviving a restart. A condition watched by many is judged by each of them. A check-back wakes early on any
message or news, so what wakes a chain is not written anywhere. ADR-0013 needs one more kind of wait: a chain that watches a
signal after its change is live.

## Options

1. Stored signals and subscriptions, one evaluator in the server's process, adapters per source.
2. A durable event log as the one truth, chain state derived from it. Fits the telemetry store at 100k events/s; here it
   would put a second storage model beside `step`/`apply` and rewrite events, which the store never does.
3. An in-memory reactive graph over the records that exist. Wakes are lost at a restart unless re-derived, and there is
   no stored subscription for the owner to read.
4. Keep the four mechanisms and extract helpers. Changing the spine stays a four-place change.

## Decision

Option 1.

- **Signal** is a named fact with a current value and a revision (`delivery:gitlab`, `rollup:checkout.error_rate`,
  `chain:<id>.mr.<repo>`). A **reading** is one value at one revision, with the time and the cause (who or what, and the chain
  it stems from). A **subscription** says who wants a signal, the condition (CEL, the language of preconditions), the window,
  and the action: wake a chain, start a chain, tell a Chair, hand to a Dispatcher, skip. A **firing** records that a
  subscription acted on a reading; `(subscription, key, revision)` is its key, written in the same transaction as its effect,
  so a retry or a restart acts once.
- **Each source has an adapter and keeps its own rules.** Deliveries keep sender authentication, the store before the answer,
  the quota and the Dispatcher's judgement; telemetry rollups are read, never the raw store, and a silent source holds its
  subscriptions and says so; a merge request's state and a deploy are readings of the chain's own records. Only normalising a
  source into readings is source-specific; everything after is shared.
- **Time is the subscriber's own.** A clock subscription carries an instant, a cron or an interval, stored as the next
  absolute instant. Time is not a stored signal and no reading is written for it: the scheduler's tick takes the
  subscriptions that are due. Any subscription may also carry an `until`, a deadline at which it fires once, expired.
  A time missed while the server was down fires once and counts on from now; the missed times are not replayed.
- **`check_back` is a subscription.** The tool of the Chair that plans a return becomes the subscription tool, naming a time
  or a signal. A chain waits only when it holds a wake subscription, and the firing of one wakes it. Nothing else wakes
  a chain: an agent's background news does not, and a message to a waiting chain's agents does not cancel its subscription.
  What a chain waits for is what it subscribed to, and the Signals page lists it.
- **An agent waits on the server, not in its own loop.** To wait for a condition it subscribes; a waiting chain closes its
  sessions. The until-loop a session runs inside one turn is unaffected.
- **The owner's acts are direct.** A message from the owner, a resume and "check back now" reach the chain at once and never
  queue, as admission already holds (`admit.go`); they are rails, not subscriptions.
- **Routines and the Scout are clock subscriptions that start a chain.** The `routines` table is dropped; the Scout is the
  shipped routine `rt_scout`, seeded from `-scout-every` as `rt_self_improvement` is. The owner still tells the Assistant;
  the tool names the Assistant holds do not change.
- **Burst coalescing lives once, in the evaluator**: readings of one key within the quiet time are one firing.
- **Admission is chosen by the cause of the firing.** A wake by a clock takes a slot and a provider; a start passes the review
  capacity (ADR-0001); the owner's acts take nothing. The table in `admit.go` stays true line by line.
- **Loops are damped by code.** A subscription judges each revision once; a cooldown follows each firing; a chain's wakes a
  day are counted; a reading caused by the subscriber's own chain is always skipped (`signal.Evaluate`, `hears`): the records a chain watches are the ones its own work makes, and `chain:<id>.mr.*` and `chain:<id>.live` are caused by Remit, never by the chain.
- **The board derives `watching`** from a chain's subscriptions (ADR-0013); no status is added to the chain.
- **One change, one migration.** Check-backs and routines become subscriptions in a numbered migration, the old tables and
  fields go, and the mechanisms they replaced are deleted in the same change.

## Threats

| Threat | Control |
|---|---|
| A forged or flooding post at the door | Unchanged: sender key, refusal that never says why, size and pending limits, the quota; the adapter stays at the entrance |
| Text in a reading steers a Chair or a Dispatcher (OWASP LLM01) | A reading reaches an agent as evidence with its key, revision and cause, never as an instruction; conditions are written by the Assistant from a real delivery, never by the sender |
| A chain subscribes to wake itself without end, or to another chain's signals | A subscription belongs to the chain that made it; a chain's wake subscriptions only; cooldown, wakes a day and a cap of active subscriptions per chain |
| A wake storm takes every slot | Admission by cause holds the surplus with its reason on the Signals page, earliest first; firings are idempotent, so held ones fire later without being judged again |
| A silent telemetry source leaves a watch unanswered | The source is marked silent after three periods, its subscriptions are held and say so, and the `until` still fires: the chain finishes unverified and says why |
| A restart double-fires or drops a wake | The firing and its effect are one transaction; the migration converts waiting chains' check-backs so each wakes once |
| A subscription removes a rail | Actions are wake, start, tell, dispatch and skip; none merges, signs in, pauses, stops or touches a credential |

## Consequences

- One evaluator, one idempotency rule, one coalescing rule and one place to read what is waiting for what.
- `Chain.CheckBack`, the `routines` table, the Scout's separate branch, agent news as a wake and the placement code of the
  door go. The Signals page lists subscriptions; the contract (`web/src/rpc/define.ts`) and the board learn the new kinds.
- An agent that used to wait with `Monitor` across a chain's wait subscribes instead; the briefs say so.
- A telemetry rollup becomes a source that tasks can wait on, which ADR-0013 needs.
- The change is large and sits on the path of every chain: the existing behaviour tests are the net, and new ones are written
  first (see the plan). The live instance is the working tree, so the change is backed up, built and restarted the
  `AGENTS.md` way.

## Revisit when

A kind of subscription needs a field no other kind reads (a flag in disguise); the evaluator's cost on a busy key shows in
the tick; or the telemetry store's volume makes a durable log the better home for readings than the owner's database.

Enforced by: TestAFiringIsStoredOnceWhoeverRetriesIt, TestAWatchSurvivesTheClockWakeAndTheChairWaitsAgainOnIt, TestAMessageWakesAChainThatOnlyWatchesAndLeavesTheWatchOpen, TestAChainHoldsAtMostTwentyWakesAndWatches, TestAChainIsWokenAtMostTwentyFourTimesADay, TestAChainsSensorWhoseScriptKeepsFailingIsHeldOnItsWatchLineAndTheFirstGoodRunClearsIt, TestMigration46TurnsCheckBacksIntoWakeSubscriptions, TestAFiringAndItsChainAreStoredTogetherOrNotAtAll, TestAFiringAlreadyStoredQueuesNothingAgain, TestAReadingGoesToEverySubscriptionItConcerns