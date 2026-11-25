# Codex vs Remit: cross-cutting engineering

Scope: how each codebase is engineered as a whole (lints, tests, CI, observability, build, config, docs). Domain areas
(sandbox/egress/secrets, tools/skills/plugins/MCP, runtime/sessions/memory, protocol/wire/clients) are in the sibling notes
and are left out here. Codex paths are relative to `references/codex/`; Remit paths are relative to the repository root.

Labels: **HIGH** = verified by reading the file or running `rg`/`find`; **MEDIUM** = inferred from what was read;
**LOW** = belief not checked against this code.

## Ranked proposals

### 1. Snapshot the whole brief an agent reads (value HIGH, effort M, make it work)

- **Codex:** integration tests capture every request sent to the model and render it as a readable snapshot. A normalizer
  replaces paths, ids and known instruction blocks before rendering (`codex-rs/core/tests/common/context_snapshot.rs:1-40`).
  The repo holds 1771 `.snap` files; 180 sit in `core/tests/suite`, and 65 suite files call `assert_snapshot` (rg/find
  counts). HIGH
- **Remit:** `crates/orch/src/prompts.rs` (614 lines, "every rule an agent works by is stated here once", line 1) has 8
  tests, and each one checks fragments with `contains` (`prompts.rs:389-520`). No crate depends on `insta` (rg over
  `crates/*/Cargo.toml`). HIGH
- **Why better:** Remit's AGENTS.md says judgement lives in briefs ("improve a brief or playbook before adding a flag"). So
  the brief is the behaviour the agent actually gets. A `contains` test proves one sentence is there. It cannot show that
  an edit to a shared helper (`customer.rs`, `gate.rs`) silently rewrote the Chair's brief. A snapshot puts the whole
  rendered brief in the diff a reviewer reads. MEDIUM
- **Proposal:** add `insta` as a dev-dependency of `orch` and snapshot each brief kind (Chair in each mode, worker, gate
  reviewer, gate QC) from a seeded chain. Normalize ids and workdirs. Keep the `contains` tests that state a rule's intent.
- **Tension with AGENTS.md:** a snapshot is only worth something if someone reads its diff. A reflexive `cargo insta
  accept` turns it into a test that cannot fail. So snapshot only the briefs (the model-facing contract), never internal
  structs. MEDIUM

### 2. Turn "one owner" rules into compile errors with clippy `disallowed-methods` (value HIGH, effort S, make it work)

