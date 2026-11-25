# shipping

From an agent's change to users (ADR-0096, ADR-0114): clones and checks, `land` (`src/land.rs`), the gate and the goal
judge, the merge queue that lands every approved change itself (pure core `src/landing.rs`, shell `src/queue.rs`, the
`Lander` port and its one adapter `src/direct_push.rs`), the forge's checks on what landed and the revert of a breach
(`src/landing_checks.rs`, `src/revert.rs`), canary stages (`src/stage.rs`, rule in `crates/rollout`), competing attempts at a key result and `pick_attempt`
(`src/round.rs`, ADR-0115), the one-time close of the draft merge requests carried over (`src/carried.rs`), verification,
rails checked against telemetry (`src/check_rail.rs`), and the release of what lands by each repository's own `.remit/release/` (`src/release.rs` its pure core, `src/release_run.rs` its runner, `src/install.rs` the install on this server, ADR-0116). The service is `Shipping` (`src/service.rs`), holding the services below it in `Below`. Depends on every crate
below `orch`, `crew` included.

## Verify

```sh
CARGO_TARGET_DIR=$HOME/.cache/remit-target cargo clippy -p shipping --all-targets -- -D warnings
CARGO_TARGET_DIR=$HOME/.cache/remit-target cargo nextest run -p shipping -p orch
```

The tests that need the whole application live in `crates/orch/tests/app/shipping_*.rs`.
