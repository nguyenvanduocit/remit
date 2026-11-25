# 0086. The Assistant asks where the owner is

Status: accepted, 2026-11-12.

## Context

Each message the owner sends to the Assistant carries where he is (`Where`: the page and what is open on it). The server
named it in words and put `[The owner is looking at …]` ahead of his words; a new session read it after its recap
(`placed_lead`). On 2026-11-12 the owner opened a piece of feedback (`/feedback/fb_60b71ac8ff7e`) and asked "is this
feedback still valid?". The Assistant read `[The owner is looking at the app.]` and answered that it could not tell
which feedback he meant and had no tool to read the Feedback tab. Two gaps caused it:

- The web app's `whereOf` named no id for the Feedback, Objectives, Employees and Mentors pages, and the server's page
  names lacked those pages and Duels, so the label fell back to "the app".
- The Assistant held no tool that reads feedback (`list_feedback` was only a chain capability's) or an objective.

A label also says only *where*; it does not say what can be read or done there, so the Assistant still had to guess
which tool and which manual applied.

## Options

1. **Fill in the label and keep putting it ahead of his words.** Fixes the fallback; every message still carries a line
   the Assistant may not need, and the line still says nothing about what can be done there.
2. **A tool the Assistant calls, reading where he wrote from (chosen).** The message stores `Where` as data, as before;
   `where_is_owner` answers it with the label, the manual reference for that page and the tools that read it.
3. **A tool reading where he is now (presence).** The browser reports every navigation to the server. The Assistant
   answers what he wrote, and he may move while it works; several tabs need a rule for which one counts. It adds a
   record and a push per navigation to answer a question the message already answers.

## Decision

- The owner's message to the Assistant stores `{placed: {where, label}}` when he moved since the last message that said it
  (the chip under it), and nothing is put ahead of his words. `placed_lead` is gone: a new session asks like any other.
- `where_is_owner` (the Assistant's, read only) answers the newest place his messages said: `where` (the ids), `label`,
  `manual` (the skill reference that says what can be done there) and `reads` (its tools that read what the page shows).
  One table, `PAGES` in `crates/orch/src/assistant.rs`, names every page of `web/src/router.ts` with its words, manual and
  reads; a test fails when a page names a tool the Assistant does not hold.
- The brief tells the Assistant to call it before it answers "this", "here", "it" or words that could mean more than one
  thing.
- `Where` names the open feedback, objective, employee and mentor; the Assistant holds `read_feedback` (one piece by id,
  marking nothing), `list_feedback` and `read_objective`, and the manual has `references/feedback.md`.

## Consequences

- A message carries the owner's words only; the Assistant spends one tool call when it needs the place.
- Whether the Assistant asks rests on its judgement and the brief: a model that does not ask answers "this one" blind.
- A new page needs a row in `PAGES` and a branch in `whereOf`, or the tool answers its page with the whole manual and no
  reads.

## Revisit when

- The Assistant answers about the wrong thing because the owner moved between writing and its answer: then add option 3's
  presence beside the stored place.
- A second channel (CLI, iOS) names places the web app does not: `Where` and `PAGES` then belong in the contract.
