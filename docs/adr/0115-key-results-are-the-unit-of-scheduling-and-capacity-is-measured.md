# 0115. Key results are the unit of scheduling, and capacity is measured, not counted

Status: accepted, 2026-11-19; built 2026-11-19 (*As built*, below): the measured limit, the key-result tree,
deduplication by the key result served, the progress rail, the share by key result fed by what its landings came to
(`admit::fates`), back-pressure from the merge queues (`admit::waiting`), progress read from landings
(`admit::landed`) and competing attempts with `pick_attempt` (`crates/shipping/src/round.rs`). Supersedes ADR-0001 (the review room
and `-max-waiting-mrs`), ADR-0087's admission that paces to the merging agents, and the counts ADR-0010 kept for
scheduling (`-max-concurrent`; the wakes, watches and proposals a chain may have). Extends ADR-0031 (a key result may be
refined into key results of its own), ADR-0092 (the headroom kept for the owner is one of the pressures read) and ADR-0104
(no progress is also read on a key result). Source: the owner, 2026-11-19: "fully automate a multi-agent system: very many
agents together reaching goals instead of tasks; our job is to create as many conditions as possible for AI", and his
approval the same day of: no fixed count caps, compute to key results by measured capacity, key results that decompose,
deduplication by the key result served, competing attempts for a key result that scores itself, and runaway work read
from progress, not from counted wakes.

## Context

The customer of this decision is the person each objective is for: what he gets is the result, sooner, from as many agents
as the machine and the accounts can carry, with no agent's effort spent on work that duplicates another's or produces
nothing.

What paces work today is a set of counts, each a number chosen once:

| Count | Where | Default |
|---|---|---|
| Chains running that the scheduler started | `-max-concurrent` (`crates/remit/src/flags.rs:179`) | 2 |
| Approved MRs waiting before new work waits | `-max-waiting-mrs` (`flags.rs:181`, ADR-0001, `chains::admit::fits`) | 5 |
| Chains one chain may propose | `PROPOSALS_MAX` (`crates/orch/src/propose.rs:55`), and a title not reused for 7 days | 3 |
| Wakes and watches a chain holds | `SUBSCRIPTIONS_MAX` (`crates/chains/src/watch.rs:28`) | 20 |
| Times a chain is woken a local day | `WAKES_PER_DAY` (`watch.rs:31`) | 24 |

None of them measures a limit. The limits that exist are the Claude accounts' windows (the provider warns at 90%, and
ADR-0092 already holds what the scheduler starts while every account is near one, `chains::health::no_headroom`), the
host's memory and cores (one Claude Code process per card, and builds in the jail), and, after ADR-0114, how fast a
repository's queue lands. Two of the counts are proxies for that last one (`-max-waiting-mrs`, and `-max-concurrent` at
2, which is a guess at what one owner can review). Goldratt's warning fits: a policy constraint is passing for a resource
constraint, and the system runs at the policy's pace.

The unit those counts act on is the chain, so work is scheduled as tasks: first come, first served, a key result with ten
proposals starves one with one, and two chains that do the same work under different titles both run. Yet every chain
already serves a key result (`Chain.serves`): a direct request gets a hidden task objective with one key result, its goal
(design.md, *Objectives*), and an objective's key result is served by the chains its owner opens (`propose_chain`). A key
result is flat (`model::KeyResult` in `Objective.key_results`, `crates/model/src/objective.rs:169-269`): nothing refines
it.

The research behind the direction (`reports/Đa agent hướng kết quả.md`) sets the shape: planner, worker and judge scale
to hundreds of agents with recursive planners (Cursor), independent agents amplify errors 17.2 times against 4.4 with a
verifying orchestrator (Google Research and MIT, arXiv 2512.08296), multi-agent gains run from +80.8% on work that splits
cleanly to −70% on sequential planning (same study), and search with an automatic evaluator found what single attempts
did not (AlphaEvolve, deepmind.google). More agents buy breadth, not correctness; the verifier and the decomposition
decide what they are worth.

Mental models used: **Bottlenecks** (pace starts to the constraint, the drum, and release work no faster than it lands,
the rope), **Feedback loops** (a limit read from what happened after the last change of it is a balancing loop; a count is
an open loop), **Diminishing returns** (the tenth chain on one key result adds less than the first on another). Patterns:
Load Shedding's admission control on measured signals and Queue-Based Load Leveling (vols. 3 and 4).

## Options

