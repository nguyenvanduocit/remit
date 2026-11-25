# 0075. A workflow script runs in QuickJS, a fresh VM per evaluation, and oxc reads its meta

Status: accepted, 2026-11-11. Supersedes ADR-0073's engine (goja inside Go's `internal/workflow`); every other part of
ADR-0073 (the entity, the verbs, replay from the journal, the shell) stands. Builds on ADR-0072 (the backend is Rust).
Source: the owner's words of 2026-11-11, "we no longer do anything in Go", with the Rust port's largest gap left: the
engine. Read against the working tree, rquickjs 0.14.0, boa_engine 0.22.0 and oxc_parser 0.158.0 on 2026-11-11. Proof of
concept: `/tmp/remit-js-poc/` (outside the repository).

## Context

The customer is the owner, whose Chairs and routines run scripts (ADR-0073, decision 3). In the Rust backend every script is
refused: `Unported::advance` answers `Failed` for any script (`crates/orch/src/unported.rs:116-130`), so `run_workflow`
and every routine with a script fail before anything is stored. The shell around the engine is ported
(`crates/orch/src/workflow_run.rs`: `start_workflow`, `new_script_run`, `advance`, `issue`, `move_run`), and its tests run on
a fake engine (`crates/orch/src/workflow_run/tests.rs:157`, `fake_advance`).

What the engine must do, from `internal/workflow` (HIGH, read in full):

- **Pure and synchronous.** `Advance(script, args, journal) → Outcome` (`workflow.go:112-148`): a fresh VM per evaluation,
  the script started, then each recorded result handed to it in arrival order with the promise jobs drained after each
  (`workflow.go:163-226`). A script suspends at a verb because its promise stays pending, not because the engine yields;
  no event loop, no async runtime.
- **Divergence is uncatchable.** Every verb goes through one host function that numbers the call, hashes
  `(verb, canonical input)` and compares it with the journal's entry at that step (`workflow.go:230-246`); a mismatch
  interrupts the VM so no `try/catch` in the script can swallow it.
- **Deterministic.** A prelude removes the clock (`Date.now`, `new Date()`), `Math.random`, `eval`, `Function` and the
  constructors reachable from any function (`prelude.go:8-34`); the VM has no timers, modules or host objects.
- **Bounded.** One evaluation may run 2 s of JavaScript (`workflow.go:28`); a script that loops without calling a verb is
  interrupted, also inside `try/catch`.
- **Meta is a literal.** `export const meta = {...}` must be the first statement and a pure literal, read from the AST
  without running code (`workflow.go:326-434`).
- **Hashes are stored.** `workflow_steps` hold each call's hash (`hash.go:13-35`): canonical JSON is Go's `json.Marshal`
  of the decoded value, keys sorted, numbers as written, and `<`, `>`, `&`, U+2028, U+2029 escaped (Go's default). A run in
  flight at ADR-0072's cutover is replayed by the Rust engine against hashes Go wrote.

House rules that bind the choice: `unsafe_code = "forbid"` and clippy's `unwrap_used`, `expect_used`, `panic` denied in
every crate of ours (`Cargo.toml:14-21`); `cargo deny check` on every build (`deny.toml`, `Makefile:130`); the workspace
already compiles C into the binary: bundled SQLite (`crates/store/Cargo.toml:19`), `ring` and `aws-lc-sys` (`Cargo.lock`).

## Options

Measured in the PoC on this machine (Apple silicon, rustc 1.99.1, clean `cargo build --release`):

| | rquickjs (QuickJS-ng, C) + oxc_parser | boa_engine (pure Rust) | deno_core (V8) | another language (Rhai, Starlark) |
|---|---|---|---|---|
| Interrupt a running script | `Runtime::set_interrupt_handler`, uncatchable (HIGH, PoC) | none: only a per-frame loop-iteration count, recursion depth and stack size (HIGH, `boa_engine-0.22.0/src/vm/opcode/iteration/loop_ops.rs`); `fib(32)` ran 2 s unbounded in the PoC, `fib(40)` would run minutes | `terminate_execution` from another thread (MEDIUM, not built) | Rhai `on_progress` counts operations (MEDIUM) |
| Sync stepping | `Ctx::execute_pending_job`, `Promise::state` (HIGH, PoC) | `run_jobs`, `JsPromise::state` (MEDIUM) | built around a tokio event loop and async ops (MEDIUM) | Starlark has no async: `parallel` cannot exist |
| Sandbox | intrinsics chosen per context: no Performance, WeakRef, DOMException; no std/os module, no timers (HIGH, PoC) | host hooks can refuse code from strings (MEDIUM) | a full runtime to strip down | hermetic by design |
| Meta from the AST | needs a parser: oxc (pure Rust, MIT) | `boa_parser` in the same crate | needs a parser | n/a |
| Build, binary | 82 s, 2.7 MB with oxc and sha2 (PoC) | 128 s, 12.7 MB for the bare engine (PoC) | a prebuilt V8 static library downloaded at build time, outside cargo-deny's view; tens of MB (MEDIUM, rusty_v8's README) | small |
| cargo-deny | `advisories ok, bans ok, licenses ok, sources ok` under our `deny.toml` (PoC) | MIT/Unlicense (HIGH) | BSD-3 / MIT | MIT/Apache |
| Memory safety of the interpreter | C: a VM bug is a bug in our process | Rust | C++ | Rust |
| Runs Claude Code's scripts unchanged (ADR-0073) | yes (top-level await and return through the async wrapper; PoC) | yes | yes | no: every script and the `workflows` skill rewritten, in-flight runs lost |

