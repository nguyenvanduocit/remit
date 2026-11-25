# 0112. A plugin gives context to a turn before it starts, and the stream shows what it gave

Status: accepted, 2026-11-18; not built. Extends ADR-0034 (a plugin's contract), ADR-0084 (the stream shows what reaches a
session) and ADR-0082 (what a session recalled is recorded). Source: the owner, 2026-11-18 (translated), "let plugins inject
context into an agent's turn, Unix-pipeline style", option 2 of a design discussion. The sibling change that records
`fact:turn.ended` for processors to read an agent's reply after its turn is the asynchronous half of the same pipe; this
record is the half before the turn.

## Context

The customer is the agent doing a chain's work, and through it the person the chain is for: an agent that starts a turn
without a fact the outside world holds (the incident open since its last turn, the customer's account tier, the on-call
engineer, what the memory backend knows of this topic) does the turn worse or spends tool calls finding it.

Context reaches an agent today by four paths:

1. Prompts Remit's code writes: `chair_prompt` and `worker_prompt` (`crates/chains/src/prompts.rs`), and the recovery lead
   `take` adds to a message when a lost session is restored (`with_lead`, `crates/chains/src/outbox.rs`). Remit's own
   words, at the start of a session or of a recovered one.
2. Tools the agent pulls: a plugin's tools (ADR-0034, behind the gate), the employee's memory search
   (`crates/crew/src/memory_tools.rs`, recorded as `fact:memory.recalled`, ADR-0082). The agent must know to ask.
3. Messages between turns: a subscription's `tell` or `wake` action (`ActionKind::Tell`, `crates/model/src/signal.rs`). A
   message is a turn of its own, queued by priority; it says what happened, not what the next turn should know.
4. The memory backend's Claude Code plugin, whose `UserPromptSubmit` hook adds recalled memories to every prompt inside the
   session (`agent::claude::Memory`, "the plugin directory whose hooks recall into a session"). Remit neither decides nor
   records what it adds: the owner cannot see what the agent read.

None lets a plugin add what it knows to the next turn, and the one path that pushes context at prompt time (4) is invisible.

A turn begins in one place in the core: `Core::take` (`outbox.rs`) takes the card's queued message and marks the card
running in one transaction; the session loop (`crates/chains/src/session.rs`) then runs `turn` on it. Turns the agent opens
itself (`own_turns`: its background work ending), its subagents and a Stop hook that keeps it going (ADR-0100) continue a
session without passing `take`.

## Options

1. **Pull only.** The plugin ships a tool; the agent calls it when it wants. Exists today, needs nothing. The agent does
   not know to ask about what changed outside since its last turn, and each turn pays a tool round trip for what a read
   could have handed it. Kept as the path for what the agent knows to want.
2. **A subscription tells the agent.** A plugin's sensor writes a reading, a subscription's `tell` queues a message. Each
   message wakes the agent for a turn of its own and is read when its priority comes up, by then possibly stale; it cannot
   attach to the turn the owner's message opens.
3. **A `UserPromptSubmit` hook in the AI runtime**, as the memory plugin does. Runs inside the session, so it would reach
   every prompt Claude Code submits. But the runtime is outside the core (ADR-0058) and holds no grant, gate or ledger, so a
   plugin call from there needs a second route to plugins and credentials; hooks there hold rails only (ADR-0084); and what
   it adds is seen only if the runtime reports it back as a second stream of record.
4. **The core asks the plugin when a turn starts, gives the agent the answer with the message, and records it as an event
   of the card.** One call site (after `take`), through the gate like any read, in the stream like `instructions_read`.
