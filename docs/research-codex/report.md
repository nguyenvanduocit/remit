# What Remit learns from Codex and its Rust peers

Study of 2026-11-11. Goal: Remit becomes an excellent product. Codex (`references/codex/codex-rs`, Apache-2.0) was read in
both directions: Remit's crates mapped onto Codex's, and Codex read on its own terms for engineering Remit lacks. Remit
learns designs; it does not depend on Codex crates (they pin forks and Codex-specific types). Small files (Seatbelt
`.sbpl`) may be copied with the Apache notice.

Notes, each claim with `file:line` and confidence:

| Area | File |
|---|---|
| Sandbox, egress, secrets, exec policy | [notes/rails.md](notes/rails.md) |
| MCP tools, skills, plugins, hooks | [notes/tools-skills-plugins.md](notes/tools-skills-plugins.md) |
| Sessions, recovery, workflow engine, memory | [notes/runtime.md](notes/runtime.md) |
| Contract, link, clients, CLI | [notes/protocol.md](notes/protocol.md) |
| Lints, tests, CI, observability | [notes/engineering.md](notes/engineering.md) |
| Other Rust projects in the field | [notes/peers.md](notes/peers.md) |

## 1. Defects found (fixed now, not hardening)

| # | Defect | Evidence | Status |
|---|---|---|---|
| D1 | macOS Wrap: renaming a hidden path's parent makes it readable | reproduced by the orchestrator: `mv w w2 && cat w2/data/secret.key` prints the secret under a profile shaped like `crates/jail/src/sandbox_darwin.rs` `hidden_profile`; Codex denies `file-write-unlink` on ancestors (`sandboxing/src/seatbelt.rs:1013-1021`) | fixed: `holders_stay` in `crates/jail/src/sandbox_darwin.rs:99`; jail tests pass on macOS and Linux |
| D3 | A key-result check can write `.git` in its worktree and run code outside the jail at the next server `git` | `crates/orch/src/repochecks.rs:167-190` says read-only, worktree passed writable | fixed: the check writes the worktree, never its `.git` (`repochecks.rs:193`, read-only wins in `mounts_linux.rs:50`); ADR-0021 updated |
| D4 | A writing agent session leaves its worktree's `.git` writable; the server's next `git` runs its hooks outside the sandbox | `crates/orch/src/policy.rs:157`, `crates/claudecode/src/conn.rs:515` | fixed: `.git/config`, `.git/hooks` and worktree links read-only for every session (`crates/orch/src/policy.rs` `git_controls`) |
| D2 | Linux Wrap may read hidden files via `/proc/<pid>/root` | `crates/jail/src/namespaces_linux.rs:65-68`: no pid namespace or own `/proc` | not a defect: the Wrap's own user namespace refuses `/proc/<pid>/root` (run in a Linux container, ADR-0021) |
| P1 | A resume sent right after a pause can land first; chain ends paused, both calls succeed | `crates/orch/src/lifecycle.rs:414-445` | fixed: per-chain line in orch (`request.rs` `act_on_chain`) and `order = id` in `link::contract!` |
| P2 | A draft saved from two tabs silently replaces the other's text | `crates/orch/src/drafts.rs:198-228` (orch's own test shows it) | fixed: compare-and-set on the draft's revision, typed conflict, merge in the replica (migration 0099) |
| R1 | QuickJS out-of-memory is a catchable error; a script's `try/catch` swallows it | `quickjs.c:8554-8563` in rquickjs-sys 0.14.0 | fixed: 64 MiB heap, OOM stops the run even inside try/catch; a heap full to the last bytes throws null, which needs an unsafe allocator hook |

## 2. Ideas ranked by value to the product

"Work" = makes a thing work (build now). "Hardening" = waits for the owner's ask, seam recorded.