A fifth option, keeping goja behind a Go sidecar, contradicts the owner's Rust-only rule and was not weighed further.

## Decision

1. **A new crate, `workflow`** (`crates/workflow`), is the pure engine: `pub fn advance(script: &str, args:
   Option<&RawMessage>, journal: &[Entry]) -> Outcome`. It owns `Entry`, `Call`, `Outcome`, `ScriptState` and `ScriptError`,
   which leave `crates/orch/src/workflow_run.rs` (one concept, one owner); it depends on `model` (for `WorkflowVerb`,
   `RawMessage`), `rquickjs`, `oxc_parser`, `serde_json` and `sha2`, and on nothing that stores, starts or reads a clock
   other than the evaluation's deadline. `#![forbid(unsafe_code)]` holds in it; the C lives in `rquickjs-sys`, as SQLite's
   does in `libsqlite3-sys`.
2. **QuickJS-ng through rquickjs** runs the script: a new `Runtime` and `Context::custom` per evaluation with the intrinsics
   Date, Eval, RegExp and its compiler, JSON, Proxy, Map/Set, typed arrays and Promise; Performance (a clock), WeakRef (it
   shows the garbage collector) and DOMException are not added. Eval stays because `JS_Eval` evaluates through it
   (`ctx->eval_internal`, set by `JS_AddIntrinsicEval` in `quickjs.c`); the prelude removes `eval` and `Function` from
   scripts, as in Go. rquickjs's default features only (`std`): no `futures`, `loader` or `parallel`.
3. **The prelude and the API are JavaScript files** in the crate (`prelude.js`, `api.js`), carried verbatim from
   `prelude.go` and `api.go` and loaded with `include_str!`: the script's API is JavaScript and is read as JavaScript.
   One Rust host function, `call(verb, input, label, phase) → seq`, numbers, canonicalizes, hashes and checks each call.
4. **oxc reads the script first** (module syntax, return outside a function allowed): the first statement must be
   `export const meta = <literal>`, converted to `Meta` without running code; `export` is blanked to spaces so positions in
   errors stay true; the body is wrapped in `(async function () { … })` and evaluated in QuickJS.
5. **One interrupt handler bounds an evaluation**: it returns true once 2 s have passed since the evaluation began, or once
   a call has diverged. QuickJS raises that as an uncatchable exception (PoC: `while (true) {}` inside `try/catch` and an
   exponential recursion both fail at 2 s with "the script ran 2s without waiting on a call").
6. **Canonical JSON and the hash are byte-for-byte Go's**: keys sorted, numbers as written, Go's escaping of `<`, `>`, `&`,
   U+2028 and U+2029, then `sha256(verb ‖ 0x00 ‖ input)` in hex; vectors generated from `hash.go` before Go is deleted pin it.
7. **The shell calls the engine off the async runtime**: `workflow_run.rs` calls `workflow::advance` inside
   `tokio::task::spawn_blocking`, so a 2 s evaluation never holds a tokio worker; `Unported::advance` and the tests'
   `fake_advance` are deleted, and the shell's tests run on the real engine.

## Threats

The script is written by an agent (a Chair, or the Assistant for a routine), and its text and arguments reach the engine
from the model's output; no outsider reaches it directly, but a prompt injected into a Chair can.

- A memory-safety bug in QuickJS's C, reached by a crafted script, runs in the server's process. Bounded by: no host object
  but `call`, no I/O, no module loader, no timers; a fresh runtime per evaluation, dropped after it.
- A script that allocates without bound (`'x'.repeat(2 ** 29)` in a loop) can exhaust the server's memory before 2 s pass;
  Go's engine had the same exposure.

## Consequences

