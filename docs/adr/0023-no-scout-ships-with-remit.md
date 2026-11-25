# 0023. No Scout and no routine ship with Remit: the owner sets up what looks for work

Status: accepted, 2026-11-01; amended the same day on the owner's word: the self-improvement routine and its playbook `self-scout` go
too, so no routine ships. Source: the owner's requests of 2026-11-01 (the Scout is built in; it is not needed, so remove it and
let each user set up what they want; then the self-improvement routine as well), `AGENTS.md` (Agent-centric), ADR-0011, ADR-0012, ADR-0014, ADR-0016.

## Context

A Scout shipped with Remit: the routine `rt_scout`, seeded from `-scout-every`, started the built-in playbook `scout` every hour.
That playbook's brief was one team's: Jira's DEMO bugs, the vault's PRD and task folders, a Claude transcript path, a repository
catalog. Beside it the code held a chain origin `scout` that rules branched on (the goal judge and the acceptance and plan checks
skipped it, the board archived its runs, the outcomes and the overlap note left it out), a browser call `scoutRun` with a "Find work now"
button, an Assistant tool `run_scout` and a subscription kind of its own.

Which work is worth finding, where it is read from and how often to look are the owner's choices, and the owner makes such
choices by telling an agent (`AGENTS.md`, Agent-centric). A shipped Scout made them for every installation, and its origin made
the code treat one routine's chains differently from every other routine's.

## Options

1. Keep the Scout shipped. Every installation looks for one team's kind of work, on a clock the operator sets with a flag.
2. Ship it paused (`-scout-every 0`). The playbook, the routine, the origin and the three branches stay, and the owner still has
   to learn what a Scout is before setting up his own.
3. Remove it. What looks for work is a routine and a playbook the owner sets up by telling the Assistant, from the capabilities
   that stay (`propose`, `history`, `issues`, `customers`, `observe`).

## Decision

Option 3.

- The routine `rt_scout`, the playbook `scout`, the origin `scout`, the subscription kind `scout`, `-scout-every`, the call
  `scoutRun`, the button "Find work now" and the Assistant's `run_scout` are gone. A routine that looks for work is a routine like
  any other: its chains are of origin `routine`, they are judged, accepted and kept on the board like any routine's.
- `propose_chain` and the capabilities `propose`, `issues`, `customers` and `observe` stay: they are what the owner's playbook lists.
  The self-improvement routine (ADR-0012, `rt_self_improvement`) and its playbook `self-scout` are gone too. `self-development`, the
  playbook the owner starts for Remit's own repository, stays. With no routine left that ships, the machinery that kept
  a shipped routine up to date (`SeedRoutines`, `reset_routine`, the refusal to delete one) goes too.
- Stored data moves forward once (migrations 50 and 51): chains of origin `scout` become chains of origin `routine`, keeping the
  routine they name, and `rt_scout` and `rt_self_improvement` are deleted. One the owner changed is theirs: it stays, no longer built
  in. Their firings stay.
  `SeedPlaybooks` retires the playbooks `scout` and `self-scout` as it retires any built-in playbook no version ships, unless the owner edited it.

## Consequences

- A new installation starts no work by itself. The owner asks the Assistant for a routine and the Playbook author for a playbook.
- The rule that a goal a Scout proposes opens with a CUSTOMER section (ADR-0016) went with the Scout's brief. The owner's playbook
  carries it if they want it; the Chair, the gates and the judges still apply the lens to every chain.
- Every branch on the origin `scout` is deleted (goal, step, prompts, outcomes), and the board, the outcomes and the wire contract
  learn one origin fewer.
- A launcher that still passes `-scout-every` fails to start: the flag is unknown. It is removed from the launcher.

## Revisit when

Owners of most installations ask the Assistant for the same work-finding routine, which would make a template of it worth shipping
as a library item the owner chooses, not as a routine that starts by itself.

Enforced by: TestMigration50RetiresTheScout, TestMigration51RetiresTheSelfImprovementRoutine, TestEveryOriginOfAChainIsNamed