- **Codex:** `codex-rs/clippy.toml` bans every sqlx pool and connection constructor ("Create SQLite pools through
  codex-state's sqlite shim", lines 13-26), and the workspace sets `disallowed_methods = "deny"`
  (`codex-rs/Cargo.toml:603`). A second owner of SQLite setup cannot compile. HIGH
- **Remit:** `clippy.toml` holds only the four test allowances (lines 1-4). Today every `Connection::open` is in
  `crates/store` (rg: `store.rs`, `snapshot.rs`, `restore.rs`, `export.rs`, `workflows.rs`), but nothing keeps it that
  way. Child processes are spawned in 6+ places outside `procgroup` and `jail`, for example
  `crates/orch/src/repochecks.rs:252`, `crates/remit/src/runtime.rs:140`, `crates/plugin/src/start.rs:83` and
  `crates/claudecode/src/token.rs:82`. `childenv::of` is called only from `claudecode`, `memoryd` and `orch/repo.rs` (rg).
  HIGH
- **Why better:** `crates/fitness` already checks rules with `syn`, but it matches source text. `disallowed-methods` uses
  resolved paths, so it also catches re-exports and aliases, and it runs in the pre-commit clippy
  (`scripts/hooks/pre-commit`) instead of only in `cargo test`. It adds to fitness and replaces nothing. MEDIUM
- **Proposal:** list `rusqlite::Connection::open*` (reason: the store owns the database) and
  `std::process::Command::new` / `tokio::process::Command::new` (reason: a child's environment is decided by `childenv`,
  ADR-0018). Each spawn that is legitimate gets `#[expect(clippy::disallowed_methods, reason = "…")]`, which makes every new
  spawn a decision someone reviewed. `disallowed_methods` defaults to `warn` (checked, see proposal 7), so
  Remit's `-D warnings` makes it an error with no change to `[workspace.lints]`; only the `clippy.toml` list is needed. That is the rail argument of AGENTS.md: a spawn is an entrance. If this needs more than
  5 `expect`s, the spawn should go behind a helper in `procgroup`. Do not add a sixth `expect`.

### 3. Correlate logs with spans per chain step and session turn (value MED, effort S, make it work)

- **Codex:** a dedicated `otel` crate (`codex-rs/otel/src/`: `trace_context.rs`, `otlp.rs`, `metrics/`) carries trace
  context into MCP calls, and a test checks that it is propagated (`codex-rs/core/tests/mcp_trace_propagation.rs`). A
  separate opt-in local trace bundle follows "observe first, interpret later" (`codex-rs/rollout-trace/README.md:1-20`).
  HIGH
- **Remit:** `crates/remit/src/main.rs:124-137` writes JSON logs to stderr with `with_current_span(false)`. The crates
  emit 279 `tracing::{info,warn,error,debug}!` events against 5 span or `instrument` sites (rg). HIGH
- **Why better:** when a chain misbehaves on the pod, the only way to follow one step across `orch` → `runtimelink` →
  `claudecode` is to grep ids out of log text. A span with `chain`, `card` and `turn` fields puts them on every line
  automatically. MEDIUM
- **Proposal:** add one `info_span!` at the step transition and one per session turn, and turn on `with_current_span`.
  The seam is the span fields; leave out an OTLP exporter. Remit's facts and signals (`crates/orch/src/fact.rs:1-5`)
  stay what an agent watches. Spans are for whoever debugs the server.

### 4. Bound every test and CI job in time (value MED, effort XS, hardening)

- **Codex:** nextest gives each test a 30 s slow period and terminates it after 2 periods
  (`codex-rs/.config/nextest.toml:5`). CI jobs carry `timeout-minutes` (`.github/workflows/rust-ci-full.yml:95,130,160`).
  HIGH
- **Remit:** `cargo test --workspace` (`Makefile:60`) has no per-test bound, and neither `.github/workflows/rust.yml` nor
  `release.yml` sets `timeout-minutes` (rg: no hits). The auto-memory records a mutex hang in the orch tests. HIGH (files),
  MEDIUM (hang history)
- **Proposal:** add `timeout-minutes` to both workflows now. That is one line each, and a hung test then fails in minutes
  instead of after GitHub's 6 h default. Adopting nextest is a later step. Codex's `retries = 1`
  (`nextest.toml:6`) **conflicts** with Remit, because a flake should be fixed, not retried. The jail test blocks
  nextest as it is written. It is `harness = false` (`crates/jail/Cargo.toml` `[[test]] name = "jail"`), and its `main`
  reads no arguments: it runs `jail::init()`, then every test, filtered only by the `JAIL_TEST` env var, and prints
  `ok: <name>` (`crates/jail/tests/jail.rs:15-50`). nextest starts each test binary with `--list --format terse` and then
  `--exact <name>`, so this binary would run all its tests instead of listing them. Adopting nextest therefore also means
  teaching that `main` libtest's `--list`/`--exact` arguments (a small change), or keeping the jail test under
  `cargo test --test jail`. HIGH (the jail main); MEDIUM (nextest's invocation contract, from its documented
  custom-harness protocol, not re-read here).

### 5. Wait for signals, never sleep, in tests (value MED, effort M, hardening)

- **Codex:** a local AGENTS.md says "Do not use timeouts or sleeps to synchronize tests. Wait for explicit signals or
  observable state" (`codex-rs/code-mode-host/AGENTS.md:3`). HIGH
- **Remit:** the test doubles poll in 20 ms sleep loops capped at 5 s: `crates/orch/src/tests.rs:249-257`, `:598-605` and
  `:902`, `crates/orch/src/workflow_run/tests.rs:213`, `crates/orch/src/source_tests.rs:879`,
  `crates/claudecode/src/tests.rs:210,226`. 17 sleep calls sit in test code (rg). HIGH
- **Why better:** a poll with a deadline fails under load (CI, mutants, parallel orch tests). A `tokio::sync::Notify` or
  `watch` in the fake `Agents` double wakes exactly when the prompt arrives. MEDIUM
- **Proposal:** have the `Agents` double in `orch/src/tests.rs` publish through `watch::Sender<Vec<String>>`, so `until`
  awaits `changed()`. The outer bound comes from proposal 4.

### 6. Release and dev profiles with line tables (value MED, effort XS, make it work)

- **Codex:** `debug = "line-tables-only"` in dev (`codex-rs/Cargo.toml:647-650`) and in release, with thin LTO and symbols
  kept until packaging strips them (`:658-667`). A `profiling` profile also exists (`:669-673`). HIGH
- **Remit:** only `[profile.wasm]` (`Cargo.toml:41-47`). Release and dev use Cargo's defaults: release has no debug info,
  so a dependency panic or a tokio task panic on the pod prints a backtrace with no file:line. HIGH (config), MEDIUM
  (consequence)
- **Proposal:** add `[profile.release] debug = "line-tables-only"` and `[profile.dev] debug = "line-tables-only"`. The
  second also cuts the size of the target dir, and the auto-memory records disk-space blockers. Measure binary size
  before and after.

### 7. Opt into the non-default clippy lints that back Remit's own rules (value MED, effort XS, make it work)

- **Codex:** denies, among others, `redundant_clone`, `needless_collect` and `trivially_copy_pass_by_ref`
  (`codex-rs/Cargo.toml:620-630`), plus `await_holding_invalid_type` for tokio guards (`:601` and
  `codex-rs/clippy.toml:3-7`). HIGH
- **Remit:** denies `await_holding_lock` and `await_holding_refcell_ref` (`Cargo.toml:30-31`) but not tokio guards, and
  30 `tokio::sync::{Mutex,RwLock}` uses exist (rg). `projects/CLAUDE.md` bans "`clone()` added only to satisfy the
  borrow checker", yet `redundant_clone` is not enabled. HIGH
- **Lint levels, checked** with Remit's pinned clippy (`clippy-driver rustc -W help` from toolchain 1.99.1, which compiles
  nothing): `redundant_clone` and `needless_collect` are in `clippy::nursery` at `allow`; `trivially_copy_pass_by_ref`,
  `uninlined_format_args`, `redundant_closure_for_method_calls` and `manual_let_else` are in `clippy::pedantic` at `allow`.
  So `-D warnings` (`Makefile:59`) does **not** cover them, and Codex's explicit `deny` does add checks. HIGH.
  `await_holding_invalid_type` and `disallowed_methods` default to `warn`, so they bite under `-D warnings` as soon as
  their `clippy.toml` list is non-empty. Today Remit's list is empty, so neither has any effect. HIGH
