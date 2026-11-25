# Errors

## Core question

**Who acts on this failure, and what do they need to act?** The answer picks the variant, its fields, the layer that
adds context, and the one place that logs it.

## Shape

- `#[derive(Debug, thiserror::Error)]` enums. No anyhow, no `Box<dyn Error>` in our own APIs (a `BoxError` exists only
  where a plugin or a foreign library hands one over, as `source::BoxError`).
- One enum per crate (`chains::Error`, `store::Error`) is the default. A separate enum for one operation is worth it
  when its callers branch on outcomes the crate enum would blur.
- A variant carries **data, not prose**: `Read { path: PathBuf, source: io::Error }`, a retry variant carrying `after: Duration`,
  `NotAdmitted(String)` where the string is the reason shown to the owner. Callers and tests match fields, not text.
- Messages are lowercase, no trailing period, and say what happened and what to do, for whoever reads them: the owner
  in the app, an agent in a tool result (`"it changed since it was read: read it again"`, `crates/store/src/store.rs:34`).
- A large payload is boxed (`Box<Payload>`) so `Result<T, Error>` stays small on the happy path.

## Context once

The layer that knows the context adds it, as fields, once. Layers above pass the error up with `?` and add nothing,
or convert it into their own variant when their caller needs a different question answered.

| Situation | Write |
|---|---|
| the inner error is the whole story | `#[error(transparent)] Store(#[from] store::Error)` |
| the inner error needs a name for this layer | `#[error("telemetry database: {0}")] Db(#[from] rusqlite::Error)` — the house form: the cause is in `Display` |
| the inner error needs *which* file / call | no `#[from]`: `.map_err(|source| Error::Read { path: p.clone(), source })?` |
| a foreign error type crosses a crate boundary | one translator function per boundary, mapping each foreign case to ours |

`#[from]` makes the foreign type part of the enum's public face and lets `?` convert silently. Use it when every
occurrence of that error means the same thing; otherwise map explicitly.

Because `Display` already includes the cause, code that walks `source()` de-duplicates
(`crates/source/src/api.rs:95`). Log with `%err`: `{:?}` prints the nested variant structure (`Store(Disk)`,
`Os { code: 2, .. }`), not the readable messages.

## Log or return

Never both. Return while some caller can still decide; log where the error is handled and goes no further:

- a task's root (`Err(err) => tracing::error!(chain = %id, %err, "start chain")`, `crates/orch/src/tick.rs:79`),
- a loop that skips one item and continues (`warn`, saying what is skipped and what the consequence is),
- the shell's last frame before the user.

`#[instrument(err)]` logs the error at the span; then the caller must not log it again. `inspect_err(|e| error!(..))`
followed by `?` is the same double log.

## Where a failure goes

Decide per failure which of these it is; each has its own path:

| Kind | Path |
|---|---|
| expected outcome (not found, already done, conflict) | a variant the caller matches; `Result<Option<T>, E>` for "found / absent / failed" |
| transient (timeout, 429, connection reset) | a variant that says so, with what the retry needs (`after: Duration`); classified in one exhaustive `match` at the lowest layer that knows, with no `_` arm |
| answer to an agent | two layers, never one string: the inner `Result` is what the model reads and can act on (a tool result with `is_error`), the outer is a protocol failure — `Result<Result<String>, ErrorData>`, `crates/chains/src/tools.rs:121` |
| a bug (an invariant the types could not hold) | restructure so it cannot happen; else `#[expect(clippy::expect_used, reason = "..")]` with the values in the message |
| a panic in a task or a foreign fault domain | `JoinError::is_panic()` or `catch_unwind` **only** at a supervision seam (a pool that must not shrink, `crates/store/src/store.rs:253`; a plugin), turned into an error event |

A panic is not an error channel. Nothing in our code relies on catching one except those seams.

## Idempotency and errors

An operation that fails halfway and is retried must end in the same state as one run. That makes the retry path part
of the error design:

- write with a key (`INSERT .. ON CONFLICT DO NOTHING`, a request id), so a replay is a no-op;
- an "already done" outcome is a success variant or a distinct `Ok`, not an `Err` the caller must swallow
  (`chains::Error::FiredAlready` exists so the effect is not done twice);
- a failure after a side effect records what was done before it returns.

## Testing errors

Assert the variant and its fields: `assert!(matches!(r, Err(Error::Conflict)))`. A test on the English text breaks on
every wording fix and proves nothing about behaviour. A bare `is_err()` passes for the wrong error.
