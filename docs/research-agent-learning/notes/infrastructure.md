# Per-employee memory infrastructure for Remit: research findings

Goal (owner): find out how AI employees can learn knowledge, experience and taste from humans. This slice covers only the per-agent layer: storage, retrieval, embeddings, write gating, consolidation and decay.

Coverage: about 28 web searches and 11 page fetches, plus two local files (`memory/DESIGN.md`, `memory/internal/embed/embed.go`). No files were edited.

## 1. Retrieval: is vector search needed at one employee's scale?

**What the products do**
- **Claude Code memory** loads the first 200 lines or 25KB of a `MEMORY.md` index into every session and reads topic files "on demand using its standard file tools." The docs mention no embeddings. (https://code.claude.com/docs/en/memory) **HIGH**
- **Claude Code dropped RAG.** Confirmed from the primary source, the Latent Space episode of 7 May 2025 (https://www.latent.space/p/claude-code, [00:45:30]). **HIGH** that he said it; the "by a lot" comparison itself is unpublished. Boris Cherny says:
  - "very early versions of Claude actually used RAG… we indexed the code base, and I think we were just using Voyage."
  - Dropped because "the code drifts out of sync" and of "security issues because this index has to live somewhere."
  - "we landed on just agentic search… it outperformed everything. By a lot."
  - "at the cost of latency and tokens."
- **Anthropic memory tool:** a client-side, file-based `/memories` directory. With context editing it reported +39% on an internal eval. (https://www.anthropic.com/news/context-management) **HIGH** that the claim is made; the eval is vendor-run.
- **Codex:** memories are off by default, stored under `~/.codex/memories_v2/`, and built in the background from idle threads, with secrets redacted. A `consolidation_model` exists, but its method is not documented. (https://learn.chatgpt.com/docs/customization/memories.md) **HIGH**
- **Cursor** removed the Memories UI in 2.1 and pointed users to Rules. Staff answers conflict on whether the backend still runs. (https://forum.cursor.com/t/are-my-memories-gone/144057) **MEDIUM**
- **Devin** proposes Knowledge items from chat feedback. The user edits, regenerates or dismisses each one before it is saved. (https://docs.devin.ai/product-guides/knowledge) **HIGH**
- **Mem0** (April 2026) moved to add-only extraction. Semantic search picks the candidates; BM25 and entity matches boost the ranking. Graph memory moved to their paid platform. (https://docs.mem0.ai/migration/oss-v2-to-v3.md) **HIGH**
- **Letta:** a GPT-4o-mini agent with file tools, including a semantic `search_files`, scored 74.0% on LoCoMo against Mem0's then 68.5%. Vendor-reported. (https://www.letta.com/blog/benchmarking-ai-agent-memory) **HIGH**
- **A-MEM:** Zettelkasten-style notes with embeddings, LLM links and "memory evolution". NeurIPS 2025. (https://arxiv.org/abs/2502.12110) **HIGH**

**Benchmark controversy**
- LoCoMo: Mem0's paper scored Zep at 65.99. Zep's correction gave 75.14. A third party recomputed 58.44, saying the adversarial category had been included. (https://blog.getzep.com/lies-damge-lies-statistics-is-mem0-really-sota-in-agent-memory/) **MEDIUM**
- Mem0 now self-reports 91.6 on LoCoMo and 93.4 on LongMemEval, with no reproduction.
- **Vendor scores cannot rank systems.** **MEDIUM**

**Lexical vs vector vs hybrid**
- Public results are self-reported and mixed. **MEDIUM:** a weak arm can hurt RRF.
  - BM25-only: R@5 97.0 on LongMemEval-S.
  - Hybrid beat BM25-only 95.2 vs 86.2.
  - Naive RRF lost to vector-only 0.435 vs 0.739, but on only n=23. (https://dev.to/sattyamjjain/measure-your-retrieval-default-our-hybrid-rrf-lost-to-plain-vector-search-ad0)
- **The owner's own eval:** keyword-only hit@3 was 60.0%; adding vectors at floor 0.45 gave 80.0% (`DESIGN.md:233-234`). **HIGH** on his dataset. The broader point, that embeddings help with paraphrase and mixed Vietnamese/English text, rests only on that eval. **MEDIUM**

**Answer**
- At tens to a few thousand notes, no ANN index is needed. Brute-force cosine in RAM is enough, and it is what the owner runs (`DESIGN.md:11`). **HIGH**
- The primary path is a short index in the prompt plus agentic search, with hybrid search as the tool behind it. **MEDIUM**

## 2. Embeddings

**Models**
- **EmbeddingGemma 2:** 740M parameters in total (270M text model), 8K context, 768 dimensions with Matryoshka truncation. MTEB Code v1 is 78.68 at 768d and 76.18 at 256d; Multilingual v2 is 61.36. (https://huggingface.co/google/embeddinggemma-2) **HIGH**
- **Qwen3-Embedding-8B:** MTEB Multilingual 70.58, dimensions 32–4096, 32K context. The model card has no Code score (https://huggingface.co/Qwen/Qwen3-Embedding-8B). **HIGH**
  - A blog figure of "≈80.7 Code" exists (https://regolo.ai/embeddinggemma-2-vs-qwen3-embedding-benchmarks.md). **LOW**
- **voyage-code-3** beats text-embedding-3-large by 13.80% over 238 code datasets, on Voyage's own suite rather than MTEB (https://huggingface.co/voyageai/voyage-code-3). **MEDIUM**
- **The owner's choice** is `baai/bge-m3` via OpenRouter (`embed.go:22`, `DESIGN.md:43`). It is multilingual, and isolated projects are never embedded (`DESIGN.md:99`). **HIGH**

**Storage in Go without cgo**
- `modernc.org/sqlite/vec` bundles sqlite-vec v0.1.9 CGo-free and is enabled with a side-effect import (https://pkg.go.dev/modernc.org/sqlite@v1.58.0/vec). **HIGH**
- A `BLOB` column plus a Go cosine loop is enough at this scale. **HIGH**

**Local inference from Go**

| Option | How it runs | Caveat | Source |
|---|---|---|---|
| `goccy/go-llama` | llama.cpp translated from WASM, no cgo, `Embed()` API | 4 GiB cap | https://pkg.go.dev/github.com/goccy/go-llama@v0.4.3 (**MEDIUM**) |
| `hybridgroup/yzma` | purego, no cgo | Needs llama.cpp shared libraries plus a GGUF file; native embedding support unconfirmed | https://github.com/hybridgroup/yzma (**HIGH**) |
| `knights-analytics/hugot` | ONNX; ORT backend needs the onnxruntime shared lib | A pure-Go GoMLX backend is reported, but sources conflict | https://awesome.ecosyste.ms/projects/github.com%2Fknights-analytics%2Fhugot (**MEDIUM**) |

**Re-embedding**
- Store `model` and `dim` per vector, keep the text as the source of truth, and compare only vectors from the same model.
- The client sends 64 texts per request (`embed.go:23`), so 3,000 notes is 47 requests. **HIGH**, arithmetic.
- Wall time is not measured.

## 3. Memory record shape

- **Types**
  - LangMem uses semantic, episodic and procedural memory, storing procedures as rewritten prompt instructions (https://blog.langchain.com/langmem-sdk-launch). **HIGH**
  - The owner uses `principle | decision | intent | fact` (`DESIGN.md:95`). **HIGH**
  - Remit needs four kinds (**MEDIUM**, design inference):
    - **preference/taste**, inferred per edit as PRELUDE/CIPHER do
    - **fact**
    - **procedure**, which graduates into a skill
    - **lesson**, a judged strategy or pitfall
- **Provenance**
  - The owner's system keeps an `evidence` quote and a `transcript_ref` (`DESIGN.md:100`).
  - Remit adds the chain/card, the validating verdict and the teacher.
- **Trust is separate from lifecycle.** `state auto|confirmed|rejected` vs `status active|superseded|done|abandoned` (`DESIGN.md:96`). **HIGH**
- **Bi-temporal fields**
  - Graphiti keeps `valid_at/invalid_at` and `created_at/expired_at`, and a contradiction closes the old edge (https://mintlify.com/getzep/graphiti/concepts/temporal-model). **MEDIUM**
  - LongMemEval: "naive time-agnostic memory designs perform poorly on temporal reasoning" (https://arxiv.org/abs/2410.10813). **HIGH**
  - `valid_from`, `valid_until` and a `supersedes` edge give this benefit without a graph store. **MEDIUM**

## 4. Write gating and quality

- **Poisoning is practical.** MINJA injects through ordinary queries with over 95% injection success, and sanitization was largely ineffective (https://arxiv.org/abs/2503.03704). **HIGH**
- **Judged failures help.** ReasoningBank: 49.7 vs 46.5 success rate with successes only (https://arxiv.org/abs/2509.25140). **MEDIUM**, figures from a secondary summary.
- **Human approval** is the shipping pattern: Devin's suggest, edit or dismiss flow. **HIGH**
- **Consolidation**
  - Letta's sleep-time agents report about 5x less test-time compute (https://letta-home-site.pages.dev/research/). **MEDIUM**
  - The owner's dream job is a working Go reference: cosine ≥ 0.70 pairs, judge confidence ≥ 0.85 to supersede, idempotent, never rejects (`DESIGN.md:158,162`). **HIGH**
- **Context budget**
  - Claude Code warns near the index limit. **HIGH**
  - The owner's floors cut injected items per prompt from 4.4 to 2.9 at the same hit@3 (`DESIGN.md:232-234`). **HIGH**
- **Decay is a design choice, not a research finding.** No study was found. Memories leave through supersede, `valid_until`, or deletion the owner approves.

## 5. Evaluation

- **Public benchmarks**
  - LongMemEval covers five abilities, with a 30% drop for commercial assistants. **HIGH**
  - LongMemEval-V2 (ICML 2026): the best system scored 74.9% on Small. **MEDIUM**
  - Evo-Memory uses task streams (https://arxiv.org/abs/2511.20857). **HIGH**
  - MemoryAgentBench (ICLR 2026). **HIGH**
- **Proposed gate for Remit** (**MEDIUM**, a design proposal): a chronological replay of one employee's chains with memory on, off and shuffled. Measure:
  - judge pass rate
  - owner overturn rate
  - **repeat-correction rate**: how often the owner has to give the same correction twice
  - recall hit@3 and items per prompt, as `DESIGN.md:257-267` does

## Recommended design for Remit (per-employee layer)

1. **The server owns typed SQLite rows, and agents write through an MCP tool.** Each row carries kind, `state`, `status`, `valid_from/valid_until`, `supersedes`, evidence and provenance. The markdown index is rendered from the rows.
   - *Rejected:* Claude Code writing memory files directly. That path has no gate, no provenance and no trust state, and it gives one concept two owners. **MEDIUM–HIGH**
2. **Retrieval: a budgeted index in the prompt (about 200 lines or 25KB) plus one `memory_search` tool backed by FTS5 and brute-force cosine, fused by RRF with floors.**
   - *Rejected:* an ANN index, sqlite-vec or an external vector DB first. They add a dependency with no gain below about 10k notes. Cherny's staleness and security reasons apply to a separate index too. **HIGH**
3. **Embeddings sit behind an `Embedder` interface, defaulting to bge-m3 via an API, with `model` and `dim` per vector and a background re-embed when the model changes.** Local inference comes later as a plugin; EmbeddingGemma 2 at 256d is the strongest small candidate.
   - *Rejected:* inference in the core binary, because of shared libraries or a 4 GiB WASM cap.
   - *Rejected:* voyage-code-3 or Qwen3-8B as the default, because the memory is mostly prose and their code numbers are vendor or blog figures. **MEDIUM**
4. **Writes are gated by teacher and verdict.**
   - A human correction is stored as `confirmed`.
   - An agent's self-lesson is stored as `auto`, only after the verdict, and failures only as judged pitfalls.
   - Text from outside sources is never memorized verbatim.
   - *Rejected:* agent writes in the middle of a task, because of MINJA and SWE-Bench-CL. **HIGH**
5. **Run a nightly, idempotent dream job per employee.** It dedupes, supersedes at judge confidence ≥ 0.85, closes expired facts and proposes skills; the owner approves deletion.
   - *Rejected:* Mem0 add-only, which lets contradictory taste rules both rank. **MEDIUM**
6. **Gate memory per employee on chronological replay and the repeat-correction rate.**
   - *Rejected:* public benchmark scores, which are disputed and do not match an employee's work. **HIGH**

## Assumptions and remaining items

- "Assumption: one employee holds under 10k notes, because the brief states tens to a few thousand."
- **Re-embed wall time:** needs a paid OpenRouter run with the owner's key. Owner: whoever builds the embedder in decision 3, as its first benchmark.
- **Background tasks `a70e331f3e8907fcd` and `ab7cf34a9d9bed738`:** this subagent did not start them, has no tool to see them, and none of its findings depend on them. Owner: the parent agent, which should wait for both to finish before writing its own final report.
