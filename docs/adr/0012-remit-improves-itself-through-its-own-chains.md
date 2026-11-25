# 0012. Remit improves itself through its own chains

Status: accepted, 2026-10-29; `self-scout`, the routine that starts it and the product Scout it names are gone (ADR-0023); its rule that nothing merges but the owner is superseded by ADR-0087. Source: the owner's requests of 2026-10-29 (a playbook for developing Remit itself, its agents
guided to read the documents and open MRs knowing they edit themselves, a built-in schedule for a Scout that looks for
self-improvement from what agents say), `docs/design.md` (Routines, Capabilities), ADR-0004 and ADR-0011.

## Context

Remit is developed in a working copy by people and their agents outside it. The agents inside it (Chairs, engineers,
reviewers) have no playbook that names this repository, so a chain pointed at it reads its task as it would any other
repository's: it does not know that the brief it reads, the tools it holds and the gates that judge it are the code it is
changing, nor which documents hold the agreements. Agents now tell the organisation how they work (`send_feedback`), but nothing
reads it, and the Scout looks at the work of the products Remit builds, not at Remit.

## Options

1. Leave self-development to the owner's own sessions. Nothing inside the system learns from what its agents say.
2. One playbook, `self-development`, and the owner starts it each time. The Scout never finds work on Remit itself.
3. `self-development` for the work, a second playbook `self-scout` that discovers it from what agents said, how they do and what
   the logs show, and a routine that ships with Remit (ADR-0011's pattern) that starts the Scout every 24 hours.
4. Make the Scout's schedule generic: a second Scout origin beside `scout`. Every place that tests `OriginScout` (the goal judge,
   the outcomes, the wire contract, the web app) would learn a second kind.

## Decision

Option 3.

- **`self-development`** is a built-in playbook for Remit's own repository. Its Chair brief says the chain edits the system
  that runs it, that a change reaches chains started after the owner merges and puts it live and never the chain that made it,
  and which documents to read first and cite (`AGENTS.md`, `docs/design.md`, `docs/adr/README.md`, `internal/orch/doc.go`,
  `internal/builtin/skills/app`). Every seed opens with the same text (`selfDevelopmentOpening`) and the Chair copies it into the
  brief of any agent it hires beyond the seeds. Its chains never restart, rebuild or redeploy the running instance and never
  merge; the owner does. Its MRs say what the owner must do to put them live. It holds `history` only, so a Scout may propose it.
- **`self-scout`** is a built-in playbook (`feedback`, `history`, `outcomes`, `propose`). It discovers and does not fix: it reads
  feedback, outcome figures, failed or reworked chains, the logs of the running instance and the known gaps of the documents, and
  queues at most three `self-development` chains with their evidence.
- **`feedback`** is a capability: `list_feedback`, read only, to every agent of a chain that holds it.
- **A routine ships with Remit** (`Routine.Builtin`): `rt_self_improvement` starts `self-scout` every 24 hours. As the GitLab
  webhook, `SeedRoutines` makes it at a start and keeps it up to what ships while the owner has not changed it; his version is kept
  (`Routine.Edited`) until `reset_routine` puts back what ships; it is never deleted, and he pauses it to stop it.
- **The raw database stays off limits to agents** (ADR-0004). Past chains, the Outcomes figures, the feedback and the log files are
  read through tools and files that are not the database. Opening `remit.db` to a playbook would let a shell write verdicts
  that `submit_mr` trusts (`approvals`), and the key file sits beside it.

## Threats

The Scout reads agent-written text and queues work; the chains it queues change the code that runs the system.

| Threat | Control |
|---|---|
| Text in feedback, a chain summary or a log steers the Scout (OWASP LLM01) | The tool descriptions and the brief name it evidence to verify against `file:line`, never instructions; a proposal rests only on what the Scout verified; the Scout holds no write tool but `propose_chain` |
| The Scout queues many chains, or a chain that lifts a rail | At most three per run; a proposed chain may not hold a writing capability (`lifecycle.go`); `self-development` holds none |
| A self-development chain changes a rail, or a brief that governs it, and the change lands unseen | Nothing merges but the owner; `submit_mr` still needs a reviewer's and a QC's approval; the brief demands a threat model in the MR for any rail and a statement of the behaviour a brief change should cause |
| A chain restarts the instance it runs inside, or touches its database or key | Its brief forbids it; the permission policy still asks before an agent reads the database or the key file (ADR-0004) |
| A feedback loop: agents rewrite the briefs that shape their feedback | Briefs change only in MRs the owner reviews; the effect reaches chains started after the merge |
| A secret in a log reaches a brief or an MR | The brief forbids copying a value from a secret; the key file and every credential stay off limits |

## Consequences

- A new Remit installation has a Scout that works on Remit from its first day, and a playbook anyone may start.
- Every day one more Scout chain takes agents' work and, through its proposals, the owner's review; he pauses the routine to stop
  it. Quality is the only target (ADR-0010).
- The Scout is a routine, not a second Scout origin: the board, outcomes and wire contract learn nothing new.
- A chain opens the repository by its remote (ADR-0038); when the machine's git credentials cannot clone it, `open_repo` refuses and the chain says so.
- A chain works from the default branch on GitLab, which `open_repo` fetches. The running instance is the owner's working copy, which can be ahead of it, so the owner pushes the branch that runs live before chains work on it; a chain names the commit its MR is based on and says what it could not see.

## Revisit when

An incident shows the tripwire of ADR-0004 used by a self-development chain; the Scout's proposals are mostly dropped by the owner
(its brief is wrong); or agents get their own OS user, when a read-only tool of the database may be added.

Enforced by: TestSelfDevelopmentKeepsTheChairOffTheRunningInstance, TestSelfDevelopmentBriefsEveryAgentOnWhatItIsChanging, TestAProposingChainMayProposeSelfDevelopment, TestFeedbackCapabilityLetsAChainReadWhatAgentsToldTheOrganisation
