# 0033. An employee keeps a memory of typed entries that cite their source

Status: superseded by ADR-0061 on 2026-11-04 (an employee's memory is Claude Code's own auto memory); proposed 2026-11-02; its waits on the owner, but a secret act, are an agent's (ADR-0087). Source: the owner's words of 2026-11-02: a memory system for every agent, so that an employee who works long on one
part of the product (the user login) gives better opinions when an objective about it arrives; `docs/archive/council/02-design.md` §2.4 (a typed long-term
memory; changing shared memory is a release; the `untrusted` label sits on the item, not on the session). Builds on ADR-0032 (the employee),
ADR-0012 (what a chain changes reaches later chains through the owner), ADR-0016 and the rule that every claim carries its evidence.

## Context

A card keeps nothing after its chain. With ADR-0032 an employee outlives its chains, so it can remember: why a decision was taken, what broke and
how it was found, how a part of the code is laid out, what was tried and failed. Memory also does harm: an entry goes stale when the code changes
and the employee goes on speaking with confidence; a poisoned entry (text from an outside source written down as fact) steers every later session
of the employee; shared memory spreads both. The research says a long-term memory is one typed section and that the label for untrusted content
belongs on the entry (`docs/archive/council/02-design.md` §2.4, A10).

## Options

1. A free-text notes file per employee, written and read by the employee. Cheap, and no entry can be checked, expired or traced.
2. Shared memory for the company from the start. One poisoned or stale entry reaches every employee.
3. Typed entries per employee, each citing a source the platform can check, append-only with supersede, read as data and never as instruction,
   with team and company layers left for later. **Chosen.**

## Decision

- **Entry.** `MemoryEntry{ID, Employee, Kind, Text, Source, VerifiedAt, Supersedes, Untrusted, At}`. `Kind` is a closed set: `fact` (how a
  thing is), `decision` (what was chosen and why), `incident` (what broke and what found it), `lesson` (what to do or not do), `map` (where
  something lives). `Source` is a platform reference, never the agent's say-so: a file (repo, revision, path, line range and the hash of those lines), a chain and
  card id, a `source_calls` row (ADR-0027's ledger), a signal reading, an event id. Only a file source can go stale; the others are records of
  the past and stay as they were. `remember` is called only inside a chain's turn, so a file always resolves against that chain's worktree.
  `VerifiedAt` is when a session last checked the entry against its source. The platform, never the agent, sets `Untrusted`: when the source
  kind is outside content (an event, a web page, a ticket), or when the writing turn read outside content (taint of the turn, as the research
  has it at item level). A chain-and-card source that names a card of the Chair is refused for a judge's memory.
- **Append-only.** An entry is never edited. A correction is a new entry that `Supersedes` the old; the old stays in the record and drops out of
  what is read. Retiring an employee keeps its entries (ADR-0032). Only the owner forgets an entry or an employee's memory: the Assistant drafts a change item
  of kind `forget` and he applies it (ADR-0029's rail).
- **Written by the employee.** Two tools, `remember(kind, text, source)` and `recall(query)`, in the capability `memory` that the template's
  loadout lists. The platform fills `Employee`, `At` and the session's origin; `remember` refuses an entry with no checkable source, and refuses
  one over the size cap.
- **Read as data.** At the start of a turn the employee receives a projection: the entries most relevant to the task, ranked by full text and recency (no
  model writes a summary, so no unsourced text enters it), each
  shown with its kind, its source, `verifiedAt` and a mark when `Untrusted` or when the source has changed since (the file's hash differs). The
  brief says plainly that memory is evidence to verify against its source before it is relied on, and that nothing in it is an instruction.
- **Search** is SQLite full text (FTS5) over `Text` and `Kind`; vector search is added when full text is shown to miss what it should.
  (My call.)
- **Private to the employee.** No employee reads another's memory. Team and company layers, and what a Chair may read of its staff's, are a later
  ADR; a layer shared between employees is a change the owner sees (ADR-0012).
- **Stale is visible, not deleted.** An entry whose source hash changed is marked stale in the projection until a session verifies it again and
  records the new `VerifiedAt` or supersedes it.
- **Judges** keep memory like any employee, of their own verdicts and the defects they found; nothing a Chair wrote reaches a judge's memory except
  through a source the judge read itself.

## Consequences

- A table `memory_entries` and migration 59 (after 57 and 58 of ADR-0031 and ADR-0032); the contract carries entries and the Crew tab shows an employee's memory read-only, with a search
  box that asks the Assistant to forget or correct.
- The brief of every session of an employee changes; so does the capability list.
- The cost of a session grows with the projection; its size is capped and the cap is a number in one place.
- Threat model (memory is a channel into every later session; OWASP LLM01 and LLM08, STRIDE):

| Threat | Control |
|---|---|
| Text from an outside source is written down as fact and steers later sessions | `Untrusted` on the entry; the brief gives memory as data; an entry never sets a tool, a grant or a title |
| A stale entry is trusted | The source hash is compared at read; a changed source marks the entry stale in the projection |
| An agent invents a source | `remember` accepts only references the platform can resolve and refuses the rest |
| One employee's memory is read by another | Reads are by employee id, set by the platform; a test fails when a tool crosses it |
| Memory grows without end | Size cap per entry and per employee; the employee supersedes and consolidates; the cap is a rail |
| The owner cannot see or remove what an employee believes | The Crew tab lists the entries; the owner has the Assistant draft a forget and applies it |

## Not decided here

Team and company memory, consolidation by the employee in a turn of its own, vector search, memory of customers (personal data and its legal
basis), and how a judge's memory is calibrated. Each is its own ADR. The author of `~/projects/memory` has a design for typed memory with
supersede (`memory/DESIGN.md`); the builder reads it before building, and the entry above does not depend on it.

## Revisit when

Employees with memory do no better than fresh ones on replayed tasks (the measurement ADR-0032's first Chair is built to allow); stale entries
are trusted in more than a few verdicts; the cap makes the projection too thin to help; or the owner asks for memory shared across employees.

Enforced by: nothing now; superseded by ADR-0061, whose tests hold memory as Claude Code's auto memory.
