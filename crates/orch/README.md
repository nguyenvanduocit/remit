# orch

Remit's application layer (ADR-0072, ADR-0096): the `Orchestrator` composing the crates below it, and what needs every
domain at once. The domains themselves live in their own crates, bottom to top: `chains` (the chain core and its
sessions), `reach`, `pipeline`, `capability`, `keys`, `crew`, `shipping`. Each depends only on those before it.

## How the crate is built

- `Orchestrator` (`src/orch.rs`): holds `core: Arc<chains::Core>` and one service per domain crate (`reach`, `pipeline`,
  `capabilities`, `keys`, `crew`, `shipping`), built by `Orchestrator::new` from `Config` (`core: chains::Config` plus the
  application's own settings) and `Deps`. It derefs to `Core`. Nothing is package state; the clock is the injected `Clock`.
- `Domains` and `Actions` (`src/domains.rs`): the one implementation of what `chains` hands up (`chains::Domains`) and
  of what a firing does (`pipeline::actions::Actions`), each delegating to the service that owns it.
- `Notifier` (`src/notify.rs`): the port the channels fill, the supertrait of each crate's own notifier;
  `src/views_watched.rs` wraps it so every change also marks the views (`src/views.rs`) that moved.
- The aggregators: the Tick (`src/tick.rs`), a card's tools from every crate (`src/card_tools.rs`,
  `src/capability_tools.rs`, `src/client_tools.rs`), the Assistant (`src/assist*.rs`, `src/assistant_tools.rs`),
  standings, boards, digests, and the lifecycle actions that cross domains.
- What shipping needs from every domain: a release's steps run through shipping's `StepRunner` over `procgroup::Program` and
  reach's injecting proxy (`src/release_steps.rs`, ADR-0116), and a repository's queue and Releases views are laid out
  from its landings and releases (`src/repo_view.rs`, ADR-0114), pushed whole as `repo_updated`.
- `src/lib.rs` re-exports the names `rpc`, `door` and `remit` use, and the crates themselves (`orch::chains`, ...).

## Tests

`tests/app/` is one integration binary that drives the whole application as a channel does, on a real store: orch's own
tests (`tests.rs`) and those of the crates below that need every domain (`<crate>_<module>.rs`). Pure tests stay beside
their module in their crate.

## Verify

```sh
CARGO_TARGET_DIR=$HOME/.cache/remit-target cargo clippy -p orch --all-targets -- -D warnings
CARGO_TARGET_DIR=$HOME/.cache/remit-target cargo nextest run -p orch
```