- **Proposal:** enable `redundant_clone`, the one that backs a written Remit rule. `needless_collect` is a cheap
  performance lint and can wait for hardening. Skip `trivially_copy_pass_by_ref`, which is pedantic style. Decide on
  tokio guards deliberately: holding a tokio guard across `.await` is legal by design, so list `tokio::sync::MutexGuard`
  in `await-holding-invalid-types` only if a reviewed audit of the 30 sites finds no intended hold. Otherwise apply
  AGENTS.md's ">5 expects → switch it off". The lint count for `redundant_clone` comes from the first clippy run that
  enables it; that is part of making the change, not of this study.

### 8. Test the darwin code in CI (value MED, effort S, make it work)

- **Codex:** full CI runs tests on `macos-15-xlarge` next to Linux and Windows (`.github/workflows/rust-ci-full.yml:486`).
  HIGH
- **Remit:** CI and release run only on `ubuntu-latest` (`.github/workflows/rust.yml:16`, `release.yml:17`). The jail's
  darwin half (`crates/jail/src/{init,plan,sandbox,sweep}_darwin.rs`) and 12 `target_os = "macos"` sites are linted by
  the macOS pre-commit hook but tested only when someone runs `make test-changed` locally. The auto-memory says Remit runs
  on the Air first. HIGH (files), MEDIUM (deployment)
- **Proposal:** a `macos-latest` job running `cargo test -p jail -p claudecode -p remit`, the crates with darwin code.

### 9. Pin CI actions by commit SHA (value LOW-MED, effort XS, hardening)

- **Codex:** pins every third-party action by SHA with a version comment, and audits CI egress with `openai/fence`
  (`.github/workflows/blob-size-policy.yml:13-17`). HIGH
- **Remit:** pins by tag: `actions/checkout@v7`, `taiki-e/install-action@v2` (`rust.yml:18-19`). The release job holds
  `contents: write` (`release.yml:12-13`), so a moved tag could publish a release. HIGH
- **Proposal:** SHA-pin the three actions the release uses. Name it under the release ADR's *Revisit when*.

### 10. Directory-scoped review contracts (value LOW, effort XS, make it work)

- **Codex:** small AGENTS.md files next to the code state the accepted approximations so reviewers do not re-flag them:
  "These are accepted limitations of this phase … Review against this scope"
  (`codex-rs/exec-server/src/sandbox_integrity/AGENTS.md:33-36`). HIGH
