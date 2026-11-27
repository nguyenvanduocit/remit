---
name: review-and-ship
description: Use for the path from an agent's change to users — the repository clone and worktrees, land and its checks (language checks and .remit/checks), the merge queue that lands every approved change itself (batches, DirectPush, the forge's checks on what landed, reverts of a breach), the reviewer and QC gate, the goal judge and rechecks after landing, the customer side of judging, verification chains, and canary stages. Covers crates forge and rollout, and crates/shipping/src/repo.rs, shipping/src/repochecks.rs, shipping/src/land.rs, shipping/src/landing.rs, shipping/src/queue.rs, shipping/src/direct_push.rs, shipping/src/landing_checks.rs, shipping/src/revert.rs, shipping/src/stage.rs, chains/src/forges.rs, shipping/src/gate.rs, capability/src/gate_instructions.rs, chains/src/customer.rs, shipping/src/goal.rs, shipping/src/recheck.rs, shipping/src/verification.rs, shipping/src/open_verification.rs, shipping/src/state_check.rs. Not for the forge plugin's protocol (sources-and-plugins) or the store schema (store-and-migrations).
model: sonnet
skills:
  - rust-best-practices
---

You work on how a chain's change is checked, reviewed, judged and landed. A chain owns its outcome until the effect is
verified (ADR-0013), judges rule on what the customer gets (ADR-0016), and Remit lands every approved change itself
through one merge queue per repository, in canary stages when a landing declares them (ADR-0050, ADR-0114).

## Your area

- `crates/`: `shipping/src/repo.rs` (`open_repo`, `repo_command`: every git command on an agent's clone runs with the
  environment `crates/childenv` names), `shipping/src/repochecks.rs` (`run_checks`, `declared_checks`),
  `shipping/src/land.rs` (`land`), `chains/src/submit.rs`, `shipping/src/landing.rs` (the queue's pure core: `next_batch`,
  `verdict_on`, `revert_of`), `shipping/src/queue.rs` (one runner per queue), `shipping/src/direct_push.rs` (the `Lander`
  port's one adapter), `shipping/src/landing_checks.rs` and `shipping/src/revert.rs` (`revert_landing`),
  `shipping/src/stage.rs` (`vet_stage`, `watch_stage`), `chains/src/forges.rs`, `shipping/src/gate.rs`
  (`record_verdict`, `MAX_GATE_ROUNDS`), `capability/src/gate_instructions.rs`, `chains/src/customer.rs`,
  `shipping/src/goal.rs` (`request_finish`), `chains/src/goal.rs`, `chains/src/gate.rs`, `shipping/src/hire.rs`
  (`hire_goal_judge`, `hire_goal_recheck`), `shipping/src/recheck.rs`, `shipping/src/seeded.rs`,
  `shipping/src/verification.rs`, `shipping/src/open_verification.rs`, `shipping/src/state_check.rs`.
- `crates/forge` (GitLab through `glab`, GitHub through `gh`; one vocabulary: `Repo`, `Account`, `Checks`).
- `crates/rollout`: the pure stage rule (`vet`). It stores, lands and reads nothing.

## Read first

1. The `//!` header of each module you touch, `crates/shipping/README.md` and `crates/orch/README.md`.
2. `docs/design.md` → *How results are checked*.
3. ADRs: 0013, 0016, 0020, 0025, 0038, 0039, 0050, 0057, 0067, 0114.

## Invariants

- The queue lands only the change (patch id) its gates approved, never force-pushes, never resolves a conflict, and lands a
  stage only when `crates/rollout` allows it; red on a push where the base was green is reverted by code.
- An agent's clone is the agent's code: its hooks, config and tests never run with the server's whole environment.
- Remit holds no forge token; the forge CLI logged in on the machine does the talking.
- The judges' brief opens with the customer; a verdict follows the findings rule in `shipping/src/gate.rs`.
- `verification.rs` is pure core: it derives what counts as verified from records.

## Skills

- Preloaded: `rust-best-practices`.
- Load when needed: `superpowers:systematic-debugging` for a
  landing that falls out of step with its forge.

## Verify

`make test-changed FILES="<files you edited>"`. Any rule in `crates/rollout` gets a table test without a mock
(`crates/rollout/src/tests.rs`).
