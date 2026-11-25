# Rust peers in Remit's field

Research date: 2026-11-11. Question: which well-known, high-quality projects in Remit's field are written in Rust, and what can Remit learn from each.

## Method and sources

- **Repo facts** (stars, last push, license, primary language, archived) come from
  `gh repo view <owner/repo> --json stargazerCount,pushedAt,licenseInfo,primaryLanguage,isArchived,description`, run on 2026-11-11.
  The tables give the result. Where `licenseInfo` came back as "other", the license was read from the repo's LICENSE file with
  `gh api repos/<r>/contents/LICENSE`. **HIGH** confidence for these facts.
- **"What it is"** is the repo's GitHub description, plus web search results where cited. **HIGH**.
- **"What Remit could learn"** is inferred from the description and public positioning. I did not read the source of any project.
  **MEDIUM** unless marked otherwise. Before an ADR cites any of these as precedent, read the project's code.
- The tables were made with no repo cloned; all searches used generic topic terms. vibe-kanban and nono were cloned afterwards
  (`git clone --depth 1` into `references/`, read only, never built or run) for the section *What to take from vibe-kanban and nono*.
- URL for every row: `https://github.com/<project>`.

## 1. Coding agents, agent CLIs, agent harnesses

| Project | Stars | Last push | License | What it is | What Remit could learn | Rel. |
|---|---|---|---|---|---|---|
| aaif-goose/goose (was block/goose; `gh` redirects) | 56,445 | 2026-11-11 | Apache-2.0 | Open-source extensible AI agent that installs, executes, edits and tests | MCP-first extension model ("extensions" are MCP servers); recipes/subagents as config. Compare it with Remit's capabilities and playbooks. | HIGH |
| zed-industries/zed | 94,715 | 2026-11-11 | GPL-3.0 / AGPL / Apache-2.0 (LICENSE-GPL, LICENSE-APACHE) | High-performance multiplayer editor with an agent panel | How a Rust host drives external agents over ACP (Claude Code, Codex, Gemini) as subprocesses: permission prompts, diffs and tool calls streamed into a UI. | HIGH |
| agentclientprotocol/agent-client-protocol | 4,619 | 2026-11-10 | Apache-2.0 | "A protocol for connecting any editor to any agent" (Rust crate + schema) | A typed standard for driving Claude Code and others (sessions, prompt turns, permission requests, tool-call updates). It could replace a hand-parsed CLI stream in Remit's `claudecode` crate. | HIGH |
| warpdotdev/Warp | 65,978 | 2026-11-11 | AGPL-3.0 (`gh api .license`) | "Agentic development environment, born out of the terminal" | Multi-agent management UI in a native Rust client; reference for future macOS/iOS clients. Native GPU UI, so limited reuse. | MED |
| antinomyhq/forge | 7,631 | 2026-11-11 | Apache-2.0 | Rust AI pair programmer across 300+ models | A Rust agent loop with a provider abstraction: tool dispatch, context compaction, multi-provider. Relevant if Remit runs agents natively instead of through the CLI. | MED |
| 0xPlaygrounds/rig | 9,112 | 2026-11-11 | MIT | "Build modular and scalable LLM applications in Rust" | The de-facto Rust LLM/agent library: provider traits, tool trait, agent builder. Use it, or learn from it, for any in-process LLM call (judges, analyst). | MED |
| bosun-ai/swiftide | 805 | 2026-11-09 | MIT | Streaming indexing, query and agentic LLM apps in Rust | Agent hooks/lifecycle design and streaming pipelines; smaller community. | LOW |
| jeremychone/rust-genai | 940 | 2026-11-04 | Apache-2.0 | Multi-provider generative AI client | Thin provider-normalisation layer: an alternative to rig when only chat calls are needed. | LOW |
| sigoden/aichat | 10,563 | 2026-02-23 | Apache-2.0 | All-in-one LLM CLI (REPL, RAG, tools, agents) | CLI ergonomics only; stale (last push Feb 2026). | LOW |
| TabbyML/tabby | 33,935 | 2026-06-30 | Apache-2.0 core + `ee/` proprietary (LICENSE) | Self-hosted AI coding assistant | A self-hosted Rust server with an embedded web app and an auth/team model: packaging precedent. Push slowed after 2026-06. | LOW |

