# 0013. A chain owns its outcome until the effect is verified

Status: accepted, 2026-10-30; its merge rail (*Ownership is not authority*) is superseded by ADR-0050 for a chain that holds the ship capability; its waits on the owner, but a secret act, are an agent's (ADR-0087). Its closing words (Confirmed, Closed unverified) and its expiry rule are amended by ADR-0067: a finished chain is Done, Waiting on you or Not done. Source: the owner's requests of 2026-10-29 (a chain must not be done when its MR is merged: work
runs, waits, is judged, improved and repaired after it ships, for every task and not only Remit's own), `docs/design.md`
(Chain statuses, `check_back`, the goal judge), ADR-0010 and ADR-0012. The reference articles consulted: safe deployments
(bake time, automated rollback), SLO/SLI and error budgets, fitness functions with ADRs (`aio-architect-reference`).

## Context

A chain ends when its Chair finishes it and the goal judge accepts the result. The owner merges the MR, a rail no agent
lifts. After the merge nothing in the system keeps looking at whether the change did what the goal asked: the board's `done`
column (`standing.go`, `columnOf`) holds every chain that is not queued, running, waiting or asking, merged or not. A change
that regresses in use is found by a person, or by a later chain that happens to read the logs (ADR-0012's Scout), and the
chain that made it, which holds the context, is long finished.

The owner wants every task to be owned by the agents until it works: they wait, observe, judge, improve and repair, and do not
stop at the merge.

## Options

1. Keep the end at the merge. The owner finds regressions himself; the Scout may notice some days later.
2. A separate standing "watch" playbook or routine that observes what shipped and opens a chain when something is wrong. The
   builder of a change and its keeper are different agents with different context, and a change has two owners.
3. The chain that made the change owns its outcome until the effect is verified: the goal names how each requirement is
   checked, the chain waits through the observation window after the owner's merge, and a bad reading wakes that same chain.
4. Rely on the deployment system's own staged rollout and automated rollback. A sound practice where it exists, but it checks
   health and not the goal, it is not Remit's, and many tasks have no deployment system.

## Decision

Option 3.

- **Done means verified.** Each requirement of a chain's goal carries how it is checked: when the change is submitted (a
  document, a test), or by a reading of a signal over an observation window after the change is live, with a threshold and an
  expiry. The goal judge, which already gives `met`, `not_met` or `unverified` per requirement with evidence, rules on it.
  The check is a property of the requirement, not a flag on a playbook.
- **The check is judged before the work starts, by someone other than its author.** A chain that sets its own measure and then
  passes it proves nothing. The goal judge reviews the check when the goal is fixed, and the owner sees it in the chain.
- **Every change ships its signal.** The chain that makes a change also makes the way to see its effect (a probe, a metric, a
  scheduled check). A task with no signal says so and proposes how to measure it; it is not finished in silence.
- **After the merge the chain waits.** It goes to `waiting` with `check_back` set to the end of the window, which takes no
  slot (`design.md`, `check_back`). At the check-back the Chair reads the signal through a named capability (not every task
  has one), and the judge rules again.
- **One change, one owning chain.** The change and its MR belong to the chain; a deploy or a signal that names the change
  wakes that chain (`wake`), not a new one without the context.
- **A bad reading reopens the owner of the change** (`reopen`), by default. The chain proposes the repair or the revert as an
  MR; a chain is started instead only when the fault lies outside the goal the chain was given.
- **The expiry is honest.** When the window ends without a reading, the chain finishes as unverified and says why; it never
  waits without end.
- **Ownership is not authority.** The merge, the sign-in, pause and stop and every other rail stay the owner's, and no agent
  lifts or moves one. A rollback the deployment system does by itself is that system's rail; the chain observes it and
  proposes the revert in an MR the owner merges.
- **The board shows it.** A chain that is merged and still observed has a column of its own, `watching`, derived in
  `standing.go` from the chain's records (it is `waiting`, has a merged MR and an unruled check) and never stored as a status.
  It adds nothing to what waits for the owner. A regression moves the chain to the column that waits for him or to running.
  `done` tells a confirmed chain from one closed unverified in words and an icon, not in colour alone.
- **Where the signals come from.** For the products Remit builds, the telemetry store (`docs/telemetry-store`); for
  Remit's own changes, its records and logs (ADR-0012), where the window counts chains started after the owner puts a
  change live and not hours.

Not yet established, to read in the code before the build: whether `finish_chain` already refuses a result with an
`unverified` requirement; where the system learns that a change is live (a deploy event or the owner saying so); and how a
signal names the change that caused it.

## Threats

| Threat | Control |
|---|---|
| A chain defines a check it can pass (Goodhart) | The goal judge reviews the check before work starts; the owner sees it; one requirement at least is a result the chain does not produce |
| Text in a signal, a log or a ticket steers the Chair (OWASP LLM01) | A reading is evidence to verify against its source, never an instruction; the capability is read only |
| A chain takes "ownership" to merge, roll back or restart | Those are rails: merge and sign-in stay the owner's; the chain opens an MR; ADR-0012's limits on a chain that edits the running system stand |
| A chain waits for ever or holds a slot | Every check has an expiry and finishes unverified; `check_back` takes no slot |
| Two chains own one change | One change has one owning chain; a reopen goes to it, a new chain is started only for a fault outside its goal |
| A woken chain has lost its context | The Chair's wake carries the goal, the check, the reading and the MR; the check must make sense without the conversation |

## Consequences

- A chain's end is a verdict on the effect, not the owner's merge. The share of merged changes whose effect was confirmed, and
  the share reopened, are outcome figures that need no cost basis (ADR-0010).
- `columnOf`, the board snapshot, the wire contract and the web board learn one column; the pure function stays the only owner
  of where a chain stands.
- The goal, the judge's brief and the Chair's brief change; so does the chain's page (the check, the window left).
- Telemetry becomes a dependency of finishing a task, which makes the telemetry store's signal definitions part of the
  product, kept in the repository beside the event schemas.

## Revisit when

Most chains in `watching` finish unverified because no signal exists for the task; the owner finds the waiting noisy; or a
reopen by default produces chains too long to read and a new chain per repair serves better.

Enforced by: TestTheBoardLaysChainsOutByWhereTheyStand, TestAFinishedChainIsDoneWaitingOnYouOrNotDone, TestEveryColumnAndAttentionIsAChainTone, TestTheMergeOfAChainsMRIsAReadingItsOwnWatchHears, TestMarkLiveSaysWhoSaidItAndWakesTheWatchOfTheChainOnce, TestFinishChainIsRefusedWhileAWatchIsOpenAndTheOwnersFinishIsNot, TestAWatchThatIsNotMetByItsDeadlineWakesTheChairExpired, TestAWatchEndsWithItsChain, TestTheJudgeIsToldWhatTheChainWatchedAndWhatItFound, TestAJudgeCanBlameOnlyAWatchTheChainHeld