- **Remit:** one root AGENTS.md, per-domain agents in `.claude/agents/`, and ADRs with *Revisit when*. 36 crates have a
  README.md (find). HIGH
- **Assessment:** Remit's ADR + domain-agent setup carries the same information. Codex's only gain is that the contract
  sits in the directory being edited. A crate README line linking its ADR's *Revisit when* gives the same result. No new
  file kind is needed.

### 11. Pre-main process hardening (value MED, effort S, hardening; owner: rails-and-sandbox)

- **Codex:** `pre_main_hardening` turns off core dumps, sets `PR_SET_DUMPABLE=0` / `PT_DENY_ATTACH` and strips
  `LD_*`/`DYLD_*` (`codex-rs/process-hardening/src/lib.rs:7-62`). It is called only by the process that holds the API key
  (`codex-rs/responses-api-proxy/src/main.rs:6`) and by the voice host. HIGH
- **Remit:** absent (rg: no `PR_SET_DUMPABLE`, `RLIMIT_CORE` or `PT_DENY` hits). The server holds source credentials while
  same-user agent processes run beside it; `crates/remit/src/main.rs` (comment after line 137) already warns about this on
  macOS. HIGH
- **unsafe, checked:** Remit already depends on `rustix` 1.1.6 with the `process` feature (`crates/remit/Cargo.toml:48`,
  `crates/jail/Cargo.toml:13,22,32`). Its source shows safe functions for two of the three calls:
  `rustix::process::set_dumpable_behavior` wraps `PR_SET_DUMPABLE`, Linux only (`rustix-1.1.6/src/process/prctl.rs:139-141`,
  module gated `cfg(linux_kernel)` at `process/mod.rs:35-36`). `rustix::process::setrlimit(Resource::Core, …)` works on
  Linux and macOS (`process/rlimit.rs:39`, `backend/libc/process/types.rs:38-40`, excluded only on haiku). There is no
  `PT_DENY_ATTACH`/`ptrace` request API in rustix (rg over `src/`: only `PR_SET_PTRACER` and wait hits). HIGH
- **So:** Linux hardening (non-dumpable, no core file) and core-off on macOS fit inside `unsafe_code = "forbid"`
  (`Cargo.toml:15`) with no new crate. macOS `PT_DENY_ATTACH` would still need `libc::ptrace` and `unsafe`, so it needs
  an exception the owner decides on. Codex also clears `LD_*`/`DYLD_*`, which `crates/source/src/extension.rs:263`
  partly covers for child extensions. The decision, and whether this matters beside the jail, belongs to the
  sandbox/secrets notes and the `rails-and-sandbox` owner. Recorded here so it is not lost.

## Where Codex's choice conflicts with Remit's stance (do not adopt)

| Codex practice | Evidence | Conflict | Conf. |
|---|---|---|---|
| Feature-flag registry with stages; 48 `Removed` variants kept "for backward compatibility" | `codex-rs/features/src/lib.rs:53-68`; 204 variants (rg) | "Leave nothing behind" and "no flag … built before it is needed". Remit ships through canary stages (`merge_mr`, ADR-0050) and deletes the old path. | HIGH |
| Layered config with enterprise "requirements" constraints and per-layer provenance | `codex-rs/config/src/config_layer_source.rs:4-69`, `constraint.rs:58` | Agent-centric: the owner configures by talking to an agent; Remit has flags (`crates/remit/src/flags.rs`) plus state agents write. | HIGH |
| Bazel + justfile + Python CI scripts; 8073 lines of workflows | `MODULE.bazel`, `justfile`, `.github/workflows` (wc) | Cost of change. A 113-line Makefile and two workflows fit Remit's scale; Bazel even forced Codex to ban crate features (`.github/scripts/verify_cargo_workspace_manifests.py:67-71`). | HIGH |
| Global test mode switched on by `#[ctor]` | `codex-rs/core/tests/common/lib.rs` (ctor `set_deterministic_process_ids`) | "No global state, dependencies passed in". | HIGH |
| `retries = 1` for every test | `codex-rs/.config/nextest.toml:6` | A flake is a defect with a root cause. | HIGH |
| `anyhow` in libraries (72 manifests including `core`, `protocol`, `state`); 686 `Result<_, String>` outside tests | `codex-rs/core/Cargo.toml:16`, rg counts | "A typed error returned with `?`". | HIGH |
| `#![allow(clippy::expect_used)]` with no reason at the top of test crates | `codex-rs/core/tests/all.rs:1` | Remit denies `allow_attributes_without_reason`. | HIGH |
| Argument-comment Dylint (`/*param*/ true`) on a pinned nightly | `tools/argument-comment-lint/README.md:1-30` | Its own README says to prefer an enum or newtype, which is Remit's "illegal states unrepresentable". Take that advice and skip the tool. | HIGH |

