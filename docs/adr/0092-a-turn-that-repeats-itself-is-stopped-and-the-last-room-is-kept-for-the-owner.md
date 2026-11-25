# 0092. A turn that repeats itself is stopped, and the last room on the accounts is kept for the owner

Status: accepted, 2026-11-13; its headroom is one of the pressures ADR-0115's limit reads. Extends ADR-0005 (the provider's outages hold what Remit starts on its own) and the
liveness rails of `docs/design.md` (*How a chain runs*: the stall reminder, the silent turn). Ideas 8 and 9 of
`docs/research-agentes-chat/report.md`; the reasoning is agentes.chat's ADR 0015 (addendum "runs that go in circles
stop"), 0118 (long work pauses near a plan's limit) and 0131 (autonomy brakes), read for their ideas only.

## Context

**A loop that keeps talking.** Remit's two liveness rails both read silence. A chain where nothing has moved for 10
minutes has its Chair reminded, and is finished after 3 reminders; a turn that sent nothing for `-turn-idle` (30 minutes)
is cut. An agent that runs the same failing command, re-reads the same page or polls the same status for an hour is
heard from at every call, so neither rail fires, and the Chair cannot see inside the turn until it ends. agentes.chat met
the same class: a fixed cap of 25 steps stopped real work and still let a loop spend 25 calls; they raised the cap to a
backstop of 200 and caught loops by structure instead (fingerprints of each step's calls and results, numbers masked, the
last 8 kept; once a note, twice an end with a report).

**The last room on a plan.** Remit reads each account's limit windows (`agent::Limit`), marks an account `Near` at 90%
of a window the provider warned of (`health.rs` `Reading::near`) and puts new work on an account with room first
(`accounts.rs` `Standing`). It holds what the scheduler starts on its own only once no account can run agents at all
(`admit.rs` `Room::no_agents`). So routines, wakes and proposed chains run every account up to its limit, and the
Assistant, which answers the owner, then has nothing left: on a weekly window, for days. agentes.chat's 0118 pauses long
work at 95% of any window so quick replies to people still run.

## Options

1. **A count cap per turn** (tool calls, minutes). Stops real work (a large refactor makes hundreds of different calls)
   and still lets a loop run to the cap. Rejected, as agentes.chat did.
2. **A judge model on every turn's tail.** Catches loops a fingerprint misses (the same idea in other words), but it is
   a model call per check, and a judgement on a rail. Rejected for now: the rail stays deterministic code
   (`AGENTS.md`, the rails that protect the owner).
3. **A repetition fingerprint, deterministic (chosen).** Each finished tool call is printed with what varies masked; the
   same call with the same result, or the same error, `-turn-repeats` times among the last 8 is a sign.
4. **Headroom by a percentage of each window** (agentes.chat's 95% pause). Remit has one owner, no money metering
   (ADR-0010) and the `Near` reading already; a second threshold would be a second owner of "near". Rejected.
5. **Headroom as one more admission need (chosen).** While every account that can run agents is `Near`, what the
   scheduler starts on its own waits, exactly where and as it waits while the provider is out.

## Decision

- **Repetition rail** (`crates/orch/src/repeats.rs`). Every finished tool call of a run's turn (completed or failed,
  each call id once) is printed from its tool, its input and its result, with every word that holds a digit masked and
  spacing collapsed, so times, ids, counters and sizes do not make two calls differ; a failed call also prints its
  error alone. The last 8 are kept on the run (`RunState::repeats`), from nothing at every turn's start (`begin_turn`).
  A sign is the same call with the same result, or the same error, `-turn-repeats` times (default 3; 0: off) among them.
  `Repeats::saw` is the one decision, pure; after a sign only newer calls count.
  - **First sign: a note.** Remit steers a message into the turn (`deliver_now`, read between the agent's steps):
    "you ran X n times, with the same result; try another way, or say plainly what blocks you".
  - **Second sign: end with a report.** A second steered message tells the agent to make no more calls and end the
    turn with its report: what it did, what it tried, what is left. The report goes where every report goes (the Chair,
    or the owner for the Assistant's own turn).
  - **Calls past it: the turn ends.** The third call after that note cuts the turn (`Cut::Repeating`); it ends as a
    cancelled turn whose words so far are its report, and its open tool calls say why.
  - A subagent's calls are its parent's turn: the rail holds the session's own run only.
- **Headroom** (`Health::short_of_room`, `Orchestrator::no_headroom`). While some account can run agents and every one
  that can is near a limit the provider warned of, admission refuses `Needs::headroom`: a queued start the owner did not
  ask for (himself, or the Assistant on his words), a scheduled wake, a workflow wait's firing, and a routine's or an
  event's start firing. The owner's own actions and the Assistant's turns take no such entry and go on, and so do chains
  already at work. An intake does not need it (one short turn; its chain still waits to start). It lifts as soon as one
  account has room again: a turn's limit reading, or a limit that resets.

## What the owner sees

- **A repeat**: in the agent's own stream, a message from Remit, "You are repeating yourself", steered into the turn,
  and after a second, "End the turn with your report". A turn the rail ended stops `cancelled`, its open calls saying
  "Remit ended the turn: the agent kept repeating the same calls", and its report reaches the Chair as every report
  does. Nothing goes to the owner's inbox: no agent waits on him (ADR-0087).
- **Headroom kept**: the Health bar already names each account's near limit and how much is used. A routine whose run
  is due, and a waiting chain whose wake is due, show on the Subscriptions tab as held, with the reason ("every account
  is near its limit (a@b: Weekly limit 93% used): the room left is kept for the owner's and the Assistant's own turns,
  and this starts once an account has room again"), counted on the tab as every held line is; the idle watchdog names
  the same reason.

## Consequences

- A loop is stopped within a few calls of its third repeat, whatever the turn's length; a long turn of varied work is
  never stopped by it.
- A poll that gets the same answer (CI still pending) is a repeat: the agent is told to stop polling. Remit's way to wait
  is `subscribe`, which frees the slot; the note says so in effect.
- With every account near a limit, routines and wakes wait, possibly for days on a weekly window; the owner and the
  Assistant keep working, and either can start what waits by asking for it.
- One flag (`-turn-repeats`), no migration, no contract change.

## Revisit when

- **Models stop looping.** The rail is one file and two call sites (`stream.rs` `on_tool_call`, `session.rs`
  `begin_turn`); `-turn-repeats 0` turns it off, and deleting `repeats.rs` removes it.
- **Repeats in other words.** A loop of different calls with the same effect (three ways to read one missing file) is
  not caught. The seam: `Repeats::saw` returns a sign; a judge asked only on a near-sign, as agentes.chat does, would
  slot in before the note.
- **Steering is refused.** A note that cannot be steered (the turn ended, the agent cannot take it) waits as a queued
  message and is read in the next turn.
- **Headroom by window.** A finer rule (keep room per window, a weekly one weighing more than a 5-hour one; a running
  chain's next turn held too) belongs in `Health::short_of_room` and admission's `Needs`, not beside them.
