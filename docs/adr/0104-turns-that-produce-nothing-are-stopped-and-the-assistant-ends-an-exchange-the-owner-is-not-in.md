# 0104. Turns that produce nothing are stopped, and the Assistant ends an exchange the owner is not in

Status: accepted, 2026-11-13; extended to key results by ADR-0115. Extends ADR-0092 (a turn that repeats itself is stopped) and the liveness rails of
`docs/design.md` (*How a chain runs*). Row R5 of `docs/reports/Long running agent systems beliefs.md` (paperclip#390,
"too chatty", heartbeat cycles that change nothing; Vending-Bench runs that melt down over many turns).

## Context

Remit has three liveness rails, and each reads one thing:

- **The stall rail** (`schedule.rs` `unstick`) reads silence across the chain: nothing moved for 10 minutes, the Chair is
  reminded, 3 reminders later the chain is finished. Any card at work counts as movement.
- **The silent-turn rail** (`cut_silent_turns`) reads silence inside a turn: nothing heard for `-turn-idle`, the turn is cut.
- **The repetition rail** (ADR-0092, `repeats.rs`) reads one turn: the same call with the same result, or the same error,
  `-turn-repeats` times among its last 8 calls. Its state is reset at every turn's start (`session.rs` `begin_turn`).

None reads what turns produce across turns. A Chair that routes, a worker that reports, a Chair that sends it back, a
worker that reports again: every card is busy, every turn is different, no turn repeats a call, and nothing comes of it.
The stall rail never fires because a card is always at work; the repetition rail never fires because each turn starts
from nothing. The same error met once per turn, turn after turn, passes both.

The Assistant runs ADR-0092's repetition rail already: `on_tool_call` (`stream.rs`) takes no account of a card's origin,
`repeats_of` (`repeats.rs`) leaves out only a subagent's run, and a note reaches the Assistant's card through `send`'s
standalone path (`outbox.rs`). Across turns it has nothing: no chain holds it, so no stall rail either, and it takes a
turn on every reply to a message it sent (`assistant.rs` `replies_to_assistant`). An exchange with an agent can go on
for ever without the owner: it messages an agent, the reply opens its turn, it messages again; `message_agent` even
reopens a chain the stall rail finished.

## Options

1. **A cap on turns per chain.** Stops real work (a long chain takes hundreds of turns) and still lets a loop spend up to
   the cap. Rejected: quality is uncapped.
2. **A judge model reading the chain's last turns.** Catches loops a count misses, but a model call on a rail. Rejected
   for now, as in ADR-0092: the rails stay deterministic code.
3. **Read whether the worktree has new commits.** Remit records no commit, and a research chain makes none; reading git
   on every tick for every chain is I/O the rail does not need.
4. **Count turns since the chain last produced something, from what Remit records (chosen).** Every card counts its
   turns (`Card::turns`, stored); what a chain produces is recorded already, whatever kind of chain it is: a step of a
   plan completed (`Card::plan`), work accepted, a judge's verdict or pick, the goal checked, a stage merged or live, an
   insight told, a chain proposed, a release installed, a lesson shared (decisions). A code chain commits through
   accepted work and MRs whose gates give verdicts; a research chain closes plan steps and has its reports accepted. So
   one rule fits both, and nothing new is stored.
5. **For the Assistant, count the turns in a row it took on agents' replies (chosen).** The owner's words, an item put to
   it, anything not an agent's reply, break the run; the count is read from its stored messages, newest first.

## Decision

- **Chains** (`crates/orch/src/spin.rs`, called from `unstick`). A chain's tally is its cards' turns and what it
  produced (`tally`: completed plan steps plus decisions of the kinds in `PRODUCED`: accept, verdict, duel, goal, ship,
  live, insight, propose, release, learning). Hires, routes, reworks, asks and waits move work and are not counted. The
  rail keeps, in memory, the tally at the chain's last product (`Spin`); every `-spin-turns` turns (default 10; 0: off)
  with nothing produced call for a reminder, `Spin::saw` being the one decision, pure.
  - The reminder goes on the stall rail's own path (`Orchestrator::remind`): the same message shape, numbered of the same
    `MAX_NUDGES` (3), naming what Remit saw ("the agents of this chain took 20 turns and produced nothing") and what to
    look at; past the last, the chain is finished as stalled (`DecisionKind::Stall`, `Unjudged::Stalled`), so it counts
    where the stall rail's finishes count. It fires whether or not an agent is at work: that is the gap it fills.
  - Something produced starts the count again; a plan rewritten with fewer steps completed is neither progress nor a
    loss. A chain that stops running (a wait, a pause, a finish) is forgotten, so a wake starts the count from its own
    turns. The stall rail's spares hold: a chain waiting on the owner's answer, or with an agent waiting for a provider,
    is left alone, and nothing is reminded while no provider can run agents.
- **The Assistant** (`message_agent`, `assistant_tools.rs`). Before a message goes, Remit counts the turns in a row the
  Assistant took on agents' replies, the one in progress included (`Orchestrator::replies_in_a_row`). Every `-spin-turns`
  of them, the tool's answer carries a reminder (n of 3) that an exchange the owner is not in is a loop unless it moves
  his work; past the third, `message_agent` is refused until the owner writes or an item comes to it, and the refusal
  tells it to tell him what the exchange was for. Its other tools are not touched: only the one that feeds the loop is.

## What the owner sees

- **A chain**: in the Chair's stream, "Nudged for going round in circles", from Remit, beside "Nudged for standing
  still"; a chain finished by it reads "Remit finished this chain because the agents of this chain took N turns and
  produced nothing … after 3 nudges to the Chair", a `stall` decision, counted by Outcomes as the stall rail's are.
- **The Assistant**: the reminder in the answer of its `message_agent` call, in its own stream; after the third, the
  refused call and its answer to him saying where the exchange stands. Nothing goes to his inbox (ADR-0087).

## Consequences

- A chain that produces nothing is finished after 4 × `-spin-turns` turns (40 by default) of producing nothing, however
  busy its cards are; a chain that keeps closing plan steps or having work accepted is never stopped by it.
- An agent that never writes a plan and whose reports are never accepted produces nothing the rail can see: its Chair
  is reminded. That is the protocol's own rule (every agent keeps its plan true, `plan.rs`), not a new one.
- The same error met once per turn, turn after turn, is a run of turns that produce nothing: this rail takes it; the
  same error within a turn stays ADR-0092's.
- The marks live in memory, as the stall rail's reminders do: a restart starts every count again, which delays the rail
  and never stops a chain early.
- One flag (`-spin-turns`), no migration, no contract change.

## Revisit when

- **Turns are the wrong unit.** A chain whose turns are long and few, or short and many, would want the count weighed by
  what a turn spends; the seam is `tally`, the one place a chain's work is measured.
- **A product Remit does not record.** A chain whose output is neither a plan step nor a decision (a file written for
  the owner, a message sent outside) is reminded falsely; the fix is to record that product as a decision kind and add
  it to `PRODUCED`, not to spare the chain.
- **Loops that look productive.** A chain that ticks plan steps without doing them passes; a judge asked only when the
  count nears a reminder (the seam: `Spin::saw` returns `Due`) would read the turns themselves.
- **The Assistant loops another way.** It takes turns only on the owner's words, items and agents' replies today; a new
  source of its turns that an agent can trigger needs the same count.
