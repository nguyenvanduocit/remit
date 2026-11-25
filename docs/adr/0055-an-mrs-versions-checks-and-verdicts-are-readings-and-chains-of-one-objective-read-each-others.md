# 0055. An MR's versions, checks and verdicts are readings, and chains of one objective read each other's

Status: accepted, 2026-11-04; its MR readings become a landing's, `chain:<id>.landing.<repository>` (ADR-0114). Builds on ADR-0013 (a chain owns its outcome until the effect is verified), ADR-0014 (the signal is the
backbone), ADR-0031 (an objective is a set of key results the owner approves) and ADR-0049 (every sense is a sensor; signals are one
log). Source: the human-employees design (`docs/superpowers/specs/2026-11-04-employee-runtime-design.md`, decisions 10, 11 and 14):
people and AIs build on each other's merge requests, so the lead must see an MR move as it moves, and so must the chains that work
beside it.

## Context

`chain:<id>.mr.<repository>` said only an MR's state (`iid`, `url`, `state`, `decidedAt`), written by `fireRecordMR` when the forge
said it was merged or closed. `submit_mr` wrote none, so a watch on a fresh MR heard nothing until it was decided. What a lead plans
on is what happens in between: a new version is up, its checks pass or fail, a reviewer or QC rules on it. Those lived in three
places (the chain's record, the forge's pipeline, the approvals table), none of them a signal, so no condition could join them and
no watch could wake on them.

A chain read and watched only its own records (`readableBy`). When the owner puts two chains to one objective (a backend and the app
that calls it), the one that builds on the other's MR could not see it move.

## Options

What moves:

1. A signal per field (`.version`, `.checks`, `.verdicts`). A condition that needs two of them ("version 2 passed its checks and was
   approved") cannot be written over one reading.
2. A forge event the Chair reads. It carries what the forge knows, not the verdicts, and is the forge's shape, not Remit's.
3. One reading carrying the whole standing (state, head, version, checks, verdicts, who made the version), written whenever any of
   them moves, with the cause of the move. **Chosen:** one condition reads version, checks and verdicts together, and the cause says
   who moved it.

Who may read another chain's MR:

1. Any chain. A chain would read work it has nothing to do with.
2. A pair the owner grants. A form or a tool for each pair, for a grouping the owner already made.
3. Chains that serve one objective. **Chosen:** it is the owner's own grouping (ADR-0031), and a chain that serves none
   (`o_<chainID>`) reads only its own.

## Decision

Option 3, both times.

- **The reading** (`mrValue`, `internal/orch/live.go`) is `iid`, `url`, `state`, `decidedAt`, `head`, `version`, `checks`, `verdicts`
  (by role, the latest verdict on the change at `head`: `{}` when none) and `by` (the employees whose accepted work makes the version:
  `[]` when none). `mrValueOf` builds it from the MR record and the approvals; `mrMoved` is the one rule for when one is written: when
  the value differs from the one before, whoever moved it.
- **Version** rises by one when a submit pushes a head the MR does not stand at (`MRRef.Pushed`); a new MR starts at 1; the same head
  again writes nothing. The MR records the patch id of the change at its head (`MRRef.Change`), which is what verdicts are keyed by.
- **Checks** at `head`: the forge's while the MR is open and the forge runs some; otherwise Remit's own, `passed` at a head it
  pushed (it pushes only heads whose checks passed) and `pending` at any other. A new version starts `pending`; the next sync settles
  it; a merged or closed MR keeps its last checks (`mrStandingOf`, `merge_request_sense.go`).
- **Causes.** A version is the Chair's (`ByAgent(chairID)`, with the chain), a verdict its card's (`actorOf`, with the chain), the
  forge's answer `ByRemit("mr-sync")` with no chain. A chain's own watches hear the forge and not its own acts; a peer's hears all.
- **Reading across chains.** A chain reads and watches another chain's `mr.<repository>` when both `Serves.Objective` are one id
  (`readableBy`, `servesOneObjective`); another chain's other records stay its own. Every agent holding `read_signal` reads them, only
  the Chair watches, and `list_signals` lists the peers' MRs (`peersOf`).
- **A person's session** reads none of this directly: the lead's message to it carries, ahead of its text, the MR readings that moved
  since the lead's last message (`withMRDelta`, `mr_delta.go`), naming no reviewer, author or chain.

## Consequences

- A Chair can watch a peer's MR (`value.version >= 2 && value.checks.state == "passed"`) and route the version to whoever builds on it.
- On GitHub every sync of an open MR makes one more forge call (the checks at its head).
- A new version reads `pending` until the next sync, at most `mrSyncEvery` (3 minutes) later.
- Stored MRs migrate once (migration 80): each is its first version with its checks to be read.

## Revisit when

- **Checks should settle at once:** a pipeline event (GitLab's Pipeline Hook, `chain_jobs.go`) writes the sense's reading, so the
  record follows without waiting for the sync.
- **A chain should read a peer's other records** (its `live`, its stage): widen `readableBy` for that key, with its own reason.
- **The forge stands at a head the chain did not push** (someone pushed to the branch on the forge): `Head` moves while the version
  and its change stay, so the reading lists that version's verdicts beside a head they were not given on. List verdicts only while
  `Head == PushedSHA`.
- **Hardening is asked for:** a cap on how many peers' readings a chain may watch, and a way for a chain to opt out of being read.

Not enforced yet: the tests that will hold it are named in `docs/superpowers/plans/2026-11-03-human-employees/05-mr-signal.md`,
Tasks 3-7 and 9, and are written when this slice is hardened; this line then becomes their `Enforced by:`.