# 0080. The owner's next step and every send are his words to an agent

Status: accepted, 2026-11-11; an MR's next steps go with the MR (ADR-0114); its next steps on a merge (`NextStep`, `wait_step`) superseded by ADR-0087, which leaves no merge
waiting on the owner. Source: the owner's agreement of 2026-11-11 to `docs/research-peers-product/report.md`
(proposals 2 and 3) and its deck (slides "A2", "A3").

## Context

The customer is the owner deciding about a chain's result and steering its agents. Read in code (HIGH unless marked):

- The board asks him to "merge MR !12, !13" (`owners_action`, `crates/orch/src/standing.rs:420-438`) and the only control is
  the forge link "Open !12" (report remit-15): the decision leaves Remit. A chain holding `ship` can merge its own MR in canary
  stages through `merge_mr` (ADR-0050, `crates/orch/src/ship.rs:113`); a chain without it asks for the capability with
  `request_capability`, which the owner's Apply decides (`crates/orch/src/capability_request.rs:4`).
- An MR's checks (`model::MRChecks`, `crates/model/src/mr_checks.rs:19-24`: state and the failed check's name) and its review
  threads (`model::ReviewThread`, `resolved`, `outdated`, `repo`) are stored. Whether the forge sees a conflict is not.
- The owner's message reaches an agent through one path, `owner_says` (`crates/orch/src/session.rs:1104-1125`), recorded as his.
- A permission answer carries a reason to the session already: `inbox_answer(item_id, option_id, text)`
  (`crates/wire/src/app.rs`, contract) reaches `answer_permission_request(by, request_id, allow, why)`
  (`crates/orch/src/permission.rs:568-600`). The page sends `text: ''` (`web/src/components/InboxCallout.vue:27`), so a refusal
  never has one.
- Claude Code takes input in the middle of a turn: `Session::steer` puts text into the running turn with priority `now`
  (`crates/agent/src/claude.rs:378-403`), and the orchestrator steers a queued message into a busy card for senders that need it
  read now (`crates/orch/src/outbox.rs:377-420`, `apply.rs:333`). The owner is offered only Send (after the turn) and Send now,
  which interrupts the turn (`card_prompt(draft_id, interrupt)`, `session.rs:1083-1125`; `CardComposer.vue:43,82`).
- A message to a card of a paused chain is stored and not taken until the chain runs again (`outbox.rs:483,515,567`); the
  composer still reads "Talk to the Chair…" (report remit-06). Pause acts at once with no confirmation (`ChainActions.vue`).

## Options

1. **Owner actions that act**: a Merge button that calls the forge from the browser's RPC. The browser would edit the domain,
   past the Chair, the judges and the canary stages (AGENTS.md, *Agent-centric*; ADR-0050).
2. **Buttons that fill the composer** with suggested words for the owner to send. One more step, and the words are the page's,
   not the server's.
3. **Next steps the server lists, each a message the server words and sends as the owner's to the Chair** (Claude Projects'
   "Merge it", "Fix CI", "Address comments"). The agent stays the one who acts.

For sends: keep Send and Send now; or name the three things a message can do while a turn runs (steer into it, queue after it,
interrupt it), now that the harness is shown to steer.

## Decision