1. **Keep the counts and raise them, or let the Assistant set them.** Simple to read, but still a number, still on the
   chain: right on one day and wrong when an account is added or the queue slows.
2. **An adaptive limit on chains** (additive increase while nothing presses, halve when something does), first come first
   served. Removes the guessed number but keeps the task as the unit: duplicates by title, no share between key results.
3. **The key result as the unit, on measured capacity:** a global limit read from pressure, split across key results so
   each served one runs before any runs twice, each key result's share grown and cut by what its work did on landing,
   back-pressure from the queue, key results that refine into key results, deduplication by the key result served,
   competing attempts where a check scores them, and runaway work read from a key result's progress. **Chosen.**
4. **A market:** key results' owners bid for capacity from a budget. It needs a price, which ADR-0010 removed with every
   cost record, and gives agents a currency to game. Rejected.

## Decision

Option 3.

### The global limit is measured

- `chains::admit::capacity(prev, pressure) -> usize`, pure: with no pressure and a start waiting, the limit grows by one a
  Tick; under pressure it halves; it is never below 1 and never below the chains running (a limit stops nothing that runs,
  it holds what would start). It starts at the number running when the server starts, at least 1.
- Pressure is any of: every account that can run agents near a limit its provider warned of (ADR-0092's headroom, which
  keeps its own rule for the owner's turns), a rate-limit refusal or an account failover since the last Tick, the host's
  free memory below twice the mean resident memory of a running session, or its one-minute load above its core count.
  Each pressure is a field of `Pressure`, the seam where a new one is added.
- `-max-concurrent` goes. As today, the owner's own actions, an early wake by a message and a chain already at work take
  no place in the limit (`chains::admit`'s table).
- The limit and the pressure that set it are shown where held starts are shown today (the Subscriptions tab and a held
  chain's Overview), worded by the server.

### The limit is shared by key result

- A start is charged to the key result its chain serves. Each key result has a share: 1 to begin; one more each time a
  landing of its work lands and is not reverted; halved, never below 1, when one of its landings is reverted or returned
  for a conflict or a failed combined check (ADR-0114). Work that breaks the base gets less room; work that lands gets
  more.
- When the limit has room, the next start is chosen among key results under their share: first those with no chain
  running (every served key result runs one before any runs two), then by the objective's horizon (`Horizon.until`,
  soonest first, none last), then the oldest queued start.
- **Back-pressure:** a key result whose landings waiting in queues (`ready`, `combining`, `checking`) already number its
  share takes no new start: its work waits on the queue, and more of it would only wait too. The queue's measures (time
  from ready to landed, the share returned) are the signal; no count of waiting changes stands in for them.
- A scheduled wake of a chain at work is not a new start: it takes a place in the limit, charged to the chain's key result,
  and is never refused by its share.

### Key results refine into key results

- `model::KeyResult` gains `parent` (the key result it refines, of the same objective) and becomes an entity of its own
  (table `key_results`), since agents add them without bound: what grows is not a field of `Objective`.
- The key results without a parent are what the owner committed to; they are drafted and applied with the objective as
  today (ADR-0031, the Assistant for him, ADR-0087). A child is written by whoever answers for its parent (that employee's
  chain, or the Assistant for a key result the owner answers for) with `refine_key_result` (text, check, who answers, who
  judges), recorded as a `Change` the owner reads and overturns, and applied at once: it changes how the result is reached,
  not what was promised, and the parent's check stays the commitment.
- A chain refines, and opens chains for, the key result it serves and what refines it when it works for an employee or
  on the owner's direct request (its task objective); a chain serving a key result the owner answers for in an objective
  he committed to refines nothing under it: that key result is his, refined by the Assistant for him. Beyond its own
  key result, a chain opens work only for key results answered by the employee it works for (`Crew::may_open_for`).
- Refining is one transaction, replayed to the same end: a new child's id is derived from its parent and its words, a
  change keeps what it leaves empty, and who answers and who judges pass the rule an objective's do
  (`names_who_answers`: an active employee under the title the field asks for, the judge outside the owner's team).
- A key result with a check is met when its check is; one without a check is met when every child not retired is
  (refinement by AND, as in KAOS goal models). A child retires with its parent and when its parent is met. Status stays
  derived on every read and never stored.
- The Objectives tree shows the children under their parent, each with where it stands and, when stalled, why it takes
  no new chain.

### Deduplication is by the key result served

- `propose_chain` names the key result its chain serves. One that names none serves a new child of the proposer's own key
  result, made from its goal. A proposal is refused when its key result is met or retired, or already has a start queued:
  the refusal names that chain, whose Chair the proposer messages instead. Similar intents under different key results
  are caught where judgement is: `refine_key_result` and `propose_chain` list the siblings and their chains before they
  write.
- `PROPOSALS_MAX` and the title rule of `orch::propose` go.

### Competing attempts where a check scores them

- A key result whose check is a `command` (run in a repository's worktree) may be served by several chains at once as
  attempts of one round (`Chain.attempt`: the key result and the round), opened by whoever answers for it, as many as its
  share. Each works in its own worktree and passes its gates as any chain.
- An attempt's version does not enter the queue by itself. When every attempt of the round has an approved version or has
  finished, a fresh judge on the strongest model (ADR-0102) runs the check on each approved head, reads them on the
  customer's side, and picks one (`pick_attempt`, a decision with its reasons, overturnable like any). The pick enters the
  queue; the others finish without landing, and the judge's reasons are kept on the round for the next.
- A `reading` check reads the running system, so its attempts cannot be compared before one lands; they wait for a stage
  that splits exposure between attempts (*Revisit when*).

### Runaway work is read from progress

- The no-progress rail of a chain stays (ADR-0104, `chains::spin`), and so does the stall rail.
- Progress on a key result is made by a finished chain serving it or a key result refining it: its work landed
  (`admit::landed`), or its goal judge's latest verdict found its key result met (`chains::goal::verdict_on`, the rule
  the key result's status reads). A key result whose last 3 such chains (`MAX_NUDGES`, the stall rail's number) finished
  without progress is stalled: neither it nor what refines it takes a new start (`admit::stall_on`), so a fresh child
  does not step around it, and the Objectives tree, the held start and `read_objective` name those chains. The Assistant
  lifts it with `lift_key_result_stall`, recorded as a `Change` with who and why, and only chains queued after the lift
  count again; the owner lifts it by telling the Assistant. A chain the owner asks for is never held. The lift is rail
  state, not the commitment: writing the objective again keeps it.
- `WAKES_PER_DAY` and `SUBSCRIPTIONS_MAX` go: a chain woken to no effect is caught by its spin rail, and a subscription
  that never fires costs nothing. A watch's one minute between firings stays: it coalesces a burst of readings into one
  wake, which is correctness, not a cap.

### What stays a count

The limits on what an outsider can start are entrance rails, not scheduling: a client key's chains a day and acts an hour
(ADR-0009) and `-event-chains-per-day`. They bound what a key or a door's sender makes Remit do, and AGENTS.md keeps a
change to an entrance behind its own decision; this record does not lift them.

The script sensors one chain runs stay a count (20, `pipeline::sensor_settings`), now its own constant rather than
`SUBSCRIPTIONS_MAX`: a sensor runs a script in the jail every period without waking any chain, so neither a chain's spin
rail nor the progress rail sees what it costs the host.

## Threats

| Threat | Control | Seam not built |
|---|---|---|
| A flood of starts exhausts the host | The limit halves under memory or load pressure and only gates new starts; the owner's pause and stop are untouched | A ceiling from the host's cores and memory, if halving is measured too slow |
| An agent writes key results without bound | A child takes no compute until a chain serves it, and serving it is charged to its share; every child is a `Change` the owner reads | A depth or breadth bound on refinement, if trees are measured growing without landing |
| Progress is gamed by trivial landings (Goodhart) | Every landing passes its gates on the strongest model and the combined checks; the parent's check stays the commitment | Progress read as the key result's check reading moving toward its condition |
| An outside system floods a door | The entrance counts above, the door's keys and signatures | A rate per door |

## Consequences

- Throughput follows the accounts, the host and the queue: an account added or a faster queue raises it with no flag
  changed, and next year's models, which break the base less, get more room by the same rule.
- What the owner committed to stays the top of a tree only he and the Assistant for him change; how it is reached is the
  agents', recorded and overturnable.
- The chain stays the unit of work and of the stall rails; the key result becomes the unit of scheduling, deduplication
  and progress.
- `chains::admit::Room` loses `mrs_waiting`, `max_waiting_mrs` and `slots` for a limit and the shares; `merges` and
  `asked_by_release` go with the review room. The flags `-max-concurrent` and `-max-waiting-mrs` go.
- Migration 0121: each objective's `key_results` move to `key_results` with no parent, keeping their ids and their
  order; each hidden task objective's key result likewise.
- A chain's Chair states the check of a refined key result that has none (`state_check`), since a chain serving it
  cannot finish without one (ADR-0068); once stated, whoever answers for it changes it with `refine_key_result`. An
  amendment names only the committed key results: one naming a refined key result's id is refused.
- A proposal that names no key result serves a new one refining the proposer's own, its id derived from the proposer
  and the title, so a proposal made again finds the key result it made.
- The rails it moves keep their tests where they exist and gain one for each new one: the limit that never stops what
  runs, a share halved by a revert, a stalled key result that holds starts but never the owner's, the migration of key
  results read back. None is named before it exists.

## As built

- **Competing attempts (2026-11-19).** `Chain.attempt` names the round a chain is an attempt of, a `model::Round` of its
  own (table `rounds`, migration 0125: the key result, its number, its state `open`, `judging`, `picked` with the
  judge's reasons and the owner's overturn, or `ended`). `propose_chain` with `attempt` opens one, joining the key
  result's open round or a new one (`round::joined`), refused for a check that is not a command and past the key
  result's share (`round::attempt_refusal`); the attempt's goal is followed by what it is and what earlier rounds taught
  (`round::attempt_goal`, `round::earlier_rounds`), and the round is written over the revision read in the transaction
  that queues the chain, so a join after a judge started is refused. `land::approve` reads one rule,
  `round::approved_state`: an attempt's approved version becomes `LandingState::Contending` instead of `Ready` while its
  round is unsettled, and goes back not picked once it is settled. `round::standing` says when a round is judged (every
  attempt contends or finished), read when a version contends and at each landing sync (`judge_rounds`); the judge is a
  `JudgeKind::Round` card on the judge model in the first attempt's chain, holding the working tools and
  `pick_attempt`. The pick is a `DecisionKind::Pick` on the picked chain, `round::picked` moves the versions in one
  write (the pick's `Ready`, the others `Returned { NotPicked }`, which the outcomes count as nothing the queue found,
  and no fate of the share), each Chair is told, and the queue runs. The owner's overturn (`overturn` on the pick, through
  the Assistant) sends the pick back while it waits or reverts it once landed (`overturn_pick`), then records the
  overturn on the round. `admit::outcomes_of` reads an attempt's progress from its whole round. Tests: the round's rules
  as tables (`shipping/src/round/tests.rs`) and the main path through the application
  (`orch/tests/app/shipping_round.rs`: two attempts contend, the judge picks one, it lands and the other never does).

## Revisit when

- **ADR-0114's queue records landings:** `admit::share` takes them (`Landing::Held`, `Landing::Taken`) for each key
  result, the back-pressure of its waiting landings joins `Room`, `admit::landed` reads the queue's landing in place of
  a merged MR (and a reverted landing stops counting as progress), the Objectives tree shows each key result's share
  once it can differ from 1.
- **A stalled key result needs telling, not only showing:** whoever answers for it is told once what those chains did;
  today the Objectives tree, the held start and `read_objective` say it, and the seam is the `chain.finished` fact a
  shipped automation would subscribe to.

- **The limit oscillates** (halving and growing within minutes) or holds work while the accounts have room: the seam is
  `capacity` and the `Pressure` it reads.
- **The owner's order of key results differs from the horizon's:** a priority he states in words to the Assistant, a field
  of the objective read before the horizon.
- **A reading check needs competing attempts:** a stage that splits exposure between attempts (ADR-0114's stage plan) and
  a judge that reads each attempt's share of the signal. Today `round::attempt_refusal` refuses an attempt at a key
  result whose check is a reading; the seams are that refusal and `round::approved_state`, where a reading round's
  versions would go to their stage instead of contending.
- **A round's judge fails or its chain goes:** today the round ends without a pick and every version contending in it
  goes back (`round_judge_ended`), and a judge whose chain is deleted while it judges leaves its round judging. The seam
  is `judge_rounds`, which would start a judge again on a round whose judge is gone or failed, with a count of tries.
- **A pick's overturn should land another attempt:** today an overturn takes the pick back and keeps the owner's word for
  the next round, since the other versions went back at the pick; holding them until the pick lands is the seam
  (`round::picked`).
- **A command check prints a score:** the judge reads the score and later rounds start from the best (evolutionary
  search), instead of a pick among passes.
- **Hardening is asked for:** the host ceiling, a refinement bound, a rate per door.
