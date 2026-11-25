# 0082. The work reads as it happened: edges are acts, and the coach writes each chain's retrospective

Status: accepted, 2026-11-11. Source: the owner's agreement of 2026-11-11 to `docs/research-peers-product/report.md`
(proposals 5, 8 and 9, journeys J3, J6, J9) and its deck (slides "Change B", "B2", "B3"). Builds on ADR-0063 (an employee
learns only what people and verdicts teach it; the coach holds the policy) and ADR-0073 (a workflow is a script Remit replays).

## Context

The customer is the owner reading a chain's work: what failed while folded, how agents passed work, and what the chain
should teach. Read in code (HIGH unless marked):

- A folded group of tool calls says how many ran and how long, and whether one still runs, never whether one failed
  (`web/src/stream.ts:65`, `running` only; `web/src/components/StreamBlocks.vue:113-117`); each call's status is typed by the
  server (`Activity` `tool`, `status`), and each call and group already shows its duration (`StreamBlocks.vue:117,126`).
  The fold itself is computed in the browser (`stream.ts` `blocks`).
- Edges are already typed and recorded when an act happens: `hire` (a spawn, `crates/orch/src/spawn.rs:128-135`),
  `handoff` (a hire's inputs, a route, a hand-over: `chair_hire.rs:133-155`, `handover.rs:348`), `feedback` (sent back) and
  `question` (`tools.rs:464-490`, `route`), each with a label (`relation_label`, `tools.rs:767-774`). What caused one is the
  Chair's decision beside it (`accept`, `rework`, `route`). A conditioned flow declared ahead ("on failure, send back") is
  what a workflow script is (ADR-0073, ADR-0075).
- A message to an agent that is not in a turn waits in its queue (`Card.queued`, shown only as "+n queued" on a running line,
  `crates/orch/src/agentstanding.rs:307-313`).
- Teaching is recorded as `fact:teach.*` and the coach, a built-in chain whose Chair lives by subscribe, is woken by them
  (`crates/orch/src/teach.rs:1-11`, `coach.rs:1-7`); its writing tools include `promote_memory`, `draft_forget`,
  `request_contest` (`coach.rs:621-657`); a lesson for every employee and a forget wait for the owner (`coach.rs:4-6`).
  `read_chain` gives a chain's goal, summary, verdicts and reworks (`crates/orch/src/capabilities.rs:544-551`). No record says,
  after a chain, what went wrong, which memory helped or misled, or what a better brief would have been.

## Options

For edges:
1. **Declared, conditioned edges** (GraphCode, Multica's "when done, move to"): the owner or the Chair wires the team ahead.
   It is a second orchestration beside the Chair's judgement and beside workflows, and a form in all but name.
2. **Edges stay records of acts; the view names their kind and what caused them**, and a message waiting for an agent shows as
   staged. The Chair's judgement is the condition; a flow fixed ahead is a workflow.

For the retrospective:
1. **The chain's own Chair writes it at finish.** The verified would judge itself ("the verifier stays outside the verified").
2. **Remit computes it** from reworks, failures and verdicts. It can count, not weigh: which memory misled is a judgement.
3. **The coach writes it, woken by the chain's finish**, as a record of its own, and proposes a lesson through the lesson path it
   already has.

## Decision

Edges option 2; retrospective option 3. Patterns: event sourcing's read side (edges and decisions are what happened; the view
reads them), separation of concerns (the coach owns lessons; the Chair owns its chain). Mental models: Gall's law (the working
Chair-and-hire model stays; no second orchestration beside it), incentives (an independent coach has no stake in the chain looking
good), next year's models (a better coach writes a better retrospective with no change here).

### A transcript that scans

- A folded tool group shows how many of its calls failed, with an icon and the words "1 failed", from the statuses the server
  typed; durations stay as they are. The fold stays in `stream.ts` for now (see *Revisit when*).
- "While you were away" heads the stream when the owner returns (ADR-0079, `ChainView.away`).

### Edges

- No new edge kind and no declared condition. The view says each edge's kind in words and by line style, never colour alone:
  `hire` "spawned" (or "hired"), `handoff` "hand-off", `feedback` "sent back", `question` "asked"; and what caused it, from the
  Chair's decision of the same moment: "on accept", "on rework". `relation_label` writes these words; the web maps the closed
  `EdgeKind` to its line style.
- An agent not in a turn with messages waiting reads "{n} message(s) staged: read on its next turn" (`AgentStanding.doing`),
  and its composer says the same (`on_send`, ADR-0080).
- A worker whose turn ended with a report the Chair has neither accepted nor sent back reads "Report waits for the Chair".

### The retrospective

- A finished chain writes the fact `chain.finished` in the step that finishes it, as `chain.merged` is written
  (`crates/orch/src/fact.rs:43`). The coach ships a subscription to it; its brief says it writes a retrospective for every
  finished chain but its own and the analyst's.
- What a session recalls is recorded where it is recalled: `search_memory` (`crates/orch/src/memory_tools.rs:83-100`) writes
  the fact `memory.recalled` naming the employee, the card and the ids of the memories it returned. The tool event cannot
  serve: its output is cut at 4000 bytes (`crates/orch/src/stream.rs:322-343`, `tool_data`), so the ids of a long answer are
  lost. `read_chain` adds, per card, the memories those facts name, so the coach can say which helped and which misled.
- A new coaching tool, `write_retrospective(chain_id, issues, memories, brief)`, the coach Chair's, stores one
  **Retrospective** per finish: the issues with their impact (`high`, `medium`, `low`) in words, each recalled memory marked
  helped or misled with why, and an improved brief. It is an entity of its own (a table, a push, carried in `ChainView`),
  because chains finish without bound, as decisions are.
- The lesson it draws goes through the coach's existing lesson path (`promote_memory`); the retrospective names the change item
  that path raises, when it raises one, and the chain page shows the item's Apply and Not now there. ADR-0063's rails decide who
  accepts it; nothing here adds an acceptance path.
- "Start again with this brief" starts a new chain from the improved brief (`chain_create` with `intent` = the brief), which the
  owner may do from a browser (AGENTS.md: starting one is a browser RPC).

## Consequences

- A failure inside a folded group is visible without opening it.
- How agents relate reads as acts with causes; nothing new to configure, and nothing for the owner to wire.
- Every finished chain costs the coach one turn; quality outranks that cost (AGENTS.md, *Where we are*).
- The coach's view of a chain grows by the memories it recalled.

## Tests (written first)

- orch: `a_finished_chain_wakes_the_coach_and_its_retrospective_is_carried_in_the_chain_view`.

## Revisit when

- A second channel shows a chain's stream (iOS, the CLI): the fold and its failed count move into the client core
  (`crates/replica`), so every channel folds alike. Until then the browser's fold is the one fold, over statuses the server typed.
- The owner asks for a team wired ahead: that is a workflow (ADR-0073), extended, not edges with conditions.
- A retrospective's lesson should be accepted by a mentor other than the owner: ADR-0063's mentor path, named there.
