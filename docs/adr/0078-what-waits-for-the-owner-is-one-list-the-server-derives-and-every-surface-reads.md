# 0078. What waits for the owner is one list the server derives, and every surface reads it

Status: accepted, 2026-11-11; `merging` removed by ADR-0114; its Merge and Put live waits removed by ADR-0087: an agent merges and puts live, and the list
holds only what no agent decides for the owner. Source: the owner's agreement of 2026-11-11 to `docs/research-peers-product/report.md`
(proposals 1 and 6, the first defect) and its deck (slides "Change A", "A1", "A5"). Narrows ADR-0067: its three endings
stand, and the word for the second is this record's. Refines ADR-0001: the merge requests it counts are this list's.

## Context

The customer is the owner, who has to know at a glance what needs him and act on it. Today the server says it four ways
for one state, and counts it two ways (all read, HIGH):

- The board card of a chain held up by an inbox item reads "Blocked" (`crates/orch/src/standing.rs:183,194`, `Attention::label`),
  a finished chain waiting on a merge reads "Waiting on you" (`standing.rs:338,407`, `Ending::label`, `outcome_of`), the
  agent row reads "Needs you" (`crates/orch/src/agentstanding.rs:350-366`), the chain header "Running".
- The card says why in one string (`ChainStanding.why`, the first item's title or the Chair's MR rating reason,
  `standing.rs:226-262`) but not what kind of thing waits: a question, a permission, a failure, an MR.
- The board places a chain in the column that waits for the owner when it has an open inbox item, an open MR, or a
  finished outcome that waits (`standing.rs:153-154`) and skips archived chains (`standing.rs:91`). "MRs to review" sums
  open MRs over every chain (`crates/outcomes/src/outcomes.rs:158-161`) read by `Reader::chains`, which has no archive
  filter (`crates/store/src/store.rs:497-498`). Admission (`crates/orch/src/admit.rs:281`), the scheduler
  (`schedule.rs:153`), the watchdog (`watchdog.rs:39,102`) and the board's held-for-review line (`views.rs:662`) read
  the same sum. An archived chain's open MR is counted and shown nowhere.
- A failure already waits for the owner: `report_error` raises an `error` inbox item (`crates/orch/src/inbox.rs:22-56`),
  so the board already counts it. What is missing is that the card says so.

Archiving is the owner putting a finished chain away: "the lists leave it out, nothing else changes. Its MRs are still
followed, and a review comment or a message from the owner reopens it" (`crates/orch/src/archive.rs:157-158`). Deleting a
chain with an open MR is refused with "merge or close it first, or archive the chain" (`crates/orch/src/step.rs:707-712`):
archiving with an MR open is the path Remit offers for "I will not review this here".

## Options

1. **A new entity**: a `waits` table, a row per thing that waits, written when an item opens or an MR opens and closed when
   answered, with its own notification. It stores what is already stored (inbox items, MR states): two records of one fact,
   which drift (the archived MR defect is exactly that drift between two readers).
2. **Fix the words and the filter where they are**: rename the labels, add `archived_at.is_none()` to `outcomes::waiting`.
   Smallest, but the count and the column stay two functions that agree by care, and the card still cannot name the kind.
3. **One derived list, computed once from the records, carried in the board**: the server's pure function lists every wait
   from inbox items and chains; the column, the count, the card, the top bar, admission and the browser's keys all read it.
   It grows only with what is open (it shrinks as the owner answers), so it is a view, not an entity (AGENTS.md: what grows
   without bound gets its own table).
4. **Archived MRs wait too**: the list keeps them, and the board shows an archived chain in the column. It reverses what
   archiving means and what the delete refusal offers.

## Decision

Option 3, with archived chains' MRs out of the list. Patterns: a materialized view (derived, never written by hand, one
function owns it), single source of truth. Mental models: inversion (what guarantees the count and the board disagree: two
functions computing them, so there is one), first principles (a wait is a fact already stored; the list is a reading of it).

### The list

- `crates/orch/src/waiting.rs` (new, pure) holds the rule: `waits_of(chains, inbox, verdicts, owners) -> Vec<Wait>`, and
  `chain_waits(c, items, verdicts, owners) -> Vec<Wait>` for one chain, which `waits_of`, `column_of` and admission call.
  Nothing else decides what waits.
