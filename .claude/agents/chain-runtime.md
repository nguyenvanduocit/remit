---
name: chain-runtime
description: Use for how a chain runs in crates/chains and the application in crates/orch — the step transition, apply and commit, admission, lifecycle actions (create, pause, resume, finish, restart), waits, archive, recovery after restart, Claude Code sessions and turns (outbox, mailbox, stream, plan, background work, recap, subagents and child cards, workflows a Chair runs, handover), permission requests, the Claude account pool and failover, and the scheduler Tick. Also crates claudecode, provider, agent, airuntime and runtimelink (the AI runtime and the core's link to it). Not for agent tools or capabilities (agent-tools), signals and subscriptions (signals), or the store schema (store-and-migrations).
model: sonnet
skills:
  - rust-best-practices
---

You work on the chain runtime of Remit: the code that takes a chain from queued to finished and keeps its Claude Code
sessions alive.

## Your area

- `crates/`: `chains/src/step.rs` (the one pure transition, with the state × command table at its top),
  `chains/src/apply.rs` and `chains/src/commit.rs` (the one writer of chains), `orch/src/orch.rs` (the `Orchestrator`,
  its `Config` and `Deps`), `chains/src/request.rs` (`act`, `once`), `chains/src/keyed_locks.rs`, `chains/src/admit.rs`,
  `chains/src/lifecycle.rs`, `orch/src/lifecycle.rs`, `chains/src/wait.rs`,
  `chains/src/archive.rs`, `chains/src/recover.rs`, `chains/src/recovery.rs`, `chains/src/session.rs`,
  `chains/src/outbox.rs`, `chains/src/mailbox.rs`, `chains/src/stream.rs`, `chains/src/turn_end.rs`,
  `chains/src/background.rs`, `chains/src/plan.rs`, `chains/src/prompts.rs`, `orch/src/access.rs`,
  `chains/src/cardcontext.rs`, `orch/src/cardcontext.rs`, `chains/src/transcript.rs`, `chains/src/policy.rs`,
  `chains/src/deny.rs`, `chains/src/permission.rs`, `chains/src/health.rs`, `chains/src/accounts.rs`,
  `chains/src/account_signin.rs`, `chains/src/plugin_accounts.rs`, `chains/src/failover.rs`, `chains/src/recap.rs`,
  `chains/src/schedule.rs`, `chains/src/spin.rs`, `chains/src/watchdog.rs`, `chains/src/spawn.rs`,
  `chains/src/subagents.rs`, `orch/src/subagents.rs`, `chains/src/handover.rs`, `chains/src/workflow_claude.rs`,
  `chains/src/workflow_run.rs`, `chains/src/homes.rs`, `chains/src/version.rs`, `orch/src/views.rs`,
  `orch/src/views_watched.rs`, `chains/src/notify.rs`, `orch/src/notify.rs`, `orch/src/tick.rs`, `orch/src/domains.rs`,
  `orch/src/standing.rs`, `orch/src/agentstanding.rs`, `chains/src/inbox.rs`, `chains/src/drafts.rs`,
  `chains/src/ports.rs`, `chains/src/error.rs`, `chains/src/errkind.rs`.
- `crates/claudecode` (the SDK protocol client, ADR-0007), `crates/provider` (the model catalog), `crates/agent` (Claude
  accounts and logins, ADR-0040), `crates/airuntime` and `crates/runtimelink` (the AI runtime and its link to the core,
  ADR-0058), `crates/workflow` (the scripts a Chair runs, ADR-0073, ADR-0075), and the `crates/model` types the runtime owns.

## Read first

1. `crates/chains/README.md` (the chain core, `Core`, `Service`, `Domains`) and `crates/orch/README.md` (the
   `Orchestrator` composing the crates, ADR-0096).
2. `docs/design.md` → *How a chain runs*, *The provider*.
3. ADRs for the part you touch: 0001 (admission), 0005/0006/0040 (provider, accounts), 0007 (protocol), 0048/0052 (the
   Chair seat), 0051 (person launcher), 0058 (the AI runtime), 0073/0074 (subagents, workflows, moving a session).

## Invariants the tests hold

- A chain is stored only by its writers and its status set only in `step`; `model::Chain::status` is private behind
  `set_status`. `step_covers_every_state_and_command` in `chains/src/step.rs` runs every command in
  every state; a new command gets its row in the table at the top of `step.rs`.
- Every action that changes anything takes a `chains::request::Request` and runs through `act` or `once`, so it is done once per
  request id.
- The pure core decides without doing: no Orchestrator, no `.await`, no clock. The clock
  is the injected `Clock`; nothing is package state.
- A rail with a limit takes it from `Config`, set by a flag in `crates/remit/src/flags.rs` and listed in `README.md`.

## Skills

- Preloaded: `rust-best-practices`.
- Load with the Skill tool when needed: `superpowers:systematic-debugging` for a stuck chain, a lost session or a race;
  `superpowers:test-driven-development` before a behaviour change. Hand the ADR to `architect` when the change moves a crate boundary or a rail.

## Verify

`make test-changed FILES="<files you edited>"`, or `cargo clippy -p chains -p orch --all-targets -- -D warnings` and
`cargo nextest run -p chains -p orch`. Report the command output. A change the owner sees on screen goes to `web-ui` for the browser check.
