# 0079. Seen is what the owner viewed, kept by the server

Status: accepted, 2026-11-11. Source: the owner's agreement of 2026-11-11 to `docs/research-peers-product/report.md`
(proposals 5 and 7) and its deck (slides "A5", "Change B"). Replaces the Assistant feed's own seen mark
(`Assistant.decided_seen_at`, `see_feed`).

## Context

The customer is the owner coming back to work that went on without him. He needs three answers: which chains finished
since he last looked, what happened in a chain while he was away, and which waits are new (the ones worth a notification,
ADR-0078).

- Remit keeps one seen mark today: the Assistant feed's `decided_seen_at` on the Assistant's record
  (`crates/model/src/model.rs:537`), set by `see_feed` (`crates/orch/src/assist_feed.rs:98-115`). The browser calls it whenever
  the feed is mounted and new lines arrive (`web/src/components/AssistantFeed.vue:16-28`), visible or not: seen on mount,
  which is the defect the report saw in Vibe Kanban ("a turn watched live flagged as unseen", and its mirror, a hidden tab
  marking things seen). HIGH.
- No chain has a seen mark; nothing says "finished since you last looked" (report J1, J3). HIGH.
- A recap of a card exists for agents (`crates/orch/src/recap.rs`, `what_happened`), not for the owner. HIGH.
- Every chain's decisions are already words with who and why (`model::Decision`; `docs/design.md` table, Decision row). HIGH.

## Options

1. **The browser keeps seen** (localStorage per chain). Lost on another device or a cleared profile; two tabs disagree; a
   rule in the browser (AGENTS.md *Server-driven*).
2. **A field on each record** (`Chain.seen_at`). Writing it moves the chain's `rev` and `updated_at`, which reorders the
   Running and Done columns (`standing.rs:107`, `newest`) and pushes the chain to every channel each time the owner looks.
3. **One small table of seen marks, keyed by what was seen**, written by one call the browser makes when the owner has
   actually viewed something; the Assistant feed's mark moves into it.
4. **An agent writes the "while you were away" recap** each time he returns. A model call per visit for words that the
   chain's decisions already hold, and a recap that may say what did not happen ("reported, never estimated").

## Decision

Option 3, with the recap made by the server from the chain's decisions. Patterns: a read model kept beside the records it
annotates; command-query separation (seeing writes one mark, nothing else). Mental models: inversion (what would make seen
lie: a mark set on navigation or by a hidden tab, so the browser reports only a real view), second-order thinking (a field
on Chain would reorder the board every time he looks).

### The record

- Table `seen(subject TEXT PRIMARY KEY, event_id INTEGER NOT NULL, at INTEGER NOT NULL)` (`at` in Unix milliseconds), one row per subject: a home (a chain's
  id, the Assistant's home) or `assistant.feed`. `event_id` is the newest event of that home the owner viewed (0 for the
  feed), `at` the server's time when he viewed it. A mark only moves forward: a call with an older `event_id` changes
  nothing, so a replayed or late call is harmless. A deleted chain's row goes with its home (`delete_home`).
- The migration creates the table, copies `decidedSeenAt` from the Assistant's document into the row `assistant.feed`, and
  removes the field from the document. `Assistant.decided_seen_at` and `see_feed` are deleted.

### What counts as viewed

- The browser calls `seen_mark(subject)` only when the page is visible and focused (`document.visibilityState` and
  `document.hasFocus()`) and the newest event of the chain's stream (or the newest feed line) has been in the viewport for a
  second. Opening a page in a background tab, or a page whose stream is scrolled up, marks nothing.
- The call is a change with a request id like every other; the server stamps `at`.

### What reads it

- `Wait.unseen` (ADR-0078): the wait began after its home's mark (`since > at`). The browser notifies only for unseen waits.
- `ChainStanding.unseen`: the chain finished after its mark (its last `finish`, `stall` or `goal_failed` decision is newer than
  `at`). The Done column marks it "new"; it clears when he views the chain.
- `ChainView.away`: when the owner opens a chain whose mark is older than its newest decision, the view carries what
  happened since, as the chain's decisions after the mark (who, what, when; at most 8, with how many more), and the waits it
  holds now. The stream shows it at the top as "While you were away · 42 min". It is read once, in `chain_get`; viewing the
  chain moves the mark, so the next visit starts from there.
- `AssistantFeed.unseen` counts the feed's decisions after the `assistant.feed` mark, as it counted after `decided_seen_at`.

## Consequences

- Seen is the same on every tab and device and survives a reload; a background tab never clears a mark.
- The Done column says what finished while he was away without a model call; the recap is as good as the decisions the
  agents write, so better agents write a better recap with no change here.
- One more table and one more call; marks grow with the number of homes, which `delete_home` bounds.

## Tests (written first)

- store: `seen_marks_only_move_forward`: marking event 7 then 5 leaves 7; the migration's copy of `decidedSeenAt` reads back as
  the feed's mark.
- orch: `a_chain_finished_after_its_mark_is_unseen_until_viewed`.

## Revisit when

- Another channel (the Work plugin, iOS, the CLI) shows a chain: it reports viewed by the same call; what "viewed" means on
  that surface is written down there.
- Several people share Remit: the subject gains who saw it (non-goal today).
- The owner asks for a recap in prose: an agent may write it from the same decisions, beside them, never instead of them.
