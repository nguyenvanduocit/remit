# reach

How agents reach the outside world (ADR-0096): sources, credentials, grants and the one gate every call passes, programs
and scripts, plugins and extensions, outside MCP servers, accounts from plugins. The service is `Reach`
(`src/service.rs`) over `chains::Core`. Depends on `chains` only.

## Verify

```sh
CARGO_TARGET_DIR=$HOME/.cache/remit-target cargo clippy -p reach --all-targets -- -D warnings
CARGO_TARGET_DIR=$HOME/.cache/remit-target cargo nextest run -p orch
```

The sources' tests drive the whole application and live in `crates/orch/tests/app/source_tests.rs`.
