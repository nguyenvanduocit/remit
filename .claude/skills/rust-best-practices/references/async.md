# Async and tokio

The lints catch a std guard held across `.await`. Everything below compiles and is still wrong.

## Core question

**Who owns this task, and what happens to its half-done work when it is cancelled?** Every future can be dropped at any
`.await`: by a `select!` that took another branch, a `timeout`, an `abort()`, or its owner's drop. Code is correct only
if every such point leaves state as it was or as it should be.

## Cancellation safety

A future is cancel-safe when dropping it at an `.await` loses nothing.

| Cancel-safe (fine in a looping `select!`) | Not cancel-safe (loses data or its place) |
|---|---|
| `mpsc::Receiver::recv`, `broadcast::Receiver::recv` | `AsyncReadExt::read_exact`, `read_to_end`, `read_line` into a buffer kept in the future |
| `watch::Receiver::changed` | `AsyncWriteExt::write_all` (part written, part not) |
| `CancellationToken::cancelled` | `mpsc::Sender::send` (loses its slot in the queue; the value is dropped) |
| `Interval::tick`; a `sleep_until(deadline)` pinned outside the loop | `Mutex::lock`, `Semaphore::acquire` (loses its place in the fair queue); a `sleep(d)` created inside the loop (restarts every iteration, never fires while other branches are busy) |
| `JoinSet::join_next`, `TcpListener::accept` | any `async fn` of ours that does two awaits with a state change between them |

To use a non-cancel-safe operation in a loop, create it once outside and select on a reference:

```rust
// The reader is owned by one future that keeps its own partial state; select! only polls it.
let mut frames = std::pin::pin!(async move {
    loop {
        let frame = conn.read_frame().await?; // never dropped mid-read: this future lives across iterations
        if frames_tx.send(frame).await.is_err() { return Ok::<_, Error>(()); }
    }
});
loop {
    tokio::select! {
        biased;
        () = stop.cancelled() => return Ok(Ended::Stopped),
        ended = &mut frames => return ended.map(|()| Ended::Closed),
        Some(frame) = frames_rx.recv() => handle(frame)?,
        else => return Ok(Ended::Closed),
    }
}
```

Or give the read loop its own task and receive its frames on a channel. Re-creating `conn.read_frame()` on each
iteration (`frame = conn.read_frame() => ..`) is the bug this section is about.

Keep accumulated state (a partial buffer, a counter) **outside** the future, in the loop's owner, so a dropped branch
cannot take it with it. A function of ours that is not cancel-safe says so in its doc.

`CancellationToken::run_until_cancelled(fut)` (tokio-util 0.7.20, the version in `Cargo.lock`) runs `fut` unless
stopped and returns `None` when cancelled. It is biased towards the future when both are ready, and cancel-safe only if
`fut` is: where stop must win, use `select!` with `biased;`.

## `select!` rules

- `biased;` and the stop branch first wherever stop must win a tie. Without it tokio picks a ready branch at random.
- A cancel branch that leaves waiters behind answers them (a `oneshot` dropped is an answer; a pending approval is
  given a "declined" reply), so nothing waits on a task that is gone.
- `else =>` handles "every branch is disabled" (all channels closed). Without it, `select!` panics in that case.
- A pattern that does not match disables its branch for this call: `Some(x) = rx.recv()` stops polling a closed channel.

## Owning tasks

Dropping a `JoinHandle` **detaches** the task: it runs on with nobody to stop it or see its panic.

