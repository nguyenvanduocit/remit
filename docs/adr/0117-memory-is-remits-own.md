# 0117. Memory is Remit's own: a librarian writes it, words, meaning and links find it, a cross-encoder judges it, each turn is given it, and a review keeps it whole

Status: accepted, 2026-11-19. Source: the owner, 2026-11-19 (translated): "implement it in Remit, I want it inside Remit, use
its good engineering, do better than that to build the memory, provide embedding, graph and a full database", then "rewrite
all of it, I need it long-term, as good and as polished as it can be", "use OpenRouter, do not be afraid of the cost", "look
at more rerank and embedding models". Supersedes ADR-0063's backend (the owner's memory system run as a child daemon, with
its Claude Code plugin) and the recall path ADR-0112 names as path 4; keeps ADR-0063's gate (only what people and verdicts
teach), ADR-0088's learners and ADR-0082's record of what a session recalled.

## Context

On the owner's instance on 2026-11-19 (`memory report -days 14` over `~/.remit/memory`):

- **Nothing reaches the agents.** Of 86 prompts the daemon's hook recalled for, 3% got a memory; of 109 file reads, none;
  of 19 session starts, 26%. Of 38 memories then, 16 were ever shown.
- **Meaning is off.** No embedding key was configured, so no memory had a vector: recall was words only, and Vietnamese
  prompts against English memories share no words.
- **Teachings are lost.** 8 of 32 admitted sessions failed extraction at the daemon's spend cap and were dropped.
- **The Assistant learned nothing** (5 admits, 0 memories), and the owner's plain requests reached the mentor's memory as
  teachings ("Create a project for …").
- **The scope is the person, not the work.** Every memory belongs to one employee; what it learned of a repository helps
  neither the next employee on it nor fits the same employee on another one. ADR-0108 made the project the durable context.
- **Nothing keeps it whole.** Duplicates pile up (two of the owner's 45 memories say "record_verdict needs a clean
  worktree", cosine 0.915), a contradiction stays recalled on both sides, and nobody learns that a memory misled.
- **Nobody can change it.** The backend's source is on no machine and no remote: only the installed binary is left.
- **The owner cannot see recall.** The plugin's hook adds memories inside the runtime, where Remit neither decides nor
  records them (ADR-0112 *Context*, path 4).

Measured for this decision (2026-11-19, the owner's 45 memories as `title. body`, every model through OpenRouter's
`/embeddings` and `/rerank`). The queries, with the memories that answer them: "cargo fmt changed files I did not touch"
(the `cargo fmt -p` pitfall); "how should I write the verdict of a branch review" (five verdict lessons); "viết commit
message cho hiệu ứng mới thế nào" (two commit-message lessons); "kiểm tra hiệu ứng sprite bằng mắt từng khung hình" (three
frame-inspection lessons); "record_verdict fails with uncommitted changes" (two); "golden files conflict when merging two
effect branches" (two); "mục tiêu không rõ ràng" (the owner's objective pitfall); "benchmark numbers look slow, maybe other
processes are running" (two); "acceptance criteria for a slice so the implementer cannot cheat" (three); "random noise in
sprite effects must be deterministic" (one); "add a new pixfx effect, where do I register it" (two); "tôi có nên chạy song
song các bước QC không" (one); and five about nothing they hold: "fix the login page css", "deploy the kubernetes cluster to
production", "rotate the database password", "thời tiết hôm nay thế nào", "write a SQL migration adding a column".

| Embedding model | Known answer in first 10 | Weakest best answer | Strongest unrelated | Gap |
| --- | --- | --- | --- | --- |
| `qwen/qwen3-embedding-8b` (instructed query) | 12/12 | 0.659 | 0.371 | **0.288** |
| `perplexity/pplx-embed-v1-4b` | 12/12 | 0.463 | 0.172 | 0.291 |
| `mongodb/voyage-4-large` | 12/12 | 0.486 | 0.207 | 0.279 |
| `openai/text-embedding-3-large` | 12/12 | 0.399 | 0.243 | 0.156 |
| `google/gemini-embedding-001` | 12/12 | 0.728 | 0.564 | 0.164 |
| `cohere/embed-5-pro` | 12/12 | 0.447 | 0.429 | 0.018 |
| local `multilingual-e5-base` (fastembed) | — | 0.84 | 0.798 | 0.04 |

