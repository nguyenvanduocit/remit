# chains

The chain core and its sessions (ADR-0096), the bottom of orch's crates: every crate above builds on it.

- `Core` (`src/chain_core.rs`): the store, the notifier, the clock, the launcher, what is held in memory of each chain and
  session, the stop tokens. Its fields are its dependencies, filled from `Config` and `Deps`; nothing is package state.
- `Service` (`src/request.rs`): a service over the core, with `act` and `once` (each action once per request id). Every
  crate above owns one service type, `{ core: Arc<Core>, me: Weak<Self>, .. }`.
- The one transition (`src/step.rs`, the only place a chain's status is set) and the one writer (`src/apply.rs`,
  `src/commit.rs`), with every pure rule and every `store::Tx` write they read, whatever domain it belongs to.
- Sessions and the chain's life: `session`, `outbox`, `mailbox`, `stream`, `plan`, `health`, `failover`, `handover`,
  `subagents`, workflows (`workflow_run`, `workflow_claude`, `workflow_tools`), the repetition rail, recovery, the
  lifecycle actions, the scheduler's `kick`.
- The mechanisms every crate uses: the `Tools` builder and `Effect` (`src/tools.rs`: a handler captures the service it
  acts through), events and decisions, change items and owner acts, the readings door (`ingest`), facts, the ports
  (`src/forges.rs`, `src/ports.rs`) and the `Notifier` (`src/notify.rs`).
- `Domains` (`src/domains.rs`): what the core hands up to the domains (a launch's spec, a judge's turn ending, a hiring),
  each a named method called where it was called before the split; `orch` implements it once.

Feature `testing` exposes the fake forge (`forges::fake`) and seeded draws to the application tests.

## Verify

```sh
CARGO_TARGET_DIR=$HOME/.cache/remit-target cargo clippy -p chains --all-targets -- -D warnings
CARGO_TARGET_DIR=$HOME/.cache/remit-target cargo nextest run -p chains -p orch
```

The tests that need the whole application live in `crates/orch/tests/app/chains_*.rs`; `-p orch` runs them.
