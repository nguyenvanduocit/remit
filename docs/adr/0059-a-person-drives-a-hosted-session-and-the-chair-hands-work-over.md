# 0059. A person drives a hosted session; the Chair hands work over; every permission prompt goes up one ladder

Status: accepted, 2026-11-04; the card shapes and hand-over to a person superseded by 0062; *Permission prompts* 3 amended
2026-11-18 (a session with no Chair above it is refused at the limit, not its own decider; whom a prompt waits on is
stored on its card). Extends ADR-0051 (a person's work card runs a hosted session, whose turn end is no report) and
ADR-0056 (the session the Work plugin's tab will show). Source: the owner's decisions of 2026-11-04 in
`docs/superpowers/specs/2026-11-04-employee-runtime-design.md` (*A person's card*, *Permission prompts*, *Takeover and hand back*,
*Decision records*), and ADR-0058, which runs every Claude Code session in the AI runtime.

## Context

ADR-0051 gave a person's card a launcher of its own: no session, only the person's reply as the report. ADR-0058 moved every
Claude Code session into the AI runtime behind `agent.Launcher`, so a person can drive one of those sessions as an AI does, in the
worktree the Chair gave, from their own device. Three questions follow. Which cards run a session and whose turn end is the report?
How does a card's work move from an AI employee that falls short to a person, and back? And who decides a session's permission
prompt, now that a person's session raises them and the owner should not be every session's only gate?

## Options

1. **Keep a person's card sessionless** (ADR-0051 alone): the person works in their own tools and replies in words. Nothing of their
   work is recorded on our side, and an AI's half-done work cannot pass to them in the same worktree.
2. **A flag on the card for "a person drives it"**, read beside `isPerson` wherever a rule cares. Every rule grows a second check, and
   a card's shape could disagree with its employee.
3. **A card's shape, read once (chosen).** A card is an agent's, a person's question (a decision card, ADR-0051's path) or a person's
   session; the shape is read from the employee's kind and the card's offer when its run opens and decides its launcher and whether a
   turn's end is the report.

## Decision

- **Three shapes** (`cardShape`, `person.go`): `agent`, `decision` (a person's question: `PersonLauncher`, their words are the report)
  and `session` (a person's work card: a Claude Code session in the AI runtime, in the worktree the Chair gave, holding a plain
  agent's loadout narrowed by the Chair). `shapeOf` is the one owner; `run.shape` replaces `run.person`.
- **A session card's turn end is nothing** (`endTurn`): the person talks with their session for as many turns as they like. The report
  is the session's `hand_in` tool, which reaches the Chair as an AI's turn end does; `ask_lead` is its question. The session lives as
  long as the card's work: it stays open after `hand_in` and through a send-back, and closes when the work ends (`Offer.Ended`,
  ADR-0051: the lead accepts, stops or hands over the card, or the chain finishes): the person's own prompts no longer open it, and
  the lead's next message does. A card handed over takes no
  message at all (`reachable`): a session there would work beside the new card in the same worktree.
- **A held session is work in progress** (`sessionHeld`): while the person holds it open and has not handed in, the chain waits on
  them and the stall rail leaves it alone, whatever its turn state. A restart recovers it as an AI's session (resumed when cut
  mid-turn); only a person's question keeps ADR-0051's line.
- **It opens when the person accepts** (`openLocked`): the brief stays queued until `AcceptOffer` dispatches it; a decline drops it and
  reaches the Chair as the report, with nothing to accept (`stopDeclined`): the Chair gives the work to someone else or asks the owner.
- **A person reads it as theirs.** The brief is `personBrief`: their task, the deliverable, the criteria, the inputs and a handover
  note, naming no chain, Chair, owner or product. It speaks to the session about the person, who reads it too: the session works on
  what the person asks and nothing more, its first turn says how it would go about the task and waits, and it calls `hand_in` only
  when the person says the work is ready. The lead's messages are headed `[from the lead]` (`personRead`); the person's own
  prompts, steers and cancels are `ByWorker(card)` acts (`WorkerSessionPrompt`, `WorkerSessionSteer`, `WorkerSessionCancel`), and
  what they read of the session is `SessionView` (paths relative to the worktree, deciders named "the lead", "you", "the session",
  "a rule"). Every tool act of a session card is `ByWorker(card)` (`actorOf`).

## Permission prompts

Every session's prompt goes up one ladder (`ladderOf`, `permission.go`):

1. **Refused by code first.** A deny rule, then a prompt that names a secret (`refusesSecret`: Remit's own keys and database, a
   secret path wherever it stands in a command, such as `.env` in `cat .env | grep KEY`, `~/.aws/credentials`, a key) is refused
   before anyone is asked, and nobody may allow one. `secretPath` matches paths one at a time (`pathsIn`), never a bare word, so
   `grep token` and `auth/token.go` go up the ladder.
2. **The Chair** decides a team member's prompt (`answer_permission`) or passes it to the owner (`escalate_permission`). A session with
   no Chair above it (the Chair's own, a judge's, the Assistant's) asks the owner first.
3. **The driver decides at the time limit.** The limit runs from the ask, whatever step the prompt is at: the owner's setting
   (`Settings.PermissionWait`, 30 minutes until he tells the Assistant another, `set_permission_wait`). When it passes, the Chair
   decides an AI employee's prompt alone ("is now yours to decide"; it can no longer pass it up). From the limit on, the prompt is
   in the owner's inbox too, so he can always break a tie the driver leaves.

   A session with no Chair above it (the Chair's own, a judge's, the Assistant's) has no driver: **the rule refuses its prompt at
   the limit** (`by_remit("permission wait")`), and its inbox item closes saying a rule decided it. Amended 2026-11-18, from "a
   session with no Chair decides its own": that let the asker decide its own request, against AGENTS.md's "the asker never
   decides" (ADR-0087). Options weighed:
   - *The session allows its own* (as built until then): an agent that asks for something nobody approved gets it by waiting.
   - *Wait without end*: parks the session on the owner, which ADR-0087 calls a defect.
   - *Hand it to the Assistant at the limit*: it had the prompt from the ask (the owner's inbox puts every item to it), so
     handing it again changes nothing, and for the Assistant's own prompt it would be the asker deciding again.
   - *The rule refuses it* (chosen): deterministic, as a rail is; the session reads why ("nobody decided it in N minutes, and a
     session never decides its own request") and goes on without it or asks again. Its strongest counter: a judge whose check
     needs the command loses it; but the Assistant had the whole limit to allow it, and asking again puts it back.
4. **The first answer wins** (`r.settle` under the run's lock); the others are refused with `errPermissionDecided`, and the owner's
   inbox item closes. One `permission_answered` event says who decided and why; the stream labels it by its `By`, and the events the
   previous build stored (`chair-allow`, `auto-reject`, ...) read as `permission_answered` (`store/events.go`).
5. **Whom a prompt waits on is stored on its card** (ADR-0107 step 7d): `Card.permission_wait`, a `model::PermissionWait`
   (`Chair`, `Owner`, `Driver { who }`, `who` the Chair's card id), set with status `permission` (`Card::ask_permission`) and
   cleared by every other status. The card shows its request due first, and the agent's line reads the field, never the
   activity's words. A request taken off the run (answered or refused) leaves the card on the next one it has,
   or running.

## Hand-over

The Chair calls `hand_over(card, employee | agent, why)` (`HandOver`, `handover.go`):

- **Checked at once, recorded at once.** The hand-over is refused for a card not of the team, a question card, a card handed over or
  being handed over, and by every hiring rule of the hire that would take it on (`hiringOf`, shared with `Hire`). The `handover`
  decision with the Chair's why is stored, and the card's `Handover` field holds the move.
- **The note turn.** A leaving card with an open session has its running turn cut and is asked for a handover note in one more turn
  (`handoverBy`, blocking priority), within `handoverNoteWait` (3 minutes). That turn's words are the note; a turn that fails or says
  nothing, a card with no open session, or the limit passing leaves the core's note (`coreNote`: the plan, the draft it was writing,
  its last report, `git status --short` of the worktree).
- **One change of the chain** (`completeHandover`): the leaving card stops (`handed over to <label>`), its queue is dropped, its
  `Handover` cleared and a `handed_over` event names the new card; the new card, its employee when fresh, its `handoff` edge from the
  leaving card (label `handover`), its `hire` decision and its brief (`handoverBrief`: `personBrief` for a person's session, the worker
  brief and a HANDOVER NOTE for anyone else) are stored with it. The leaving card's run then closes. It runs once per card
  (`handovers` lock, the field read again), so the note turn, the timer and a restart never make two cards.
- **A restart in between** finishes it: `Recover` notes every card whose `Handover` is set, and `ResumeInterrupted` completes each
  with the core's note once the AI runtime is up, so the new card's session can open. In a chain finished meanwhile no card
  is hired: the hand-over is cleared.
- **A person is offered the new card** as any task and may decline; their decline reaches the Chair as the card's report. Hand back
  is the same call toward an agent, with the person's session writing the note.

## Consequences

- One session path for every employee: a person carrying on an AI's work is a new card on the same path, and what their session does
  is recorded as any card's stream.
- The owner is no longer the gate of every agent's prompt: the Chair decides first, and an undecided prompt never blocks a session
  past the owner's limit.
- The policy that decided prompts by command (`decidePermission`, the outward-command lists) is gone; deny rules and the secret
  refusal are the rails that stay code.
- A Chair's judgement of when to use a person, in which shape, lives in its brief (`personWorkRules`), never in a flag.

## Revisit when

- **A refused-at-the-limit prompt hides work the owner wanted done** (a judge or a Chair asking again and again for the same
  command): the seam is `driver_decides`'s no-Chair arm, where a per-kind rule (the owner's standing allow for a command
  shape, decided by the Assistant once) would answer before the refusal. Not built: no such rule exists, and a refusal is
  the safe default of a rail.

- **An ask rule the owner wants answered only by himself:** a prompt kind that skips the Chair and never falls to the driver.
- **A person sits in the Chair's seat:** their session's prompts have no Chair above them, and their turn end settles the chain.
- **The person's own tools reach a hosted session** (a remote terminal, a file view in the Work plugin's tab): the permission prompt
  reaches them in the tab.
- **`secretPath` misses or wrongly catches a file**: it reads paths by name (a bare `cat credentials` in the working directory
  passes; `.env.example` is refused). The seam is `secretPath`, which would learn to read what a file holds, not only its name.
- **A hand-over should keep the leaving session's transcript in the new session**, not only its note.
