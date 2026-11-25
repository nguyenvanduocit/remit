---
name: rust-best-practices
description: >
  How Rust is written in Remit: toolchain 1.99.1, edition 2024, tokio, thiserror, a deny-wall of lints, a sans-IO core.
  Use whenever you write, change, review or debug Rust in crates/ — a new function or type, an error enum, async or
  tokio code (select!, tasks, channels, locks, shutdown), a child process, serde types that are stored or cross the
  wire, a compile error or lint finding you cannot clear, or a review of a Rust diff.
---

# Rust in Remit

This skill holds what the compiler and the lints cannot: the design behind a change and the traps that compile. What
the lints enforce is not repeated here; how to satisfy them is. House rules come from `AGENTS.md`; this file applies them
to Rust. Detail lives in `references/`: load only the one the task needs.

| Task | Load |
|---|---|
| async, tokio, `select!`, tasks, channels, locks, timeouts, shutdown, blocking work | `references/async.md` |
| an error type, `?`, where to log, retries, errors an agent or the owner reads | `references/errors.md` |
| a new type, newtype, enum, serde on stored or wire data, parsing input | `references/types-and-data.md` |
| a child process, its output, the jail, egress, secrets, plugins | `references/processes-and-secrets.md` |
| a compile error you do not understand, edition 2024 behaviour | `references/compiler.md` |
| reviewing a Rust diff | `references/review.md` |

## The order rules win in

When two rules pull apart, the earlier one wins.

1. **Correct.** No value is silently changed, lost or doubled; no task, child or lock outlives its owner.
2. **The illegal state cannot be written.** A type, not a check, holds the invariant.
3. **One owner.** A fact, a rule, an entrance (opening the DB, spawning a process) lives in one place.
4. **The core is pure.** Decisions are functions of values; I/O sits at the edge.
5. **Small and named.** Each unit does one thing nameable with one verb.
6. **Fast.** Only when measured, and only when asked (hardening waits).

## Writing under the lint wall

`Cargo.toml` `[workspace.lints]` denies these, and `clippy.toml` bans the methods in its own row. Tests are exempt
from the first two rows only (unwrap, expect, panic, indexing); the rest hold in tests too. A clippy finding does not stop
`cargo build`; `cargo clippy -- -D warnings`, which every commit runs, does. Write the idiom, not the workaround.

| Denied | Write instead |
|---|---|
| `unwrap_used`, `expect_used`, `panic` | `let Some(x) = .. else { return Err(Error::Missing { .. }) };`, `.ok_or(Error::..)?`, or a type where the case cannot occur |
| `indexing_slicing` | `.get(i)`, `.first()`, `split_first()`, `chunks_exact`, slice patterns `[a, b, rest @ ..]`, iterators over indices |
| `arithmetic_side_effects` | `checked_add(..).ok_or(Error::Overflow)?`; `saturating_*` only where clamping is the domain's own answer (a counter, a remaining budget); `Duration`/`Instant` methods |
| `as_conversions` | `u64::from(x)` when lossless; `u32::try_from(x).map_err(..)?` when not; `f64::from(u32)` |
| `let_underscore_must_use`, `unused_must_use` | handle it: `if let Err(err) = tx.send(v) { tracing::debug!(%err, "receiver gone: ..") }` — a closed channel is a state, say what it means |
| `allow_attributes(_without_reason)` | `#[expect(clippy::x, reason = "why it is right here")]`, which warns once it no longer applies |
| `std::process::Command::new`, `rusqlite::Connection::open*` (clippy.toml) | `procgroup::command` + `childenv::of`; open the DB through `crates/store`. The owner itself says so with `#[expect(clippy::disallowed_methods, reason = "..")]` |
| `unsafe_code` (forbid) | there is no escape: `env::set_var` (unsafe in edition 2024), `pre_exec`, FFI go through a crate that owns them safely, or the design changes |

A finding that is right where it stands keeps its line with `#[expect]`: the reason names the invariant
(`reason = "the store owns the database: this is one of its own opens"`, `crates/store/src/store.rs:349`). The same
`#[expect]` more than 5 times for the same idiomatic code means the lint is wrong for the codebase: say so; do not
scatter it.

## Design

- **Functional core.** A decision is a pure function of values: `pub fn step(c: &Chain, w: &WakeSubs, cmd: &ChainCmd,
  now: Time) -> Result<Step>` returns the next state and the `Effect`s to perform (`crates/chains/src/step.rs:395,498`).
  The shell performs them. Time, randomness and ids come in as arguments, so the core needs no clock, no mock and no
  `async`. If a test of a rule needs a fake, the rule is in the wrong layer.
- **Dependencies are struct fields.** No `static`, `OnceLock`, `LazyLock` or thread-local whose value changes behaviour.
  A constant table is fine; a hidden service is not.
- **Abstraction climbs only when forced**: concrete type → `enum` over a closed set → generic `T: Trait` → `dyn Trait`.
  A trait exists when a second implementation ships (an I/O edge, a plugin), and it is declared by the crate that
  *consumes* it, holding only the methods that crate calls. A trait with one implementor is the concrete type.
- **No `Manager`, `Service`, `Helper`, `Util`, `Factory`.** Name the thing by what it is in the domain; a function that
  makes `Foo` is `impl Fn() -> Foo` or `Foo::new`.
