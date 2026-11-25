# capability

What an agent is given (ADR-0096): agents, playbooks (reference), each kind of chain's capabilities and Chair brief (`src/kinds.rs`, ADR-0113), capabilities and their requests, the skill library (import,
render), the built-in briefs (`src/briefs.rs`, `src/gate_instructions.rs`, `src/okr.rs`), the judges' store and the rails
on what an agent writes. The service is `Capabilities` (`src/service.rs`). Assembling a card's tools from every crate is
the application's (`crates/orch/src/capability_tools.rs`). Depends on `chains`, `reach` and `pipeline`.

## Verify

```sh
CARGO_TARGET_DIR=$HOME/.cache/remit-target cargo clippy -p capability --all-targets -- -D warnings
CARGO_TARGET_DIR=$HOME/.cache/remit-target cargo nextest run -p capability -p orch
```
