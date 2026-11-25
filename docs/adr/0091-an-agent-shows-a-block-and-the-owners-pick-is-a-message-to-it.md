# 0091. An agent shows a block in its stream, and the owner's pick is a message to it

Status: accepted, 2026-11-13. Extends ADR-0087 (no agent waits on the owner) and ADR-0072 (the contract is `crates/wire`).
Source: the owner's words of 2026-11-13 (translated): "Now implement every idea you just suggested", idea 22 of
`docs/research-agentes-chat/report.md` ("Spec-drawn interactive blocks in an agent's message (spec in `crates/wire`)",
ADR first).

## Context

An agent talks to the owner in words. When it has options to offer ("three ways to cut this migration, I go with the
first") or numbers that read better as a picture (open MRs a day for two weeks), it writes prose or a Markdown table
(`web/src/components/Markdown.vue:4` renders with `marked`, tables styled at `:138`). The owner can only answer by typing.
What he picks never reaches the agent as a pick, and a series of numbers stays a table.

The one way an agent offers options today is `ask_owner` with `options` (`crates/orch/src/tools.rs:517-545`,
`AskInput::options` at `:843`): an inbox item of kind `Question` that the Assistant decides for him (ADR-0087). That is a
question the work needs answered. Nothing lets an agent show the owner a choice it does not need answered: one it has
already made, which he may change.

agentes.chat solved the rendering half (their ADR 0113, read for ideas only, AGPL): the agent sends data, a spec of parts the
app draws with its own components, checked on the server with an error naming the place that is wrong; the state lives on
the server; the agent reads it in its next turn. Their parts include sliders and formulas in an interpreted language, and
an HTML page in a sandboxed frame behind a click.

The customer is the owner reading an agent's stream: he gets the agent's options and figures in a form he can take in at a
glance and change with one click, and the agent gets his change as words it acts on.

## Options

1. **Nothing new.** Options in prose, numbers in Markdown tables, a real question through `ask_owner`. Costs nothing; the
   owner types every answer and reads every series as digits.
2. **A block: a spec of parts the app draws, its state on the server, the owner's pick sent to the agent as his message.**
   Two parts to start: a choice and a chart. **Chosen.**
3. **agentes' whole design**: parts with formulas in an interpreted language, sliders and fields, and the agent's own HTML
   in a sandboxed frame. Each piece is a second language or a second security boundary to build and keep, for uses no
   chain has asked for yet.
4. **`ask_owner` drawn as a block.** Mixes two concepts: a question the work needs (decided by the Assistant, recorded,
   overturned) and an offer the work does not need. One would be shown as the other.

Models applied: **first principles** (what a block is: data the agent already holds, drawn by Remit's own components; the
only new fact is the owner's pick), **inversion** (what would make a block park work on the owner? a choice with no answer
the agent goes on with: so every choice carries the agent's own pick from the start), **second-order thinking** (each part
kind is a component on every channel forever: start with the two that prose and Markdown cannot do). Patterns: a
boundary that parses outside data once into a closed type (the anti-corruption layer's job, at the tool's input), and
additive schema evolution (a new part kind is a new variant; stored blocks keep reading).

## Decision

### What a block is: one definition in `wire::app`

The block types live in `crates/wire/src/app.rs`, beside `Activity`, and every side compiles or generates from them
(`make gen` for the web core). With a cargo feature `schema` that `orch` turns on, they also derive `schemars::JsonSchema`,
so the tool's schema the agent is shown is generated from the same definition.

- `BlockSpec { title: String, parts: Vec<BlockPart> }`: what the agent writes.
- `BlockPart`, tagged by `type`:
  - `Choice { id: String, label: String, options: Vec<BlockOption>, many: bool, picked: Vec<String> }`: one pick, or
    several when `many`; `picked` holds option ids, the agent's own pick at the start.
  - `Chart { id: String, label: String, kind: ChartKind, x: Vec<String>, series: Vec<ChartSeries>, unit: Option<String> }`.
- `BlockOption { id: String, label: String, detail: Option<String> }`: `detail` carries a line of comparison, so options
  laid side by side do the work a table of options would.
- `ChartKind { Bar, Line }`; `ChartSeries { name: String, values: Vec<f64> }`.
- `Block { spec: BlockSpec, picked_by: Option<Actor>, picked_at: Option<String> }`: what is stored and shown. The picks live
  in each `Choice::picked`; `picked_by` and `picked_at` say who changed them last and when (none: still the agent's).
- `Activity::Block { block: Block }`: how a channel meets it in a card's stream.

A table is not a part: Markdown in the message renders one already. A poll is a `Choice`: there is one owner.

### Which agent draws one, with which tool

Every card's agent, the Assistant included, has `show_block` (`Effect::Writes`), added in `tools_for` beside
`send_feedback`, before the Assistant's early return (`crates/orch/src/tools.rs:397`). No capability gates it: it writes
only in the agent's own stream. Its input is `{ spec: BlockSpec }`, closed by `deny_unknown_fields` like every tool input.

The spec is checked once, there, by one pure function in a new `crates/orch/src/blocks.rs` (`check_block`): ids unique
across parts and options, a choice with at least two options and `picked` naming its options (one id unless `many`), every
series as long as `x` and every value finite (a negative one too: a chart draws its bars and lines from zero, below it
as well as above), labels non-empty, and the spec at most 16 KB as JSON so a block fits the
stream. A refusal names the place (`parts[1].series[0].values: 9 values, x has 10`), so the agent fixes it in the same
turn. A spec that passes is stored as written and trusted everywhere after.

The tool records the block as an event of the card (kind `System`, status `block`, data `{"block": Block}`), written in the
tool's transaction through `Records::events`, and answers the agent with the event id. `orch::activity` types that event
as `Activity::Block`, as it types a permission prompt asked (`crates/orch/src/activity.rs:88`). The event is pushed like
any event of the stream, so the block appears where the owner watches that agent.

The tool's description tells the agent the one rule: a block never waits. Set `picked` to what you go ahead with and go
on; what the work cannot go on without goes through `ask_owner` or `ask_chair`, which an agent decides.

### How the owner's pick reaches the agent without anyone waiting

One call in `wire::app`'s `changes`: `block_pick(event_id: i64, part: String, picked: Vec<String>) -> Event`, carrying
the Request id every changing call carries, so a replay after a reconnect does not act twice. The orchestrator
(`Orchestrator::pick_in_block`, in `blocks.rs`, inside `act`):

1. reads the block, refuses a `part` that is not a choice of it or `picked` outside its options, and refuses when the
   card takes no message, by the rule `card_prompt` follows;
2. writes the new picks, `picked_by` and `picked_at` on the event (its `rev` rises; the push reaches his other tabs);
3. sends the card one message by the owner through `owner_says` (`crates/orch/src/session.rs:1184`) with
   `Delivery::Queue`, its text written by one pure function (`pick_words`): the block's title, the part's label and the
   picked labels. A message from this block that the card has not read yet is replaced, not added to, so a run of clicks
   reaches the agent as one message holding the last picks.

The agent reads the pick in its next turn, as it reads anything the owner says, and an idle card is woken by it as by
any message. Nothing waits on the owner at any point: the agent has gone on with its own pick, raises no inbox item, and
the Assistant has nothing to decide. A block nobody touches changes nothing.

### How the owner sees and steers it

- The block is drawn in that agent's stream (its drawer, the Assistant's panel, a chain's page) by `BlockView.vue` from the
  replica's event, a row of its own that the stream never folds into a run of tool calls. A `Choice` is a radio or check
  group (shadcn), options as cards with their `detail`, the pick shown by a check icon and text, never by colour or a ring
  alone. A `Chart` is SVG in token colours, no chart library, with its numbers as a table behind a toggle as its text
  equivalent. Labels and details render as plain text, never Markdown or HTML.
- A click calls `block_pick` through the client core (`crates/replica`), like `card_prompt`; the page renders the state the
  server pushes back. The footer says whom the pick tells ("Tells the Chair") and, once he picked, "Picked by you · 2m".
- He steers the agent's use of blocks in words, as everything else ("show me options as a block", "no charts for this").

## Threats

An outsider reaches a block only through the agent: text the agent read (an MR, a webhook body, a page) may steer what it
draws (OWASP LLM01).

| Threat | Control |
|---|---|
| A block poses as a Remit control ("Approve the merge", "Grant production") | A pick only tells that agent the owner's words; it grants, approves and merges nothing. The footer names the agent it tells |
| A block asks for a secret | No part takes free text: a block can offer options, never a field. Credentials stay in the inbox's write-only field (ADR-0087) |
| Markup or a link in a label | Labels, details, axis names and series names render as text; no part carries a URL |
| Someone other than the owner picks | `block_pick` is a call of an owner session on `/link`; `rpc::MentorApi` refuses it to a mentor because it does not name it (ADR-0089); the client-key door offers only its own allow-listed MCP tools (`crates/orch/src/client_tools.rs:244`) |
| A flood of blocks fills the stream | The 16 KB bound on one block; a rate per turn is hardening (below) |

## Consequences

- The owner can change an agent's choice with a click and read a series as a chart; the agent hears it as his message.
- `wire::app` gains six types, an `Activity` variant and one call; `wire` gains the `schema` feature and `schemars` under it.
- `crates/orch/src/blocks.rs` holds `check_block`, `pick_words` (pure) and `pick_in_block` (the edge); `tools.rs` adds
  `show_block`; `orch::activity` types the event; the web adds `BlockView.vue`, a `web/verify` page seeded with a choice of
  three options with details, a picked `many` choice, a two-series line chart and a bar chart, and the stream's row for it.
- A new part kind is a new variant and a component on every channel; a channel that cannot draw it shows its text equivalent.
  Removing or reshaping one is a numbered migration of the stored events.
- Tests, two, both of what AGENTS.md keeps: `crates/wire/tests/round_trip.rs` reads a stored `Block` of each part kind
  across versions; one orchestrator test drives the main path (`show_block`, then `block_pick` twice before the card reads,
  then the card's next turn holds one owner message with the last pick).
- Not built: formulas, sliders, fields, an agent's own HTML, polls of several people, updating a shown block.

Enforced by: `a_stored_block_of_each_part_kind_reads_across_versions` (`crates/wire/tests/round_trip.rs`),
`a_run_of_picks_reaches_the_agent_as_one_message_with_the_last` (`crates/orch/src/blocks.rs`).

## Revisit when

- **A chain needs a computed part** (a slider moving a projection): a small formula language in `wire`, sans-IO, with a
  step budget and list and text ceilings, evaluated by the same code on the server and in the client core. The seam is
  `BlockPart`: a new variant.
- **What no part covers is asked for** (a free drawing, a game): an agent's page behind a click in a sandboxed frame with an
  opaque origin and a fixed policy, run through `aio-threat-models` first. Not before.
- **A mentor or several people answer one block**: `rpc::MentorApi` names `block_pick` for blocks of helped cards, picks
  become per actor, and a poll gains a closing time. The seam is `picked_by`.
- **An agent needs to redraw a block with newer data**: `show_block` takes `replaces: event_id` and the old block shows as
  replaced.
- **Hardening is asked for**: a rate of blocks per turn and per card, a cap on picks per minute, and the text fallback a
  non-web channel (the CLI, iOS) renders for each part. The seam is `check_block` and the `Activity::Block` row.
- **A block ever has to gate work**: it does not; that is `ask_owner`, decided by the Assistant (ADR-0087). A design that
  needs the owner's pick before the agent can go on is a defect of that design.