- **Signatures**: take `&str`, `&[T]`, `&Path` (never `&String`, `&Vec<T>`); take ownership only when the value is
  stored; keep `Arc`, `Mutex`, `RefCell` out of signatures unless sharing is the point; parameters that mean something
  are types, not `bool`s (`Visibility::Hidden`, not `true`).
- **One public path per item.** After a move, delete the old path: no `pub use` left behind for old callers, no
  re-export of another crate's items, no prelude. A crate root flattening its own modules (`pub use account::*;` in
  `crates/model/src/lib.rs`) is the existing pattern, not a second path.
- **A helper is extracted at its third occurrence.** Two stay inline. DRY is about knowledge, not text.
- **Comments say why**: the invariant, the outside fact, the failure a line prevents. A magic number is a named `const`
  whose doc says why that value and what breaks if it changes.

## Errors in one screen

Full rules: `references/errors.md`.

- `thiserror` enums, one per crate or per operation where callers act differently; no anyhow in this workspace.
- Context is added **once**: as fields of the variant (`Error::Read { path, source }`), at the layer that knows it.
- An error is **returned or logged, never both**. It is logged where it is handled: a task root, a loop that goes on,
  the shell.
- A wrapping variant either prints its cause (`#[error("store: {0}")] Store(#[from] store::Error)`, the house form) or
  is `#[error(transparent)]`. Whoever walks `source()` must de-duplicate (`crates/source/src/api.rs:95`).
- Variant messages are lowercase, no trailing period, written for the person who reads them (often the owner or an
  agent): what happened and what to do (`"it changed since it was read: read it again"`).
- A bug the type system cannot exclude is still not a `Result` the caller cannot act on: restructure first; a true
  invariant keeps `#[expect(clippy::expect_used, reason = "..")]` with a message holding the values.

## Async in one screen

Full rules: `references/async.md`.

- Every `select!` branch that loses is **dropped**. In a loop, use only cancel-safe futures (`mpsc::Receiver::recv`,
  `watch::Receiver::changed`, `CancellationToken::cancelled`, `sleep`), or pin the one future outside the loop.
- `biased;` with the stop branch first wherever stopping must win.
- Every task has an owner: `JoinSet` or `AbortOnDropHandle` (tokio-util) abort on drop; a `TaskTracker` only waits. A dropped `JoinHandle`
  detaches its task: a detached task is right only when it ends by itself once its owner goes (its channel closes, it
  watches a token).
- Stop = `CancellationToken` (one `child_token()` per subtree) → bounded grace → `abort()`. Never a fixed `sleep`.
- Blocking work (rusqlite, fsync, file locks, CPU) runs in `spawn_blocking`; the store already does this
  (`crates/store/src/store.rs:249`). A blocking task cannot be aborted: it must end on its own.
- `std::sync::Mutex` when the guard never crosses `.await` (the lint checks it); copy out, drop, then await. Shared
  state that must be held across awaits belongs to one task fed by a channel.
- Every channel is bounded unless the site says why not; `watch` when only the latest value matters; handle
  `RecvError::Lagged`.
- Every external call has a `timeout`, mapped to a typed variant.

## Tests

The rule from `AGENTS.md`: few tests, for the rails, stored data and the contract across versions, and a chain's main
path. The compiler and the lints carry the rest.

- A test drives the code as its real caller does and asserts what that caller observes. A test that restates a
  constant or mirrors the implementation's branches is deleted.
- The pure core is tested by calling it with values. No mockall, no trait made only for a test. At a network edge,
  a real local server beats a trait mock.
- Assert a rejection by its variant (`assert!(matches!(r, Err(Error::Conflict)))`), never by its English text.
- Timers: paused time (`references/async.md`, *Tests*); wait for an event, never `sleep`.
- A bug fix starts with the smallest test that fails before it and passes after.

## Defaults to resist

These are an agent's reflexes; each one is usually wrong here.

| Reflex | Instead |
|---|---|
| `.clone()` to quiet the borrow checker | ask who owns it (E0382/E0505/E0502 → `references/compiler.md`) |
| `Arc<Mutex<T>>` for shared mutable state | one owning task and a channel; or a value passed in |
| a trait "for testability" | a pure function of values; the trait waits for a second implementation |
| `Option` field for "not yet", `bool` for a state | an `enum` whose variants carry their own data |
| a `_ =>` arm on an enum we own | list the variants, so a new one is a compile error |
| a builder for a struct with 3 fields | a struct literal, `..Default::default()` if it truly has defaults |
| `#[non_exhaustive]` inside the workspace | nothing: it forces `_` arms in sibling crates |
| a new type beside the old, migrated later | replace it in the same change, every caller included |
| `tracing::error!` then `return Err(e)` | one of them |
| `parking_lot`, `crossbeam`, `anyhow`, `once_cell` | std (`Mutex`, `mpsc`, `LazyLock`) and thiserror already cover it |
| stop at a green build | read the diff as a reviewer (`references/review.md`) |

## When the compiler pushes back

The third failed attempt at the same error is the signal to stop patching and go up a level: the error is usually
about ownership or lifetime design, not syntax. Map the error to its question in `references/compiler.md`, answer the
question, then change the code.

## Before declaring done

For each crate you touched:

```sh
cargo clippy -p <crate> --all-targets -- -D warnings
cargo nextest run -p <crate>
```

Scoped with `-p`, on the shared `CARGO_TARGET_DIR`, in the main checkout. Then read your diff with
`references/review.md`.
