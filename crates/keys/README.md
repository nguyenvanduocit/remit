# keys

Client keys and their door, worker keys for mentors' devices (ADR-0096). The service is `Keys` (`src/service.rs`); a
client's own MCP tools are assembled in `crates/orch/src/client_tools.rs`. Entrance keys are a chain mechanism and live in
`crates/chains/src/entrance_keys.rs`. Depends on `chains` and `capability`.

## Verify

```sh
CARGO_TARGET_DIR=$HOME/.cache/remit-target cargo clippy -p keys --all-targets -- -D warnings
CARGO_TARGET_DIR=$HOME/.cache/remit-target cargo nextest run -p orch
```

The keys' tests drive the whole application and live in `crates/orch/tests/app/keys_*.rs`.