| Reranker | Best answer first | Weakest best answer | Strongest unrelated |
| --- | --- | --- | --- |
| `qwen/qwen3-reranker-8b` | 12/12 | **0.843** | **0.000** |
| `cohere/rerank-5-pro` | 12/12 | 0.247 | 0.008 |
| `cohere/rerank-4-pro` | 12/12 | 0.767 | 0.517 |
| `mongodb/rerank-3` | 12/12 | 0.624 | 0.294 |
| `nvidia/nemotron-3.5-rerank-8b:free` | 12/12 | 0.051 | 0.032 |

Every model found the answers; what separates them is whether a score says *relevant* on its own. Only the Qwen3 reranker
draws a line with nothing on the wrong side, and that line is what lets a turn be given memories unasked without noise.
A local model was tried first: `multilingual-e5-base` left 0.04 between relevant and unrelated, and `bge-m3` was killed for
memory (exit 137) embedding 45 memories on the Mac.

## Options

1. **Rebuild the daemon elsewhere and keep the plugin.** The same split, a second language and process, recall still
   invisible in the runtime, and the store of record outside the backups and migrations Remit owns.
2. **Memory in Remit, extraction a library call to a model, embeddings local.** Remit makes no model call to the owner's
   Claude quota outside a session (a hidden caller, no stream, no failover, no stop), and the local models measured above
   cannot tell relevant from unrelated.
3. **Memory as Remit's records and rules; extraction an agent's work; retrieval wide, then judged; a review keeps it whole.**
   The records live in the store beside what they cite; a built-in librarian reads each teaching through a gated tool and
   writes; words, meaning and links find candidates, a cross-encoder judges them; the core gives each turn what was judged
   relevant and records it; agents rate what they were given; a daily review asks the librarian to merge, settle and
   retract. Embeddings and reranking are OpenRouter's (the owner chose it, cost no object). **Chosen.**

## Decision

- **The record** (`model::Memory`, migrations 0120 and 0122): id, holder (an employee, a mentor, `assistant`), template,
  project, where it was learned (`work`, the chain's title; `chain` and `card`, their ids; `source` and `source_label`, the
  teaching named and said in words), kind (`lesson`, `pitfall`, `preference`, `principle`, `decision`, `intent`, `fact`,
  `procedure`), scope (`own` < `project` < `lineage` < `everyone`), trust (`auto`, `confirmed`), `end` (`model::MemoryEnd`:
  active, superseded by a memory, or retracted, each ended one with when and why: a sum type, written as the flat record
  `MemoryRecord` whose JSON the screens read, a record saying two things at once refused when it is read), title, body,
  evidence, teacher, `about` (subjects, parsed once by `model::subject_of`), links, `retaught`, and time. A memory is never
  rewritten to say something else; a correction supersedes it and the record keeps what was believed when.
- **Its use is counted beside it.** `recalled`, `last_recalled_at`, `helped` and `misled` (unsigned) live in `memory_use`,
  counted by an upsert and read into the record by a join: a rating never rewrites the document, its words or its edges;
  a write of the document never loses a count; and a forget or a promote item compares only the claim (title, body, scope,
  status), so it still applies after the memory was recalled. Store: `memories` (its words keyed by its own row `n`, so a
  write rewrites the FTS5 row by key and only when the words changed), `memory_text` (FTS5, `unicode61 remove_diacritics 2`,
  bm25 weights title 4, body 2, evidence 1, subjects 2), `memory_embeddings` (per memory and model, with the hash of the
  passage it was made from), `memory_unembedded` (a passage the model refused, left a day), `memory_edges` (links and
  subjects, derived, not exported), `teachings` (holder and card columns, each indexed).
- **Readers.** `store::MemoryReader` (holder, template, project) and one SQL clause decide what an agent may recall: its
  own, its project's, its lineage's, everyone's, active. Never another employee's own. Every card that remembers (an
  employee's, the Assistant's) is given `search_memory` and `rate_memory`.
