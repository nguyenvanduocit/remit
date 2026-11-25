# pipeline

The signal pipeline (ADR-0064, ADR-0096): readings to firings to deliveries, subscriptions, sensors, routines, watches,
event signals and the board. The service is `Pipeline` (`src/service.rs`). What a firing does in another domain (hire,
end a chain, read the checks on a landing, teach, a digest) goes through `Actions` (`src/actions.rs`), which `orch` implements; its own
notifications are `notify::Notifier`. Depends on `chains` and `reach`.

## Verify

```sh
CARGO_TARGET_DIR=$HOME/.cache/remit-target cargo clippy -p pipeline --all-targets -- -D warnings
CARGO_TARGET_DIR=$HOME/.cache/remit-target cargo nextest run -p pipeline -p orch
```
