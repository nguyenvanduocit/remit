# 0081. Every outcome leaves a mark, in plain words

Status: accepted, 2026-11-11. Source: the owner's agreement of 2026-11-11 to `docs/research-peers-product/report.md`
(proposal 4, journeys J6 and J7) and its deck (slide "A4").

## Context

The customer is the owner reading why something stopped and what he can do about it, and later reading what was done and by
whom. Read in code (HIGH unless marked):

- A failure reaches him as an `error` inbox item whose title is `"{name} failed: {first line of the raw error}"` and whose
  detail is the rest (`crates/orch/src/inbox.rs:22-56`). The raw text is whatever the failure said: a turn's `end.failed`
  string (`crates/orch/src/turn_end.rs:62-127`), a session that could not open (`session.rs:913-924`), a turn cut by a restart
  (`recover.rs:325`). Its agent's line repeats it as `"Error: {card.error}"` (`crates/orch/src/agentstanding.rs:351,371`).
  The report saw `Chair failed: {"code":-32603,"message":"Internal error: You've hit your session limit…` on the retry page
  (remit-17; seeded at `web/verify/retry.ts:37`). The Rust runtime still passes such text through: a failed turn's `why` is
  `err.to_string()` of the `agent::Error` (`crates/orch/src/session.rs:698-700`), carried as `TurnEnd::failed`
  (`session.rs:821`) into `report_error`'s title (`turn_end.rs:127`, `inbox.rs:31-45`). A Claude Code result with `is_error`
  and no typed provider failure becomes `Error::TurnFailed` holding the result's error strings joined verbatim
  (`crates/agent/src/claude.rs:788-791`). Only a provider failure typed by `claude_failure` (`claude.rs:617`) is spared: it
  goes to failover (`session.rs:808-810`) and makes the card wait for the provider (ADR-0005, ADR-0040).
- A key result in a change item reads `check: {"kind":"submit",…}`: `describe_objective` writes the check and the surrogates
  as stored JSON (`crates/orch/src/objective_change.rs:110-123`).
- "A newer version, reload the page" shows with no Reload button (`web/src/components/ConnectionBanner.vue`, report remit-19).
- Outcomes are recorded as decisions with who did them: `archive`, `unarchive`, `pause`, `resume`, `stop`, `finish`, `reopen`,
  `mr`, `ship` (the Chair's merge of a stage), `live` (`model::DecisionKind`, `crates/model/src/model.rs:783-860`). The stream
  shows the asks and verdicts in the order they happened (`web/src/stream.ts:62-75`) but not these. `chain_archive` takes no
  reason (`crates/wire/src/app.rs`, contract).

## Options

1. **The browser rewrites errors** (parse JSON, map codes). A rule in the browser, repeated by every channel.
2. **An agent explains each failure.** The agent that failed often cannot run (a limit, a closed session), and the owner
   waits on a model for a line he needs now.
3. **The server words a failure once, where it is raised, from its typed cause**, keeping the raw text as detail; known
   causes get a sentence and the next actions that exist; the rest say plainly that the cause is not known.

## Decision

Option 3. Patterns: anti-corruption layer (outside error shapes stop at the edge where they enter), functional core (the
wording is a pure function of a typed cause). Mental models: inversion (what guarantees JSON on screen: a raw string carried
to the title, so the title is never the raw string), audience (the owner reads the cause and the next step, the details are
for whoever debugs).

### Failures

- A failure is typed where it is raised: `turn_end`, `fail_to_open` and recovery hand `report_error` a `Failed { cause, text }`
  (crate-private in `crates/orch`), not a string. `cause` is a closed set: a provider's limit, access or service failure (with
  the reset time when known), a session that could not open, the agent's process that ended, a turn cut by a restart, a turn
  the harness ended with an error, and `unknown`.
- A pure `failure_words(who, &Failed)` writes the item's title, one sentence that names who and the cause ("The Chair stopped:
  this Claude account hit its session limit"), and its first detail line, what the owner can do ("It resets at 22:00." /
  "Retry runs the same message again."). The raw text follows in the detail after a blank line, under "Details", and is never the
  title. An `unknown` cause says "The Chair stopped on an error Remit does not recognise", with the raw text as detail.
- The item's options stay the actions that exist (Retry when there is something to retry, Dismiss). No option is offered that
  nothing performs.
- `Card.error` holds the sentence; the agent's line reads the wait's label and ask (ADR-0078), so no line shows raw text.

### Change items

- `describe_objective` words a key result's check and the surrogates with `check_words` (one sentence per check kind: "passes
  when a merge request of the chain is merged and its checks are green"), never as stored JSON. Every other `ChangedField`
  writer already words its values.

### Outcomes in the stream

- The chain's stream shows, in the order they happened, the decisions of kinds `archive`, `unarchive`, `pause`, `resume`,
  `stop`, `finish`, `reopen`, `mr` (merged or closed), `ship` and `live`, each as one line with who did it (the owner, the
  Assistant for him, the Chair, Remit) and its why when it has one: "Merged !12 by the Chair for you · stage 5%", "Archived by you
  · duplicate of Backfill zone groups". `ChainView.decisions` already carries them; the server's decision text is the line.
- `chain_archive(id, why)` and `chain_unarchive(id, why)` take a reason as `chain_pause` does, stored on the decision (empty
  allowed).
- The newer-version banner carries a Reload button.

## Consequences

- No raw error reaches a title, a badge or an agent's line; whoever debugs still reads it in the details.
- A cause Remit does not know reads as unknown, honestly, and the raw text says what it was.
- Every merge, stop and archive is visible in the chain's stream with who and why.

## Tests (written first)

- orch: `a_failure_title_is_a_sentence_and_its_raw_text_is_detail`: a turn that fails with a JSON error text raises an item
  whose title holds no `{` and whose detail ends with the text.

## Revisit when

- A cause recurs as `unknown` in real runs: it gets its own variant and sentence (the error items Remit stores show which).
- The owner asks for an explanation in prose: the Assistant reads the item and its details on his question, from the panel.