- **Teachings** (`model::Teaching`): the gate stays deterministic (job `teach`, ADR-0063's six subscriptions); role
  `accepted` (a whole session), `teaching` (only what was said to the agent), `review` (no new knowledge). A session
  teaching holds the range it taught (`from_event` to `to_event`, the last event when recorded). An overturn retracts only
  what the overturned turn taught its holder: the active, unconfirmed memories written by that holder's session teaching
  whose range holds the turn (`librarian::taught_by_turn`).
- **The librarian writes** (a chain of kind `librarian`, ADR-0113, with capability `memory_keeping`): `next_teachings`,
  `read_teaching`, `recall_for`, `remember`, `mark_retaught`, `link_memories`, `retract_memory`, `settle_teaching`. The rails,
  in code: the platform sets holder, project, place, source, teacher and trust; scope at most `project`; evidence required;
  each write re-reads the teaching in the transaction that writes, so a settled teaching writes nothing. **Reach:** a
  teaching changes only its holder's own memories and its project's (`librarian::writable`); a correction of a lineage's,
  everyone's or another holder's memory is written beside it with a `contradicts` link, which the coach reads and settles.
  **Once:** a memory's id derives from its teaching and its claim, so a remember run twice writes once. **Not twice:**
  `remember` embeds the new passage as `embed-memories` does and refuses while a memory the holder may recall that may
  say the same (*amended 2026-11-20, ADR-0118:* the decision layer judges the memories at cosine 0.65 or more; the review's duplicate rule, cosine 0.84 or more, stays as the System Two when no decider answers) is not named (in
  `links`, `supersedes`, or `distinct_from`); it stores the vector with the memory, so the next remember reads it. Without
  the models it checks none. *Amended 2026-11-19:* it first refused every memory over `TURN_FLOOR`, what bears on the
  passage as a turn's message; one memory bore on every lesson of a teaching and 10 of 10 remembers were refused, each
  naming more. Relevance is not a claim said twice: what only bears on it is `recall_for`'s and `links`'. Under a review a memory must supersede what it
  joins (into a scope at least as wide as theirs, never a confirmed one) or rest (`because`) on two or more (an
  abstraction).
- **Retrieval is wide, judgement is one model.** A turn's queries (`recall::turn_queries`): its message, its work in one
  line (the card's brief), and the subjects its message names (paths, backticked names, crates, tools), looked up as what
  memories are about. Each query's words (FTS5) and meaning (the nearest 24 by cosine under `EMBED_MODEL`,
  `qwen/qwen3-embedding-8b`, the query instructed) give an order, the subjects one more, and links one step over the graph;
  reciprocal rank fusion (k 60) joins them. `RERANK_MODEL` (`qwen/qwen3-reranker-8b`) reads the work and the message after an
  instruction beside each memory's passage (`Memory::passage`: kind, claim, body, subjects, project, the work that taught
  it): every memory the agent may recall while they are 256 or fewer, else the best 32 fused. Score: relevance × trust
  (confirmed ×1.25) × strength (`recall::strength`: the Beta posterior of helped against misled, (helped+1)/(helped+misled+2),
  centred on 0.5 and bounded to ±25%, its praise fading with a 90-day half-life of disuse; being recalled adds nothing).
  `TURN_FLOOR` 0.5 and `SEARCH_FLOOR` 0.05 are measured with that reranker. With no judge (no key, an error, or no answer in
  2 s on a turn) a turn takes only what words and meaning both found, the meaning at cosine `MEANING_FLOOR` 0.55 or nearer
  (0.18 over the strongest unrelated, 0.11 under the weakest answer), or what links to such a one; a search the fused
  order. Words drop English and Vietnamese stopwords. The ranking is pure (`crew::recall`); the models are one port the
  crew declares (`MemoryModels`, failing as `ModelError`: no key is quiet, a refused account or input is said at warn).
- **Each turn is given what bears on it.** After `take` (ADR-0112's call site) the core asks `Domains::recall`: at most 5
  memories new to the session, as a note on the message the agent reads first (framed as evidence, its words escaped by
  one helper, `memory::quoted`), the same note on the event the owner sees. Once it is stored, what was given is counted
  (`Crew::count_given`: `memory_use.recalled`, `last_recalled_at`, `fact:memory.recalled`), and a search's finds are
  recorded as searched. `rate_memory` records helped or misled (`fact:memory.used`), and a memory that misled more than it
  helped goes to the librarian at once.
- **Recall is just in time.** Beside a turn's message, a tool call that names a subject (`PostToolUse`: the file, its
  crate, the program; at most 3, `crew::tool_recall`) and a subagent's start (`SubagentStart`, for its own employee) are
  given what bears on them. Each is bounded so recall never holds the agent up (a turn 2.5 s, a tool call 3 s, Stop and
  Pause honoured meanwhile), each memory is given once per session (a message sent again carries its ids; a compaction
  clears them), and what was given counts only once its note is stored or handed over.
- **Kept whole.** Job `embed-memories` (10 minutes) embeds each active memory whose passage has no vector under the current
  model: one pass at a time, a vector stored only while its hash matches the memory's passage, a batch the model refuses
  asked one by one and a passage it refuses alone left a day. Job `review-memories` (daily, `crew::consolidate`, pure
  `reviews`) records a review teaching for memories that may say one thing (each writable by the other's holder, cosine
  0.84 or more), contradictions both sides of which stand, what misled more than it helped, four or more one reach holds
  about one subject with no abstraction yet, and what no turn was given for 180 days; its id names the memories and what
  changes it, so a settled review is not asked again until something does.