## Where Remit is already better

- **Lints:** Remit forbids `unsafe_code` and denies `panic`, `indexing_slicing`, `arithmetic_side_effects`,
  `as_conversions` and `allow_attributes_without_reason` (`Cargo.toml:14-33`). Codex has none of these
  (`codex-rs/Cargo.toml:597-636`). HIGH
- **Architecture fitness:** `crates/fitness` parses source with `syn`, reads `cargo metadata`, runs each rule on a seeded
  violation, and fails on stale exceptions (`crates/fitness/src/lib.rs:1-7`; edges at `edges.rs:343-353`). Codex's
  boundary check is a regex over source text (`.github/scripts/verify_tui_core_boundary.py:17-21`). Codex's manifest check
  does share the stale-exception idea (`verify_cargo_workspace_manifests.py:52`). HIGH
- **Mutation testing** on each PR's changed lines (`.github/workflows/rust.yml:27-39`, `Makefile:80-83`). Codex has none
  (rg `mutants` in its workflows and justfile: no hits). HIGH
- **Change-scoped checks:** `make test-changed` and the clippy pre-commit hook scoped to staged files plus their dependents
  (`scripts/hooks/pre-commit`). Remit also has a commit-size budget (`scripts/commit_size.sh`) and a check that generated
  files are fresh (`Makefile:21-25`). Codex has blob-size and clean-worktree checks of the same kind. HIGH
- **Decision records:** ADRs with *Revisit when* hold the deferred hardening. Codex keeps similar rationale in scattered
  comments and in `deny.toml` reasons (`codex-rs/deny.toml` advisories). MEDIUM
- **Build stamp and multi-call binary:** these are parity, not gaps. Remit's `crates/remit/build.rs:1-30` matches
  `codex-rs/build-info`, and the jail re-executes itself under a helper arg0 (`crates/jail/src/plan_linux.rs:66`) as
  `codex-rs/arg0/src/lib.rs:170-190` does. HIGH

## A Remit gap found on the way (not from Codex)

`Result<_, String>` appears 210 times outside tests in Remit: orch 49, remit 27, bundled 27, jail 22 (rg). AGENTS.md asks
for typed errors. Codex is worse in absolute terms, so this is not a comparison point, but it is a defect in Remit's own
terms. MEDIUM: some of these are boundary messages meant for humans.

## Coverage

Read on the Codex side: `codex-rs/Cargo.toml` (members head, lints, profiles), `clippy.toml`, `deny.toml` (advisories,
licences head), `rustfmt.toml`, `rust-toolchain.toml`, `.config/nextest.toml`, `justfile`, the three nested AGENTS.md
files, `.github/workflows` (`rust-ci.yml` steps, `rust-ci-full.yml` job names, `blob-size-policy.yml` head,
`repo-checks.yml` runs), `.github/scripts/verify_cargo_workspace_manifests.py` (head), `verify_tui_core_boundary.py`, and
the heads and APIs of `features`, `arg0`, `process-hardening`, `build-info`, `diagnostics`, `rollout-trace` README,
`test-binary-support`, `utils/cargo-bin`, `core/tests/{all.rs,common/lib.rs,common/context_snapshot.rs,common/responses.rs}`,
the `config/src` listing with `config_layer_source.rs` and `constraint.rs` docs, and `tools/argument-comment-lint/README.md`.

Skipped: `rust-release.yml` (2173 lines; Remit ships one Linux target, so the signing, notarization, npm, winget and
DotSlash machinery has no counterpart), the Bazel files (`MODULE.bazel`, `defs.bzl`, `rbe.bzl`), the internals of `otel`
and `feedback` (only their file lists were read), the `cloud-tasks-mock-client` body, the Python SDK and `codex-cli`
packaging, and every domain crate owned by the sibling notes. No build or compiling cargo command was run; every count
above comes from `rg`, `find` or `wc`. Three checks settled earlier open points: the lint groups and default levels, from
`clippy-driver rustc -W help` on the pinned 1.99.1 toolchain; the jail test's `main` (`crates/jail/tests/jail.rs:15-50`),
read for nextest compatibility; and the rustix 1.1.6 source in `~/.cargo/registry`, read for safe hardening calls.