## 2. Agent orchestration and agent servers

| Project | Stars | Last push | License | What it is | What Remit could learn | Rel. |
|---|---|---|---|---|---|---|
| BloopAI/vibe-kanban | 28,649 | 2026-11-10 | Apache-2.0 | "Get 10X more out of Claude Code, Codex or any coding agent": Rust backend orchestrating CLI agents in worktrees, with a web UI | **Closest direct peer.** Rust server that spawns Claude Code/Codex per task in a git worktree, streams logs to a web app, and generates TS types from Rust. Compare process supervision, log normalisation across agent CLIs, and the worktree lifecycle. | HIGH |
| zeroclaw-labs/zeroclaw | 33,314 | 2026-11-11 | Apache-2.0 | "Fully autonomous AI personal assistant infrastructure" in Rust | Autonomous agent daemon with channels, tools, memory and scheduling in one binary. Compare its trait-based pluggable providers, channels and tools with Remit's plugin model. | HIGH |
| nearai/ironclaw | 12,717 | 2026-11-09 | Apache-2.0 | "Agent OS focused on privacy, security and extensibility" | Security-first agent runtime: compare WASM-sandboxed tools and credential handling with Remit's jail and egress. **LOW**: I have not seen its design. | MED |
| moltis-org/moltis | 2,953 | 2026-10-23 | MIT | "Secure persistent personal agent server in Rust. One binary, sandboxed execution, ... MCP tools" | Same shape as Remit (one binary, persistent server, sandboxed execution, MCP), at smaller scale. Look at how it packages a sandbox with the server. | MED |
| rivet-dev/rivet | 6,830 | 2026-11-10 | Apache-2.0 | "The orchestrator for agentic workloads": durable actors | Actor-per-entity with durable state: a candidate pattern for Chain-as-actor. | MED |
| majiayu000/harness | 91 | 2026-11-09 | MIT | Rust control plane for Claude Code and Codex fleets: policy, cross-agent review ([GitHub](https://github.com/majiayu000/harness)) | Same problem space (adapters for Claude Code CLI and Codex CLI, cross-agent review). Low stars, so treat as idea source only. | LOW |
| liquidos-ai/AutoAgents | 781 | 2026-10-18 | Apache-2.0 | Multi-agent framework in Rust | Generic multi-agent framework; little overlap with CLI-agent orchestration. | LOW |
| tensorzero/tensorzero | 11,705 | 2026-06-11 | Apache-2.0 | LLMOps: gateway, observability, evals. **Archived** (`isArchived: true`) | Feedback/eval data model for Remit's outcomes and duels. Archived, so read it, do not depend on it. | LOW |

## 3. MCP SDKs, servers and gateways

| Project | Stars | Last push | License | What it is | What Remit could learn | Rel. |
|---|---|---|---|---|---|---|
| modelcontextprotocol/rust-sdk (`rmcp`) | 4,106 | 2026-11-10 | Apache-2.0, older contributions MIT (LICENSE: "licensing transition") | Official Rust MCP SDK | The conventional way to serve Remit's MCP tools from Rust: `#[tool]` macros, streamable HTTP transport. If Remit hand-rolls MCP in `crates/orch/src/tools.rs`, adopting `rmcp` is the framework-conformant path. Verify against Remit's code. | HIGH |
| agentgateway/agentgateway | 6,106 | 2026-11-10 | Apache-2.0 | "Agentic proxy for AI agents and MCP servers" (Linux Foundation) | MCP/A2A proxy with authn/z, RBAC and tool federation: reference for Remit's `/mcp/client` door and for credential injection at a proxy. | HIGH |
| cloudflare/pingora | 27,858 | 2026-11-09 | Apache-2.0 | Library for fast, reliable network services (HTTP proxy framework) | Production-grade Rust proxy framework. A candidate base for the egress proxy if hand-rolled HTTP CONNECT/MITM gets hard. | MED |

## 4. Process sandboxing for untrusted code

| Project | Stars | Last push | License | What it is | What Remit could learn | Rel. |
|---|---|---|---|---|---|---|
| nolabs-ai/nono | 4,926 | 2026-11-10 | Apache-2.0 | Kernel-enforced agent sandbox (Seatbelt on macOS, Landlock on Linux) with a credential-injecting proxy ([helpnetsecurity](https://www.helpnetsecurity.com/2026/07/27/nono-open-source-ai-agent-sandboxing/), [nono.sh](https://nono.sh/os-sandbox)) | **Nearly identical to Remit's jail + egress design.** It hands the agent a phantom credential and the proxy injects the real one. Compare Seatbelt profile generation, Landlock ABI fallbacks, and how the proxy's trust works. | HIGH |
| strands-agents/box | 530 | 2026-11-11 | Apache-2.0 | Rust agent sandbox: OS isolation, default-deny policies, credential injection, Seatbelt and egress ([GitHub](https://github.com/strands-agents/box)) | Same design as nono, from the AWS Strands team; its policy language is worth a look. New (~days old per search), so watch rather than adopt. | HIGH |
| landlock-lsm/rust-landlock | 339 | 2026-11-09 | Apache-2.0 / MIT | Official Rust Landlock library | The canonical crate for Remit's Linux jail: best-effort ABI compatibility handling. | HIGH |
| multikernel/sandlock | 615 | 2026-11-06 | Apache-2.0 | Unprivileged Linux process sandbox: Landlock + seccomp, no container ([arXiv](https://arxiv.org/pdf/2605.26298)) | Unprivileged seccomp + Landlock recipe and HTTP-level rules: a model for a Linux jail with no root. | MED |
| microsandbox/microsandbox | 9,021 | 2026-11-11 | Apache-2.0 | Local-first programmable microVM runtime | The next isolation tier (microVM per agent) once process jails stop being enough: a seam to name in the jail ADR. | MED |
| firecracker-microvm/firecracker | 37,960 | 2026-11-10 | Apache-2.0 | microVMs for serverless (AWS) | Gold standard VM isolation. Linux/KVM only. Reference for the threat model. | LOW |
| hyperlight-dev/hyperlight | 4,795 | 2026-11-11 | Apache-2.0 | Embeddable micro-VMM for running untrusted functions | Function-level VM isolation; a possible alternative jail for workflow scripts. | LOW |
| bytecodealliance/wasmtime | 18,845 | 2026-11-11 | Apache-2.0 | WebAssembly runtime | Wasm as the sandbox for plugins and scripts (capability-based WASI). Relevant to the plugin system. | MED |
| extism/extism | 5,832 | 2026-10-09 | BSD-3-Clause | Framework for loading wasm plugins securely | Ready-made plugin host and PDK across languages: compare with Remit's plugin runner (ADR-0034). | MED |
| containers/youki | 7,676 | 2026-11-10 | Apache-2.0 | OCI container runtime in Rust | Namespace/cgroup code in Rust. Reference only. | LOW |
| cloud-hypervisor/cloud-hypervisor | 6,508 | 2026-11-11 | Apache-2.0 / BSD-3 (LICENSES dir) | VMM for cloud workloads | Reference only. | LOW |
| phylum-dev/birdcage | 219 | 2026-07-06 | GPL-3.0, **archived** | Cross-platform embeddable sandbox (Seatbelt + Landlock) | Past art for the same macOS+Linux split; archived. | LOW |

## 5. Durable workflow and job engines

| Project | Stars | Last push | License | What it is | What Remit could learn | Rel. |
|---|---|---|---|---|---|---|
| restatedev/restate | 4,688 | 2026-11-11 | BUSL-1.1 (LICENSE) | Durable execution platform in Rust | Journal-based durable execution: replay a handler from its event log. Directly relevant to resumable agent turns and idempotent handlers. BUSL, so learn from it, do not embed it. | HIGH |
| temporalio/sdk-core | 548 | 2026-11-11 | MIT | Temporal's Rust core shared by all language SDKs | Deterministic workflow state machines and replay as a sans-IO Rust core shared by many SDKs. The same pattern as Remit's "Rust core, thin clients". | MED |
| obeli-sk/obelisk | 768 | 2026-11-11 | AGPL-3.0 | Durable, deterministic workflow runtime (WASM + SQLite) | Small single-binary, SQLite-backed durable workflows: closest in shape to Remit's agent-written workflows. AGPL. | HIGH |
| windmill-labs/windmill | 18,520 | 2026-11-11 | AGPL-3.0 + Apache-2.0 parts + proprietary EE (LICENSE files) | Self-hostable workflow, script and agent platform; Rust backend | Running user scripts (incl. Deno/Bun JS) safely, schedules, webhooks triggering flows. Mature reference for the signals → run pipeline. | HIGH |
| golemcloud/golem | 1,517 | 2026-11-11 | BUSL-1.1 (LICENSE) | "Secure, reliable runtime for agents ... powered by lightweight" WASM | Durable agents via WASM oplog. Same journal idea as Restate. | MED |
| geofmureithi/apalis | 1,496 | 2026-11-10 | MIT | Type-safe background job library (SQLite/Postgres/Redis backends) | Job queue on SQLite with tower middleware: candidate for retries and scheduling. | MED |
| get-eventually/eventually-rs | 601 | 2026-11-06 | MIT | Event sourcing for Rust | Aggregate/event-store traits: compare with Remit's event-sourced store. | LOW |
| serverlesstechnology/cqrs | 503 | 2026-11-10 | Apache-2.0 / MIT (license reported as "other") | Lightweight CQRS + event-sourcing framework | Same; well-documented aggregate pattern. | LOW |

## 6. Embedded JS runtimes

| Project | Stars | Last push | License | What it is | What Remit could learn | Rel. |
|---|---|---|---|---|---|---|
| DelSkayn/rquickjs | 1,068 | 2026-11-10 | MIT | High-level QuickJS bindings | The standard Rust QuickJS binding. Confirm whether Remit's QuickJS host uses it. Limits (memory, interrupt handler) and async are the parts to study. | HIGH |
| denoland/deno (deno_core now at `libs/core`) | 108,956 | 2026-10-29 | MIT | JS/TS runtime; `denoland/deno_core` is **archived**, and core lives in `deno/libs/core` (`gh api .../contents/libs`) | Ops + permission model (`--allow-net` etc.) as a capability system for scripts. V8 is heavy compared with QuickJS. | MED |
| boa-dev/boa | 7,675 | 2026-11-10 | MIT | JS engine written in Rust | Pure-Rust engine with no C FFI: fits `forbid(unsafe_code)` aspirations at the host level. Slower than QuickJS. | MED |
| trynova/nova | 1,960 | 2026-11-07 | MPL-2.0 | Data-oriented JS engine in Rust | Experimental; watch only. | LOW |
| bytecodealliance/javy | 2,774 | 2026-11-11 | Apache-2.0 | JS → WebAssembly via QuickJS | QuickJS inside wasmtime: double sandbox for agent-written scripts. | MED |

## 7. Local-first sync, typed contracts, transport

| Project | Stars | Last push | License | What it is | What Remit could learn | Rel. |
|---|---|---|---|---|---|---|
| redbadger/crux | 2,832 | 2026-11-10 | Apache-2.0 | Cross-platform apps with a Rust core, thin native shells | **Exact match for "Rust client core, Vue/Swift only render".** Sans-IO core with capabilities/effects, typegen to Swift/TS. | HIGH |
| loro-dev/loro | 6,288 | 2026-11-10 | MIT | CRDTs for collaborative, version-controlled JSON | Only if replicas ever accept offline writes. Remit is server-authoritative, so mostly no. | LOW |
| automerge/automerge | 6,727 | 2026-11-10 | MIT | CRDT; Rust core (JS 4.86 MB, Rust 4.35 MB by `gh api .../languages`) | Same as Loro. | LOW |
| y-crdt/y-crdt | 2,202 | 2026-11-04 | MIT (LICENSE) | Rust port of Yjs | Same. | LOW |
| superfly/corrosion | 1,878 | 2026-11-05 | Apache-2.0 | Gossip-replicated SQLite (Fly.io) | SQLite change streaming / subscriptions to clients: compare with Remit's replica push. | MED |
| vlcn-io/cr-sqlite | 3,829 | 2026-08-10 | MIT | Multi-writer CRDT SQLite | Reference only. | LOW |
| mozilla/uniffi-rs | 5,125 | 2026-11-09 | MPL-2.0 | Rust → Swift/Kotlin bindings generator | The conventional path for the iOS/macOS client core. | HIGH |
| 1Password/typeshare | 2,992 | 2026-11-08 | Apache-2.0 | Sync Rust types to TS/Swift/Kotlin | Contract → Swift + TS from one source: compare with `crates/tsgen`. | MED |
| Aleph-Alpha/ts-rs | 1,948 | 2026-11-09 | MIT | Rust → TypeScript bindings | Same; TS only. | MED |
| specta-rs/specta | 654 | 2026-11-10 | MIT | Export Rust types to other languages | Same. | LOW |
| BiagioFesta/wtransport | 719 | 2026-09-28 | Apache-2.0 | WebTransport server/client in Rust | Candidate driver for `crates/link-quic`. | MED |
| kixelated/web-transport | 275 | 2026-11-11 | Apache-2.0 | WebTransport for native and WASM (MoQ author) | One API across native and the WASM client core. | MED |
| quinn-rs/quinn | 5,343 | 2026-11-10 | Apache-2.0 | QUIC in Rust | Underlying QUIC for both above. | LOW |
| n0-computer/iroh | 12,965 | 2026-11-10 | Apache-2.0 | QUIC + NAT traversal, dial by key | Mentor devices and workers reaching the server without a public IP. | MED |

## 8. Other relevant

| Project | Stars | Last push | License | What it is | What Remit could learn | Rel. |
|---|---|---|---|---|---|---|
| vectordotdev/vector | 22,850 | 2026-11-10 | MPL-2.0 | Observability data pipeline | Source → transform → sink with typed config and back-pressure. A model for signal ingestion (doors, sensors). | MED |
| tursodatabase/turso | 25,214 | 2026-11-11 | MIT | SQLite-compatible DB rewritten in Rust | Async SQLite in pure Rust with CDC. Watch for a future store backend. | LOW |
| nushell/nushell | 41,087 | 2026-11-11 | MIT | Structured-data shell | Not directly relevant; dropped from the shortlist. | LOW |

## Top-8 shortlist (ranked by what Remit can learn)

1. **BloopAI/vibe-kanban**: the most direct peer. A Rust server orchestrating Claude Code/Codex in worktrees with a web UI. Study process supervision and normalising the streams of different agent CLIs. (HIGH relevance; MEDIUM on specifics, code not read)
2. **nolabs-ai/nono** (+ **strands-agents/box**): the same jail + credential-injecting egress design as Remit, in Rust: Seatbelt/Landlock profiles and the phantom-credential proxy. (HIGH)
3. **agentclientprotocol/agent-client-protocol** (+ **zed**): a typed standard for driving Claude Code and other agents as subprocesses. A candidate to replace a bespoke CLI-stream parser. (HIGH relevance; MEDIUM that it fits Remit's needs)
4. **modelcontextprotocol/rust-sdk (rmcp)**: the framework-conformant way to serve MCP tools from Rust. (HIGH)
5. **restatedev/restate** (+ **obelisk**, **temporal sdk-core**): journal/replay durable execution for resumable, idempotent agent turns and workflows. BUSL/AGPL, so learn from them, do not embed them. (MEDIUM)
6. **redbadger/crux** (+ **uniffi-rs**): a sans-IO Rust app core with thin Swift/TS shells, which is Remit's client-core direction. (HIGH relevance)
7. **windmill-labs/windmill**: self-hosted Rust platform where webhooks/schedules trigger user scripts and agents. A mature reference for the signal → run pipeline and safe script execution. (MEDIUM)
8. **aaif-goose/goose** (+ **zeroclaw**): large Rust agent products. Goose for its MCP-as-extension model, zeroclaw for its autonomous-daemon plugin traits. (MEDIUM)

## Coverage

- **Checked: 74 named repos** via `gh repo view`, plus `gh search repos` sweeps and 2 web searches
  ([openalternative](https://openalternative.co/categories/ai-coding-agent-orchestrators/using/rust),
  [awesome-agent-orchestrators](https://github.com/andyrewlee/awesome-agent-orchestrators), [strands box](https://github.com/strands-agents/box),
  [nono](https://nono.sh/os-sandbox), [Zujkowski](https://williamzujkowski.github.io/posts/2026-07-02-agentic-ai-sandbox-secret-proxying-gap/)).
- **Listed: 63.**
- **Dropped: 11 named repos**, plus search-only hits that were not confirmed.
  - Not Rust-primary (`primaryLanguage`):
    - sst/opencode: TypeScript
    - Aider-AI/aider: Python
    - hatchet-dev/hatchet: Go
    - rocicorp/mono (Zero): TypeScript
    - electric-sql/electric: TypeScript
    - google/nsjail: C++
    - winfunc/opcode: TypeScript (Tauri shell)
    - anthropic-experimental/sandbox-runtime: TypeScript
  - Do not exist: zortax/sandbox, coplane/jean.
  - Excluded by brief: openai/codex.
  - Too small or unverified: the durable-workflow search hits sayiir (81★) and others under 10★; Zaivern Code, Omnigent and the "ai-sandbox" crate (seen only in search snippets, not confirmed on GitHub).
- **Archived but kept, flagged:** tensorzero, birdcage, deno_core (moved into the deno repo).
- **Limits:**
  - No source code was read, so every "learn" column is **MEDIUM** inference from descriptions and search results.
  - Stars and push dates are a snapshot from 2026-11-11.
  - Sub-area (2) is a fast-moving space. The awesome-agent-orchestrators list was not walked entry by entry, so smaller Rust orchestrators may be missing.

## What to take from vibe-kanban and nono

Read 2026-11-11 from shallow clones at `references/vibe-kanban` and `references/nono` (untrusted: read only, nothing built or
run). Paths below are relative to `references/vibe-kanban/crates/`, `references/nono/` and, for Remit, `crates/`. Coverage: the
files named below, larger ones in part. Not read: vibe-kanban's frontend and task model; nono's `connect.rs`, `route.rs`,
`linux_namespace.rs` and CLI; Remit's `egress.rs`, `forward.rs`, `inject_rules.rs`.

### vibe-kanban: process supervision and normalising agent output

| Take | Their code | Remit today | Conf. |
|---|---|---|---|
| **Drive Codex through `codex app-server`**, if Remit drives Codex one day: `initialize`, then `account/read` (no account: raise "sign in" before any thread), then `thread/start`, or `thread/fork` to resume, then `turn/start`; a turn ends at `turn/completed`; approvals for file changes and commands are server requests. The `account/read` check before a session is the piece to copy into `CodexLauncher`. | `executors/src/executors/codex.rs:447-453,594-631,600-604`; `executors/src/executors/codex/client.rs:309-414,874` | no Codex (ADR-0006) | HIGH |
| **Answer every server request, never drop one.** They answer `ChatgptAuthTokensRefresh`, MCP elicitation and permission requests with `null` and a warning. `codexapp` should refuse each by name with an error so a thread never waits on Remit. | `executors/src/executors/codex/client.rs:432-440` | none yet | HIGH |
| **Interrupt before killing, then escalate the group:** SIGINT, SIGTERM, SIGKILL to the whole process group, 2 s apart, stopping at ESRCH. | `utils/src/process.rs:5-30`; the stop flow at `local-deployment/src/container.rs:1394-1466` | the group is killed with SIGKILL at once (`procgroup/src/procgroup.rs:142`) | HIGH |
| **Keep the raw lines; normalise with a pure function you can replay.** Each raw protocol line is copied to a second pipe and stored as JSONL per process; normalised entries are rebuilt from it on reload. For a second harness this is how `agent` can prove both map to the same `Update`s: replay recorded lines through each mapper in tests. | `executors/src/stdout_dup.rs:23-34`; `utils/src/execution_logs.rs:14-21`; `services/src/services/container.rs:834-860` | Claude Code's transcript is the record; no raw capture (`docs/research-codex/notes/runtime.md`, idea 7) | HIGH |
| **One normalised entry type across agents,** with a tool's status carrying `PendingApproval`, `Denied` and `TimedOut`, and a file edit carrying its unified diff. A check list for what `agent::Update` and `ToolCall` must express once Codex items flow through them. | `executors/src/logs/mod.rs:70-254` | `agent::Update` (`agent/src/agent.rs:61-72`) | HIGH |

Where Remit is already ahead: after a crash vibe-kanban only marks running rows failed and stores no process group, so leftovers
are never killed (`services/src/services/container.rs:277-331`); Remit derives what to resume from the store and its runtime
kills its whole tree on stop (ADR-0058; `orch/src/recover.rs`). vibe-kanban has one Codex login and no pool
(`executors/src/executors/codex.rs:261-271`). HIGH.

### nono: Seatbelt, Landlock and the credential proxy

| Take | Their code | Remit today | Conf. |
|---|---|---|---|
| **Refuse control characters in a Seatbelt path,** not only escape `\` and `"`. | `crates/nono/src/sandbox/macos.rs:288-304` | `jail/src/sandbox_darwin.rs:117-121` escapes two characters (validation refuses NUL only) | HIGH |
| **Landlock and seccomp for a session that keeps the host network.** nono probes the Landlock ABI down to V1, masks rights to it, opens rule paths with `O_PATH`, and adds seccomp denying `io_uring_setup`, raw, UDP and netlink sockets. Remit's Wrap mode has no syscall filter, and no fallback where unprivileged user namespaces are off. | `crates/nono/src/sandbox/linux.rs:491-600,744-777,829-858,2245-2253` | user and mount namespaces; no `.rs` under `crates/` mentions seccomp (`rg -il seccomp crates`); `jail/src/plan_linux.rs:52-61` | HIGH |
| **`GIT_SSL_CAINFO` beside the other CA variables,** so `git` inside a jail trusts the injecting proxy. | `crates/nono-proxy/src/config.rs:298-302` | four variables, no git's (`orch/src/script.rs:25-30`) | HIGH |
| **A phantom key for SDKs that refuse to start without one:** the child gets a session token as its API key and a local base URL; the proxy checks it in constant time and swaps in the real secret. | `crates/nono-proxy/src/server.rs:698-739`; `crates/nono-proxy/src/token.rs:1-20`; `reverse.rs:671-706` | the proxy overwrites the auth headers; the child holds no key (`egress/src/inject.rs:52-53`) | HIGH |
| **Secrets held as `Zeroizing`** in the proxy's memory. | `crates/nono-proxy/src/credential.rs:36-47` | plain `String` (`egress/src/inject.rs:53`) | HIGH |
| **Apply the profile with `sandbox_init` through FFI** rather than `/usr/bin/sandbox-exec`: typed errors, no deprecated binary. It needs `unsafe`, which the workspace forbids, so it waits for a reason stronger than the error text. | `crates/nono/src/sandbox/macos.rs:939-976` | `jail/src/plan_darwin.rs:10,89` | HIGH (fact), MEDIUM (worth it) |

Where Remit is already ahead or equal: Remit's per-run CA is limited to one host and its key dropped (`egress/src/inject_cert.rs:22-29`)
against nono's 24-hour session CA (`crates/nono-proxy/src/tls_intercept/ca.rs:1-60`); Remit's private-address check covers more
ranges (`egress/src/policy.rs:170-208` against `crates/nono-proxy/src/filter.rs:96-135`); macOS network rules are the same shape
(`jail/src/sandbox_darwin.rs:51`, `crates/nono/src/sandbox/macos.rs:828-883`). HIGH.

The nono rows are hardening of the jail and the egress proxy (rails-and-sandbox area): per AGENTS.md they are built when the
owner asks. The vibe-kanban rows on driving Codex wait until Remit supports a second harness.