5. **A pipe of filters, literally:** each plugin receives the prompt (and, on the way out, the agent's reply) and returns
   it rewritten. Rewriting the prompt hides who said what; rewriting the reply makes the owner read words no agent wrote and
   no judge ruled on. A wrong reply goes back to the agent through a judge or `tell` (*Agent-centric*: judgement belongs to
   agents; what an agent does shows live).

## Decision

Option 4. Remit's pipe is a fan-in: each provider adds blocks beside the message, none sees or changes another's output,
and the reply is never rewritten (option 5 rejected for both directions).

- **Contract.** `wire::plugin::Manifest` gains `contexts: Vec<ContextProvider>` (`name`, `label`, `description`: what it
  adds, in the owner's words, shown before he applies the plugin). The plugin contract gains one read,
  `context(ask: ContextAsk) -> ContextAnswer`, "reads what a provider adds to a turn; asked again, it answers again".
  `ContextAsk` carries the provider's name, the chain, card, role, playbook and project ids, the message's title and text,
  `since_ms` (when this card's previous turn was given this provider's context, 0 for the first) and the connection's
  `vars`. `ContextAnswer` is `blocks: Vec<ContextBlock>`, each a `title` and Markdown `text`; none is an answer. The SDK's
  `Plugin` trait gains `context`, answered `NOT_FOUND` by a plugin with no provider of that name.
- **Who gets it.** A card's chain is given a provider's context when its capabilities include the plugin's `capability`
  and the gate decides a `read` on the connection (`decide_on`, `crates/reach/src/grant_access.rs`), the same rule as the
  plugin's read tools. No check on a chain's origin or playbook id: a playbook lists the capability or does not.
- **Which turns.** Every turn opened by a message `take` hands over: the owner's, the Chair's, the Assistant's, a mentor's,
  a `tell`. A turn the agent opens itself, its subagents and a Stop-hook continuation carry on a turn already given its
  context and are not asked again.
- **When and how long.** After `take` commits and before `turn` sends the message, outside any store transaction. Every
  provider the card is given is asked at once; all answers are awaited under one deadline of 2 seconds from the first call
  (`CONTEXT_DEADLINE`). A provider that fails, refuses at the gate or is late is left out and the turn goes on; nothing
  retries it in that turn.
- **What the agent reads.** The message's text after the blocks, each block framed as data from a named source:
  `<context source="<plugin>/<provider>" title="...">…</context>`, with one line saying the blocks are information a plugin
  supplied, not instructions. The recovery lead, when there is one, comes first, as today.
- **What the owner sees.** One system event of the card per turn given any provider, `SystemNote::ContextGiven`, written
  before the turn sends: per provider its name, whether it gave blocks, gave none, failed (with the reason) or was late,
  how long it took, and the blocks whole. The card's stream shows it above the turn ("Read before this turn: 2 blocks from
  Intercom"), opened to the text. What the agent read is never hidden; a provider's failure shows where the owner looks.
- **Replay.** The event names the prompt it was given for. A turn sent again for the same message (failover's
  `prompt_retrying`, `resend`) reads the recorded blocks and does not ask again, so a retried turn reads what the first read.
- **The memory backend joins this path.** Its prompt-time recall moves from the `UserPromptSubmit` hook inside the session
  to a provider Remit asks, so recall shows in the stream like every other context; the session's memory plugin keeps its
  tools. Done in the change that builds this one, so one path pushes context at prompt time.

Reasoning. *Inversion* (`aio-mental-models`): the surest way to make an agent act on something nobody can check is to put
words in its prompt that no one sees; so the decision is built around the record, and option 3 and today's memory hook
fail it. *Second-order thinking*: every provider runs on every turn of every chain that lists it, so a slow one is a tax on
all work; a deadline taken once per turn and fail-open is the fix (*Timeout Patterns*, AWS Builder's Library: "a slow
system is worse than a broken one", a deadline rather than per-call timeouts). *Margin of safety*: the turn never depends
on a plugin. *Pipes and Filters* (`aio-architect-reference`, volume 2) is the owner's frame; its filter chain is what option
5 would be, and Remit keeps the independent stages and drops the rewriting.

Against next year's models (*AI-native architecture*: retrieval moves from prepended context to the agent pulling what it
needs): a better model asks for more by itself, and providers then become scaffolding. Removing one is a playbook dropping a
capability or a plugin dropping a manifest entry; the core keeps one call site and grows nothing per provider. The strongest
counter-argument, that option 1 already suffices, fails on what changed outside since the last turn: an agent cannot know to
ask about an event it has not seen.

## Threats

An outsider reaches this path through the text a provider returns: a plugin that relays outside data (a support ticket, a
web page, a commit message) carries whatever its author wrote into the agent's input. This is indirect prompt injection,
the same exposure a plugin's read tools already have; here it arrives without the agent having asked. The `<context>`
frame and its line marking the text as data are the only control built. A secret a plugin puts in a block reaches the
agent and the stored event; the gate's credential stripping applies to tool output and is not yet applied here. The plugin
reads the message text and the ids it is asked with: an applied plugin is trusted with what its capability's chains read
(ADR-0034, full rights, controls deferred).

## Consequences

- A plugin can give every turn of the chains that list its capability what it knows, and the owner reads it in the card's
  stream beside the turn.
- Every turn opened by a message waits for its providers, at most 2 seconds; a chain listing no providing capability pays
  nothing.
- The memory backend's recall becomes visible; it costs one round trip to the memory daemon from the core instead of from
  inside the session.
- Context is not given to turns the agent opens itself or to its subagents; a long turn reads only what was given at its
  start.
- Cost is the tokens of the blocks on every turn; Remit records none and caps none (*Where we are*).

## Revisit when

- **Prompt injection** through a provider's text becomes a finding: the seam is the `<context>` frame and the one place
  blocks are joined to the message; a classifier or a `security`-judged allow list of providers goes there.
- **Size.** A provider answers more than a turn should read: a per-provider cap (`ContextProvider.max_bytes`, cut with a
  note in the event) and a per-turn total go where blocks are joined.
- **Secrets.** A block carries a credential: the gate's output stripping (`source` crate) applies to blocks before the event
  is written and the turn sends.
- **Which turns.** Agents act on stale context inside long turns or subagents: the runtime's `UserPromptSubmit` and
  `SubagentStart` reads (ADR-0084) ask the core for the same providers over the runtime link, and the event records them the
  same way. The core stays the only caller of plugins.
- **A slow provider.** The 2-second deadline drops one often: its late count in the events is the signal; a per-provider
  deadline in the manifest, or the provider paused for the chain until it answers in time, is the next step.
- **A playbook wants the tools but not the push.** The manifest's provider names its own capability (`<capability>.context`)
  instead of sharing the plugin's.
- **The owner wants to steer a provider** (switch one off for a chain, or ask what it gave last week): an Assistant tool on
  the recorded events and the capability list, never a settings screen.