Option 3, and three named sends. Patterns: command message (the button is a typed command the server turns into words),
server-driven UI (the server lists what may be pressed, as it lists `Ask` today). Mental models: second-order thinking (a
merge button in the browser is a second merge path beside `merge_mr`, which every later rail must guard twice), inversion
(what guarantees the owner's decision leaves Remit: no control for it in Remit).

### Next steps

- Each `merge` wait (ADR-0078) carries `steps: Vec<NextStep>`, computed in `waiting.rs` from the MR, its checks and its
  threads:
  - `address_comments` when the MR has unresolved, current threads: "Address 2 comments" / says "Address the 2 open review
    comments on !12, reply on each thread, and push a new version.";
  - `fix_checks` when its checks failed: "Fix CI" / says "The checks on !12 fail ({failed check}). Fix them and push a new
    version.";
  - `merge`, always: "Merge it" / says "Merge !12." The Chair merges through `merge_mr` when the chain holds `ship`; without it,
    the Chair asks for `ship` with `request_capability`, citing the owner's message, and the owner (or the Assistant for him)
    applies it. The Chair's brief says so (`crates/orch/src/prompts.rs`).
- `wait_step(chain_id, wait_id, step)` (a change, ordered by the chain): the server reads the waiting list again, refuses a wait or step
  it no longer offers ("the MR moved on: look again"), and sends the step's words to the chain's Chair with `owner_says`, by the
  owner, queued as any message of his. It answers the message's event. The browser holds no words of its own.
- A `live` wait offers no step: putting a change live stays where it is today.

### Sends

- `card_prompt(draft_id, delivery)` and `assistant_prompt(draft_id, where, delivery)` take `Delivery { queue, steer, interrupt }`
  in place of `interrupt: bool`. `queue`: read after the turn in progress (or now when none runs); `steer`: put into the turn
  in progress with `Session::steer`, falling back to `queue` when the turn ended meanwhile (what `outbox.rs` `steer` does);
  `interrupt`: the turn ends and the message is read next (today's Send now).
- `AgentStanding.asks` gains `steer` when the card is in a turn and its session takes input (not a card spawned inside its
  parent's process, which takes no message of its own). The composer offers Send (queue) and, during a turn, Steer and
  Interrupt.
- `AgentStanding.on_send: Option<String>`: what a message sent now does, in the server's words, when it is not "the agent
  reads it now": "Waits for the current turn (+2 queued)", "The chain is paused: the Chair reads it first on Resume", "Wakes the
  Chair now". The composer shows it under the box.
- A queued message can be taken back before an agent reads it: `card_prompt_withdraw(event_id) -> Draft` returns its words to
  the card's message draft and deletes the queued prompt; refused once it is taken or steered, and refused while the card's
  draft holds unsent words (so nothing he typed is replaced).

### Refusals and pausing

- A permission item's answer carries the owner's text: typing in the callout turns Refuse into "Refuse with this reason", sent
  as `inbox_answer(item, refuse option, text)`; the session reads "Refused by you. <text>" (`decision_of`,
  `crates/orch/src/permission.rs:192-202`). No contract change.
- Pausing a chain asks first, saying what stops ("3 agents stop after their current step; nothing is lost"), from the chain's
  standing (its agents at work). A paused chain's header and composer say who paused it and when, from its `pause` decision.

## Threats

`wait_step` and `card_prompt_withdraw` are changes on the owner's channel, behind the owner's sign-in like every call there.

| Threat | Control |
|---|---|
| A step replayed after a reconnect sends the message twice | The request id makes the call idempotent (ADR-0072); a second call with the same id answers the first's event |
| A stale page presses a step the MR no longer needs ("Fix CI" after the checks went green) | The server reads the list again and refuses a step it no longer offers |
| A next step bypasses the merge rail | It only sends words; the merge is `merge_mr`'s, under ADR-0050's stages and the `ship` capability |

## Consequences

- The merge decision is made in Remit and recorded as the owner's message; the Chair, the judges and the canary stages still
  stand between it and the merge.
- A chain without `ship` that is told to merge raises one change item; the Assistant may apply it for him.
- `interrupt: bool` leaves the contract; every caller of `card_prompt` and `assistant_prompt` moves to `Delivery`.
- Steering is offered to the owner as it already is to other senders.

## Tests (written first)

- orch: `a_next_step_is_the_owners_message_to_the_chair`: `wait_step` on a merge wait sends "Merge !12." to the Chair by the owner;
  the same step after the MR merged is refused.
- orch: `a_withdrawn_queued_message_returns_to_the_draft`.

## Revisit when

- The forge reports mergeability: a `resolve_conflicts` step.
- The owner asks to reorder queued messages: an order field on queued prompts.
- Hardening the owner's channel (rate of steps per minute, a second confirmation for merge): threat model when asked.
