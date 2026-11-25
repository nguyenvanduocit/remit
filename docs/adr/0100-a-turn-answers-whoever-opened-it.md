# 0100. A turn answers whoever opened it, and a worker's turn does not end on a plan it did not keep

Status: accepted, 2026-11-13. Amends ADR-0062 (a mentor's messages steer the card; its report stays its AI's) and the
worker brief's report rule (`prompts.rs` `WORKER_REPORT`). Replaces the plan-sync turn.

## Context

A worker's final message in each turn was its report to the Chair, whoever opened the turn. That is right for a bot
whose only reader is the Chair, and wrong once a person talks to it. In the dev database 24 prompts came from the owner
straight to a worker, and a mentor's card (`c_1ddc092685cb`) answered every one of Lan Nguyen's messages with one text
that spoke to two readers: "To Lan Nguyen: … / To the Chair: waiting: Lan Nguyen, reason…". The mentor read the Chair's
half in the session; the Chair got a "report" for each line of a conversation.

Second, a worker that ended a turn with a plan step not completed had its report held back and was sent a Remit message,
"Update the plan" (`plan_sync_message`), which opened a whole extra turn; the report went on when that turn ended. The
rule could not tell a stale plan from a step that is honestly still open (waiting for a mentor), so it fired after every
turn: 30 of 530 prompts in the dev database, each a turn of its own, a row in the stream ("Show 4 steps of this turn"),
and a line the mentor saw as the agent talking to itself ("Plan is true: …").

## Options

1. **Keep the plan-sync turn, have it write nothing, and hide it.** Removes the visible line; keeps the extra turn, the
   split stream and the nag on an honest open step. Rejected.
2. **An explicit report tool for every worker, the final message never a report.** One mechanism, but every plain bot
   pays a tool call per turn, and a turn that forgets it leaves the Chair waiting. Rejected.
3. **A turn answers whoever opened it; a report tool for turns a person opened; the plan kept true inside the turn by
   Claude Code's Stop hook.** Plain bots are unchanged. Chosen.

## Decision

1. **A turn answers whoever opened it** (`turn_end.rs` `answers_a_person`). A turn the Chair, or Remit for the Chair,
   opened (the brief, a handoff, background news) ends in the worker's report, as before. A turn the owner, the
   Assistant on his words or a mentor opened, or Remit's word that a mentor joined (`help.rs` `mentor_joined_by`), ends
   in the worker's reply to that person alone: it stays in the session they read (the Assistant is sent it), the card's
   last report stays as it was, and nothing goes to the Chair.
2. **`report`** (`tools.rs` `add_report_tool`, `turn_end.rs` `report_now`), given to every worker: in a turn a person
   opened, it sends a report to the Chair now, on the same path as a turn's report (`send_report`: a run's step, a duel,
   review). In a turn the Chair opened it is refused, since the final message is the report there. The brief and the
   mentor's joining message say to report when the work moves: a result, a block, a new direction.
3. **A worker's turn does not end on a plan it did not keep, nor a person's turn that completed the work without a
   report** (`turn_end.rs` `owed`, `owed_before_end`). Claude Code's `Stop` hook (`crates/agent` `options.rs`, `claude.rs`
   `stopping`) asks the handler's `on_stop`; through the AI runtime it is the `to_core` read `session_stopping`
   (`wire::runtime`, additive, the contract stays 2). The answer blocks the stop once (`stop_hook_active`), and the agent
   goes on in the same turn: it calls update_plan (and report) and ends without writing more, so its last message stands.
   It asks when the plan has a step not completed and update_plan was not called in the turn; and, in a turn a person
   opened, when the plan was completed in it and nothing was reported. The Chair's, the Assistant's and a judge's cards,
   a card being handed over, and the handover and duel turns owe nothing. A failed call lets the turn end.
4. Deleted: the plan-sync message and turn (`plan_sync_by`, `plan_sync_message`, `PRIORITY_PLAN_SYNC`), the report held
   on it (`HeldReport` is `WorkerReport`), and the FYI the Chair got when the owner talked to a worker.

## Consequences

- One turn is one row in the stream; a person reads only what is meant for them; the Chair reads fewer reports, each one
  a move of the work.
- The Chair no longer hears each exchange with a person: it hears what the worker reports. The Stop hook's second ask is
  the net for a person's turn that finished the work; the session stays readable when the Chair needs more.
- A worker's plan is asked for once per turn that leaves a step open without touching the plan: one step inside the turn,
  never a turn of its own.
- A plan-sync message still queued when this ships is delivered as a Remit message; its turn's end goes to the Chair as a
  report, and the report it held is not sent. Bounded: such a message waits seconds (it was read before every other).

## Revisit when

- An agent ends a person's turn with work moved and no report, and the plan did not show it: a judge on the turn's end
  could ask instead of the plan rule.
- A channel other than Claude Code runs a worker: its harness needs its own way to keep the turn going (`on_stop`).
