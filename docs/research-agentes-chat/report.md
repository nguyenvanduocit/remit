# Agent workspace research

Remit adopted five findings from the [agentes.chat](https://agentes.chat) study. The implementation decisions are recorded in ADRs [0090](../adr/0090-one-job-per-hue-and-a-state-shows-in-a-chip.md) through [0094](../adr/0094-the-page-stays-readable-offline-and-sends-wait-in-the-core.md).

- Use one colour per purpose and show a state in a chip.
- Let an agent show an interactive block and receive a selection as a message.
- Stop turns that repeat without progress.
- Support teaching from a reply and record overturned decisions.
- Keep a disconnected page readable and queue outgoing messages in the client core.

## Findings cited by the decisions

- D4. A permission card should name the action and explain why it was requested.
- D6. A disconnected page should keep the last readable state and accept queued messages.

## 2. Ideas ranked by value to the product

"Work" = makes a thing work (build now). "Hardening" = waits for the owner's ask.

| Rank | Idea | From | Value | Effort | Kind |
|---|---|---|---|---|---|
| 1 | **Waiting-card anatomy**: action as a plain title; exact command in a sunken block, folded; why in one sentence; traits as badges ("Production", "Can't be undone"); button names the action; neutral card, coloured chip; decided → one line "Allowed by the Assistant · 2m · Overturn". Server sends title, reason, traits (fixes D4) | screens, design | HIGH | M | work |
| 2 | **One line per turn of work**: fold "Used n tools / Thinking / Worked for" into one row naming what was touched and the time, opening into plain-sentence steps; repeats merge "×n"; failures and refusals stay outside the fold | screens, agent-work | HIGH | M | work |
| 3 | **Scroll belongs to the owner**: let go of the bottom only on a real upward gesture; a long new message lands at its first line; "↓ 3 new · Chair, Reviewer" (today one 60 px rule, `web/src/useStickToBottom.ts:6`) | ux-flows | HIGH | M | work |
| 4 | **Nothing sent blocks or is lost**: the message shows at once, "sending…" only after 500 ms; offline keep the last replica read-only and queue sends in the client core (fixes D6) | ux-flows | HIGH | M | work |
| 5 | **Doing line leads with the work**: plan step or brief title, elapsed time, then the tool (`crates/orch/src/agentstanding.rs:646` leads with the tool) | agent-work | HIGH | S | work |
| 6 | **Stop takes a why the Chair reads**, "do not restart this on your own" (today a fixed "stopped by the owner", `crates/orch/src/lifecycle.rs:460`) | agent-work | HIGH | S | work |
| 7 | **"At work now" across chains**: every seat running or waiting, what it uses, since when (time warms on running work, never on a known wait), Stop with a why | agent-work, ux-flows | HIGH | M | work |
| 8 | **Repetition rail in a turn**: fingerprint tool calls; on a repeat steer a note in, on a second end the turn with a report | philosophy (their ADR 0015, 0131) | HIGH | M | work |
| 9 | **Headroom for the Assistant** near plan windows: hold what the scheduler starts on its own, let the owner's and Assistant's turns pass | philosophy (their ADR 0118) | HIGH when it bites | S | work |
| 10 | **Giving up needs evidence**: the judge and briefs treat "cannot" as unproven until the worker's own tools were tried | philosophy (their 0109, 0167) | HIGH | S | work |
| 11 | **One state grammar for every list**: skeleton after 150 ms; error with Retry only when it helps; empty = one sentence + one action, often "Ask the Assistant"; "nothing matches" + Clear search; a purpose sentence under each page title | ux-flows, screens | MED-HIGH | S | work |
| 12 | **Copy pass by rule**: agent as subject, what happens next after every wait, consequence before the button, act and scope on the button | ux-flows | MED-HIGH | S | work |
| 13 | **The agent is alive where it is shown**: state on its badge, a "Now" line on agent and employee pages, a live chain chip in the Assistant's conversation, an agent's runs as a list with outcome and source | design, screens, agent-work | MED-HIGH | M | work |
| 14 | **Body text 14px/1.6 where the owner reads** (stream, inbox); board and tree stay dense; one control ruler (buttons and inputs on one height scale); named spacing tokens; generate or delete the token copy (fixes D2) | design, screens | MED | S | work |
| 15 | **Teach from the message**: Helpful / Not helpful / Correct reply on an agent's turn, recorded as a `teach.*` signal for the coach | screens | MED | S | work |
| 16 | **ADR on colour roles**: blue does four jobs today; callouts stop being tinted, colour moves to a chip | design | MED | M | ADR, then work |
| 17 | **Keyboard over lists**: J/K/Enter over the Assistant's feed and the inbox, `/` to search, Esc closes the innermost layer first | ux-flows | MED | S | work |
| 18 | **Narrow windows** (fixes D3, D5): fold the areas into a menu, one column with a switcher, an edge cue when the board scrolls | screens | MED | S–M | work |
| 19 | **Digest from stored data, no model call**; "next run" on schedules and subscriptions | agent-work, screens | MED | S | work |
| 20 | **Overturn rate as a signal** to the coach; **already asked**: an agent reads what other chains already asked the owner | agent-work, philosophy | MED | S | work |
| 21 | A face per seat; a small Remit icon set (~20 domain glyphs) | screens, design | LOW-MED | M | owner's taste |
| 22 | Spec-drawn interactive blocks in an agent's message (spec in `crates/wire`) | agent-work | MED | L | ADR first |
| 23 | Hold-to-confirm for what cannot be undone; motion tokens; one sound when something waits | screens, design | LOW | S | hardening |