| Rank | Idea | Area | Value | Effort | Kind |
|---|---|---|---|---|---|
| 1 | Server sends typed agent activity (tool groups, turn end); browser stops parsing strings (`web/src/stream.ts:23-66`) | protocol | HIGH | M | work (server-driven rule) |
| 2 | Tools declare read-only / always-load; Claude Code runs read-only tools in parallel (`crates/orch/src/tools.rs:57-71`) | tools | HIGH | S | work |
| 3 | Refresh a live session's tool list on grant instead of restarting the Chair (`capability_request.rs:269`) | tools | HIGH | M | work, test first |
| 4 | Structured tracing: spans per chain step and per turn, OTLP export (0 spans today; `crates/remit/src/main.rs:125-130`) | runtime + eng | HIGH | M | work |
| 5 | `clippy.toml` `disallowed-methods` makes "one owner" compile-checked (SQLite only in `store`, spawn only via `procgroup`/`childenv`) | eng | HIGH | S | work |
| 6 | Snapshot tests of the full brief an agent reads (today `contains` checks, `prompts.rs:389-520`) | eng | LOW | M | dropped: owner 2026-11-11 keeps only rail, compatibility and main-path tests (AGENTS.md *Few tests*) |
| 7 | Codex as a second provider via `codex app-server` (judge independence, failover) | runtime | HIGH | L | not pursued: owner 2026-11-11, Remit does not support Codex now |
| 8 | External MCP servers as a source behind the grant gate | tools | HIGH | L | ADR |
| 9 | Escape tests for the jail (Codex has many; Remit 3) — would have caught D1–D3 | rails | HIGH | M | rail tests, kept fast (AGENTS.md *Few tests*); D1–D3 now, rest hardening |
| 10 | `remit update` (one idempotent go-live command) and `remit-client call/watch/snapshot` | protocol | HIGH | M | work |
| 11 | Edits by quoted context / revision check instead of whole-record rewrites | tools | MED-HIGH | M | work |
| 12 | Read rate-limit `utilization` / `resetsAt`, not only "rejected" (`crates/agent/src/claude.rs:500-519`) | runtime | MED | S | work |
| 13 | Fork / rewind a session (`--fork-session`, `--resume-session-at`) | runtime | MED | M | work |
| 14 | Long reports keep head and tail, say how much was cut (`prompts.rs:220-228`) | tools | MED | S | work |
| 15 | Memory use counted; unused memories age out | runtime | MED | M | work |
| 16 | Typed failure detail; typed reason for a cut turn | protocol + runtime | MED | S | work |
| 17 | Line tables in release builds; `redundant_clone` lint; macOS CI job; CI `timeout-minutes` | eng | MED | XS–S | work (timeouts: hardening) |
| 18 | Own Seatbelt base profile instead of `bsd.sb`; fcntl/TIOCSTI/XPC denies; seccomp; process non-dumpable | rails | MED | M | hardening |
| 19 | Placeholder credentials with routes, so `gh`/`aws` run jailed without the real secret | rails | MED | M | hardening |
| 20 | Debug-log ring buffer; `remit doctor`; README flag table generated from `flags::defs` | runtime + protocol | MED | S | work |

## 3. Where Remit is already better (keep)

Crash-safe recovery from the store; journaled, replayable workflows; moving a live session to another account; replay
safety typed on every changing call; questions to the owner as inbox records; a sans-IO link; resolved-address egress
gate; sealed secrets bound to their slot; no `unsafe`; stricter workspace lints; a `syn` fitness crate; mutation testing
on changed lines; ADRs with *Revisit when*; capability rails visible to the owner.

## 4. Not to adopt

Codex's own model loop, `apply-patch` and file search (Claude Code supplies them); a feature-flag registry that keeps removed
flags; layered enterprise config; Bazel; `#[ctor]` global test mode; automatic test retry; `anyhow` in libraries; JSON-RPC
framing and v1/v2 side by side; switching the workflow engine to V8 (ADR-0075 stands).

## 5. Other Rust projects to clone into `references/`

From [notes/peers.md](notes/peers.md) (metadata from `gh`, learn-points from descriptions only):
BloopAI/vibe-kanban (closest peer: Rust server running Claude Code and Codex in worktrees), nolabs-ai/nono (Seatbelt +
Landlock + credential-injecting proxy, same design as `jail` + `egress`), agent-client-protocol (typed protocol for
driving agent subprocesses), redbadger/crux (Rust core, thin shells), windmill (webhook/schedule-triggered scripts),
restate / obelisk (durable execution; licenses make them references only).

## 6. Gaps in Remit found on the way

210 `Result<_, String>` outside tests against the typed-error rule (engineering.md); worker reports are not fenced as
untrusted text (tools-skills-plugins.md, C).