| Shape | Owner |
|---|---|
| a group of tasks that live and die together | `JoinSet` (dropping it aborts them all; `join_next` reaps) |
| one background task tied to a struct's life | an `AbortOnDropHandle` field (tokio-util) |
| tasks that must finish before shutdown completes | `TaskTracker` (`close()`, then `wait()`), as `crates/airuntime/src/session.rs:46`. It only waits: dropping it does not abort its tasks, so pair it with the token they watch |
| a detached `tokio::spawn` | only when the task ends by itself once its owner goes (its input channel closes, it watches the owner's token) and nothing waits for its result; the line says which. The codebase has ~50 of these; each must pass that test |

Every `JoinError` is looked at: `is_panic()` is a bug in the task and is logged as one with what the task was doing;
`is_cancelled()` is the normal end of an aborted task.

## Stopping

Pause and stop are rails: they must work while everything else is stuck.

1. `token.cancel()`. Each subsystem got `parent.child_token()`, so a stop reaches the whole subtree and a child's
   cancel does not reach its parent.
2. Wait for the tasks to end, bounded: `timeout(grace, tracker.wait())`.
3. Whatever did not end in the grace period is aborted (`JoinSet::abort_all`, `AbortOnDropHandle` dropped,
   `JoinHandle::abort`; a `TaskTracker` cannot abort), and the timeout is logged with what was still running.

Never a fixed `sleep(5s)` as "graceful": it loses work when slow and wastes time when fast. After `run()` returns,
`token.is_cancelled()` tells "stopped" from "finished".

## Blocking work

A blocking call inside `async` stalls a worker thread and every task scheduled on it.

- rusqlite, fsync, file locks, compression, a big parse, a CPU loop → `tokio::task::spawn_blocking`. The store wraps
  every read and write this way (`crates/store/src/store.rs:249`); call through it, never around it.
- A `spawn_blocking` closure cannot be aborted. A long one checks a stop flag (`Arc<AtomicBool>`, `Relaxed` is enough)
  between units of work.
- `tokio::fs` is itself `spawn_blocking` per call: fine for one read, slow for a loop of small ones. Do the loop in one
  `spawn_blocking` with `std::fs`.
- CPU fan-out is a small, bounded pool reached through one `spawn_blocking`, not `tokio::spawn` per item.

## Locks

- `std::sync::Mutex` is the default in async code when the guard never crosses an `.await`; it is faster than tokio's.
  Copy what you need out in a block, let the guard drop, then await.
- `tokio::sync::Mutex` only when a guard must be held across an `.await`, and that need is usually a design smell:
  state touched across awaits wants **one owning task** fed by a `mpsc` of commands, answering on `oneshot`s.
- A poisoned std lock: `lock().unwrap_or_else(PoisonError::into_inner)` only when a panic mid-update cannot leave the
  data inconsistent (a cache, a pool of connections whose transaction rolled back). Otherwise reset or rebuild the
  value. `crates/store/src/telemetry.rs:144` is the recover case.
- A non-`Send` value (`Rc`, a `RefCell` borrow, a std guard) alive across `.await` makes the whole future `!Send`, and
  `tokio::spawn` refuses it. The error's main span is the spawn; its `note:` lines name the value and the `.await` it
  is held across. Narrow the value's scope with a block.

## Channels

| Need | Channel |
|---|---|
| work items, commands to an owning task | `mpsc::channel(n)`, bounded: a full queue is backpressure, by design |
| one answer to one request | `oneshot` |
| the latest state, readers that only need the newest | `watch` |
| every subscriber sees every event | `broadcast(n)`, and handle `RecvError::Lagged(k)`: say what `k` lost events mean (resync from the store) |

An `unbounded_channel` carries a comment at its site saying why its producer is bounded already (the codebase has
some; each is a decision, not a default). Choose by the producer: an outside caller filling a queue must be bounded;
an internal loop must not block on a slow consumer.

`tx.send(v).await` returns `Err` when the receiver is gone. That is information (the consumer ended), not noise:
end the loop or log at `debug` what it means.

## Bounding concurrency and time

- Many concurrent calls: a `JoinSet` filled up to `n` at a time, or an `Arc<Semaphore>` with `acquire_owned()` moved
  into each spawned task. (`futures`' `buffer_unordered` is not a dependency of any crate; adding it is a decision.)
- Every call to something outside the process (HTTP, a child's stdout, a plugin, the forge) is wrapped in
  `tokio::time::timeout`, and `Elapsed` becomes a typed variant naming what timed out. A timeout drops its inner future:
  cancellation safety applies to it too.
- A stream with a terminator (SSE, a Claude Code turn) re-arms an idle timeout on every read and treats EOF before the
  terminator as an error, not a clean end.

## Async traits

| Trait is used as | Write |
|---|---|
| generic only (`T: Trait`) | native `async fn` in the trait, or `fn f(&self) -> impl Future<Output = T> + Send` when callers spawn it (`crates/door/src/lib.rs:60`) |
| `dyn Trait` (`Arc<dyn Trait>`) | `#[async_trait]` (native async fn in traits is not dyn-compatible), as `crates/orch/src/domains.rs:24` |

A native `async fn` in a public trait does not promise `Send` futures; when the caller needs `Send`, spell the
`impl Future + Send` signature. A generic `F: AsyncFn(..)` cannot require its returned future to be `Send`; a closure
passed to `tokio::spawn` takes `F: Fn() -> Fut, Fut: Future + Send + 'static` instead.

## Tracing in async code

- `#[tracing::instrument(skip_all, fields(chain = %id))]`: `skip_all` keeps secrets and large values out of spans;
  name the fields you want.
- A spawned future carries its span with `.instrument(span)`; a `span.enter()` guard held across `.await` leaks the span
  into whatever runs next on that thread.
- Structured fields, not text: `tracing::warn!(chain = %c.id, %err, "change not read")`. `%` for `Display`, `?` for
  `Debug`.

## Tests

`#[tokio::test(start_paused = true)]` freezes time: `tokio::time::advance(d)` moves it, sleeps resolve instantly and in
order. It needs tokio's `test-util` feature in that crate's `[dev-dependencies]`; no crate enables it yet, so the first
test that needs it adds it. Call `tokio::task::yield_now().await` before `advance` so spawned tasks reach their timers first. Wait on an
event (a channel, a `Notify`), never on `sleep`.