- Every Chair's and routine's script runs in the Rust backend; ADR-0073's behaviour is unchanged, test for test.
- Runs in flight at the cutover replay on the Rust engine. The call numbering is the same in both engines: the PoC drove
  the Go test's `review` script (`parallel` twice, `phase`, `log`, a failing finder) and its `pipeline` script, each with
  results arriving oldest first, newest first and from the middle, through goja (Go's `internal/workflow` copied into
  `/tmp/remit-js-poc/goharness`) and through QuickJS; the 42 issued calls (seq, verb, phase, hash, input) and the 6
  results are identical (`diff trace-goja.txt trace-quickjs.txt`). The hashes match only once decision 6 holds: for
  `{"prompt":"a<b & c","n":1e21,"x":1.50}` Go writes `{"n":1e21,"prompt":"a\u003cb \u0026 c","x":1.50}` and plain
  `serde_json` writes `{"n":1e+21,"prompt":"a<b & c","x":1.5}`. A run that still diverges fails with the step and the
  reason, and the Chair restarts it with `resume_from`, which reuses the steps that still match (`reused_steps`).
- The binary grows by about 2.7 MB less the PoC's own code, and a clean build by about a minute (PoC); the build needs a C
  compiler, as SQLite's bundled build already does.
- oxc_parser 0.158 declares `rust-version = "1.97.0"` while the workspace declares 1.94 (`Cargo.toml:7`) and the toolchain
  is 1.99.1 (`rust-toolchain.toml`): the port raises the workspace's `rust-version` to the toolchain's, or resolver 3
  silently picks an older oxc.
- Replaying from the start costs O(journal) per evaluation, O(n²) over a run: the PoC's three-call script evaluates in under
  1 ms, so a run of thousands of calls stays far below the 2 s limit.

## Tests (written first, main path first)

Ported from `internal/workflow/workflow_test.go` into `crates/workflow/src/tests.rs`:

- `a_workflow_advances_from_its_journal` (`TestAWorkflowAdvancesFromItsJournal`)
- `a_workflow_that_diverges_from_its_journal_fails` (`TestAWorkflowThatDivergesFromItsJournalFails`)
- `a_workflow_cannot_read_the_clock` (`TestAWorkflowCannotReadTheClock`, every line of its table)
- `a_script_for_claude_code_runs_unchanged` (`TestAScriptForClaudeCodeRunsUnchanged`)
- `a_pipeline_replays_in_the_order_its_results_arrived` (`TestAPipelineReplaysInTheOrderItsResultsArrived`)
- `a_stage_that_throws_drops_its_item_to_null` (`TestAStageThatThrowsDropsItsItemToNull`)
- `remits_verbs_are_calls` (`TestRemitsVerbsAreCalls`)
- `a_script_that_never_waits_is_interrupted` (`TestAScriptThatNeverWaitsIsInterrupted`), plus the recursion case
- `a_script_needs_its_meta_literal` (`TestAScriptNeedsItsMetaLiteral`)
- `any_isolation_is_refused_at_the_call` (`TestAnyIsolationIsRefusedAtTheCall`)
- new: `a_call_hashes_as_go_did`, on vectors written by `hash.go` (escaping, number spelling, key order)
- new: `calls_are_numbered_as_goja_numbered_them`, the PoC's traces of `review` and `pipeline` in three arrival orders as
  fixtures

In `crates/orch`, the shell's tests (`workflow_run/tests.rs`) drop `fake_advance` and run their scripts on the engine; the
Go orch tests still to port with it are `TestTheWorkflowsSkillsPatternsRun` (the `workflows` skill's patterns run) and
`TestAChairsWorkflowHiresAndHandsBackTheResult`.

## Revisit when

- **Isolation of the VM** (asked for as hardening, or a QuickJS-ng advisory): `advance` is a pure function of
  `(script, args, journal)` with serializable input and output, so it moves into a jailed child process (`crates/jail`) or
  into QuickJS compiled to WebAssembly without a change to its callers.
- **Memory**: `Runtime::set_memory_limit` and `set_max_stack_size` go where the runtime is made (one function in the crate);
  not set today, as Go set none.
- **A deterministic budget** in place of the wall clock: the interrupt handler can count its own calls (QuickJS calls it
  every 10 000 interrupt-counter ticks, `JS_INTERRUPT_COUNTER_INIT` in `quickjs.c`) so a script fails at the same point on
  every machine; wanted once a run fails on a slow machine and passes on a fast one.
- **Claude Code's own Workflow tool becomes durable and resumable across processes with Remit's verbs**: the `remit` engine
  and this crate are deleted, and every run uses engine `claude`.
- **QuickJS-ng or rquickjs falls behind** (no release in a year, an unfixed advisory): boa once it can interrupt a running
  script; the engine is one crate behind one function.