- A **wait** is one of:
  - every open inbox item, wherever it lives (a chain, archived or not, the Assistant, none): an agent or Remit is held on
    the owner's answer, so archiving never hides it;
  - every open MR of a chain that is not archived (`kind: merge`);
  - a finished chain, not archived, whose ending waits on putting its merged MR live (`outcome_of`'s `waits` with no open
    MR left, `kind: live`).
- Each wait carries its kind as an enum and as the server's word, who it is about, the ask in words, since when, the Chair's
  rating for a merge, whether the owner has seen it (ADR-0079), and for a merge the next steps the owner can send (ADR-0080).
- **Order**, the one order every surface shows: what holds an agent up first (`asks`, `approve`, `failed` and the other
  inbox kinds), then merges by the Chair's rating, then `live`; oldest first within each. The Needs column orders its chains
  by their first wait in that order, so the column and the list agree.
- **Kinds and their words** (`WaitKind` → `Wait.label`): question `asks` "Asks"; permission `approve` "Approve"; error
  `failed` "Failed"; change `apply` "Apply"; credential `key` "Key"; login `sign_in` "Sign in"; exception `decide` "Decide";
  held `held` "Held"; open MR `merge` "Merge"; merged, not live `live` "Put live".

### One word

- The state is **Needs you**, everywhere: the column ("Needs you"), a chain's badge in it, an agent's badge, the chain
  header, the top bar ("3 need you"). The badge beside it is the kind of its first wait; the line is that wait's ask.
- `Attention` and its label (Blocked, High, Medium, Low) are deleted; the MR rating stays as `Wait.priority`. `ChainTone`
  loses `blocked`, `high`, `medium`, `low` and gains `needs` and `failed` (the tone of a chain whose first wait failed).
- ADR-0067's second ending keeps its rule and loses its word: a finished chain whose merge or live mark is left stands in the
  Needs column with the badge `Merge` or `Put live` and the MR as its line. `Ending::label`'s "Waiting on you" is deleted.
- An agent's line names its first wait as "{label}: {ask}" ("Approve: Bash psql …"); "Waiting for your permission" and
  "Waiting for you" are deleted.

### Carried in the board, counted from it

- `Board.waiting: Vec<Wait>` is the list, pushed in the same `board_updated` as the columns, so the column, the count and the
  list are one value. `Board.needs_you` stays the count of cards in the Needs column (chains plus loose items), which the top
  bar shows. "MRs to review" is the number of `merge` waits in `Board.waiting`.
- `Workload.mrs_waiting` and `outcomes::waiting` are deleted. Admission (ADR-0001), the scheduler, the watchdog and the
  held-for-review line count `merge` waits through `waiting::merges(chains)`, the same `chain_waits` rule. An archived
  chain's MR no longer holds back new work: the owner put it away.
- `ChainStanding` gains `wait: Option<Wait>` (its first wait) and `waits: i64` (how many of the list are its own), and
  loses `level`, `alert` and `waiting_since`.

### Getting to it

- The browser's palette (⌘K), go-to keys and the key that jumps to the next wait (`n`) read `Board.waiting` in its order.
  A palette row goes somewhere, watches, or does one of the browser's verbs (prompt, answer, pause, stop, start); Enter on
  a row never edits the domain. The last row sends the typed words to the Assistant as the owner's message
  (`assistant_prompt` through a draft), which is talking to an agent.
- A browser notification is shown only for a wait that arrives in `Board.waiting` unseen while the page is open, batched into
  one notification per push, tagged so two tabs show one. Nothing else notifies.

## Consequences

- Pill, column, card, top bar, admission and keys cannot disagree: each reads `chain_waits`.
- The card says what waits without opening it; a failure reads as a wait like any other.
- The board push grows by one entry per open wait; a busy board holds tens, which the push already carried as item ids.
- Archived chains with an open MR stop counting against `-max-waiting-mrs`.
- The palette searches what the replica holds (the snapshot leaves archived chains out); an archived chain is found through
  the Archived column's search, as today.

## Tests (written first)

- orch: `waits_are_one_list_the_column_and_the_count_read`: a chain with an open question, one with an open MR, one archived
  with an open MR and one archived with an open error item; the Needs column holds the first two, `Board.waiting` holds the
  question, the MR and the error item, `waiting::merges` counts one.

## Revisit when

- A second owner or a team shares the board: a wait gains an assignee (non-goal today, `docs/design.md`).
- Notifications reach the owner off the browser (a phone channel): the batching rule moves into the client core
  (`crates/replica`) so every channel batches alike.
- The palette must search archived chains: a server search call, not a replica read.