- **Recall is measured** (`crew::recall_eval`, in the daily job, as `fact:memory.recall-quality`, read with
  `read_signal`): of the week's facts, `recall_at_5` (of the memories an agent said helped, the share its turns were given
  unasked), `precision` (of those given and rated, the share that helped) and how often a session searched out what it was
  not given; and the frozen questions above (`recall_fixture.json`, 17), each judged against every active memory as a
  turn's message is: answers in the first 5, answers under the floor, the weakest answer, the strongest unrelated score and
  how many unrelated memories cleared the floor. A question whose answers are no longer active is left out and counted. A
  change of a model, a floor or the passage is measured before and after it by both.
- **Forget** takes only memories the employee holds (whatever their scope or status), compares their claim, and removes
  with each its words, vectors, use and the links other memories hold to it; a memory it superseded keeps its end and says
  its successor was forgotten.
- **The key** is read from `-openrouter-key-file` (default `<data>/openrouter.key`) at each call and hidden from every session.
- **The owner sees it** as a `wire::app::MemoryBook` (`employee_memory`, `card_memory`, `mentor_memory`): what the agent
  recalls now, what its holder once held with when and why it ended, where each was learned, and its teachings with the
  librarian's notes.
- **Removed:** `crates/memoryd`, `chains::Error::Backend`, the flags `-memory-bin`, `-memory-plugin`, `-memory-key-file`,
  `agent::Memory`, `Spec::memory_project`, `SessionSpec::memory_project` (`RUNTIME_CONTRACT` 4), `wire::app::Memory`. At
  start the daemon's database at `<data>/memory/memory.db` is imported once, read and moved on blocking threads (projects
  are holders; scope `project` own, `stack` lineage, `global` everyone; state `confirmed` confirmed, `rejected` retracted;
  a memory already stored is kept as it is) and moved to the backups.

## Consequences

- One database holds memory and what it cites: backups, restore, export and migrations cover it (vectors, the text index
  and the edges are rebuilt, not exported; use is exported).
- A turn waits for an embedding call and a rerank call before it starts (about 1 s each, p50, measured), 2 s at most: past
  that it is given what words and meaning agree on.
- Memory's meaning depends on OpenRouter; without it memory still works by words and links, and a turn is given only what
  two signals agree on.
- Every teaching is an agent turn on the owner's Claude accounts, shown in the librarian's chain; extraction can no longer be
  dropped by a spend cap. A remember costs one embedding call more, the price of not writing a lesson twice.
- What a turn was given is in its stream: the owner sees which memories each turn read, and which helped or misled.
- A correction the librarian may not apply stands as a contradiction both sides of which are recalled until the coach
  settles it: the review asks the librarian too, which may only retract its own side.
- Migration 0122 embeds every memory again (the passage changed), drops the use from each document into `memory_use`, and
  says where each memory was learned from its card or teaching. A forget or promote item drafted before it compares the
  whole document and so reads as moved: it is drafted again.

## Revisit when

- Tool recall reads the memories about a call's subjects through `memory_edges` on each call: an index held in memory
  replaces it once tool calls outpace the store.
- Memories pass ~50,000 per reader: the vector step scans candidates in Rust and the review compares pairs; an ANN index
  (sqlite-vec, HNSW) goes behind `readable_vectors` and `active_memories`.
- A better embedding model or reranker appears on OpenRouter: `fact:memory.recall-quality` before and after; move
  `EMBED_MODEL` or `RERANK_MODEL` with `TURN_FLOOR` and `MEANING_FLOOR`; `embed-memories` re-embeds under the new name.
- The frozen questions stop bearing on the owner's memories (`left_out` grows): write new ones from his recalls that
  helped.
- A memory about a file goes stale with the file: not built. The seam is `about` (`file:` subjects) and the review's
  findings; a finding that compares a file memory with the commits that touched the file since is the next one.
- Agents need what happened in a past card, not only what was learned from it: not built. The seam is the memory's `card`
  and `chain`; an episodic index of cards beside `memories` is the next store.
- Hardening is asked for: memory is a channel into every later turn (OWASP LLM01, LLM08). The seams: `Teaching.source`,
  `MemoryTrust`, `rate_memory`, `remember`'s duplicate check. Not built yet: marking a teaching that quoted outside content
  as untrusted and recalling it apart, a cap on what one teaching may write, a key the owner enters through a write-only
  inbox item instead of a file.
- ADR-0112's turn context is built: memory becomes one provider of it, its note one block.
- Next year's models keep their own memory: the records stay (they are what people taught); the librarian and recall go.
