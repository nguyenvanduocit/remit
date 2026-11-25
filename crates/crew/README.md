# crew

The organisation (ADR-0096): objectives and key results, employees (hire, standing, names, faces), memory, mentors and
helps (`src/mentor_act.rs`: what a mentor's device does, once per request), teaching, the coach, duels, outcomes, the
analyst, feedback. The service is `Crew` (`src/service.rs`), holding the `Pipeline`, `Capabilities` and `Keys` it calls.
Depends on `chains`, `reach`, `pipeline`, `capability` and `keys`.

## Verify

```sh
CARGO_TARGET_DIR=$HOME/.cache/remit-target cargo clippy -p crew --all-targets -- -D warnings
CARGO_TARGET_DIR=$HOME/.cache/remit-target cargo nextest run -p crew -p orch
```
