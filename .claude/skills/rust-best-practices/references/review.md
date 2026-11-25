# Reviewing a Rust diff

Read the diff as the person who maintains it next year. The lints already passed; look for what compiles and is wrong.
Before flagging, check it is not deliberate: a `#[expect(.., reason)]` or a comment that states the trade-off settles it,
unless the reason is false.

Run each **Detection** over the changed files only:

```sh
git diff --name-only --diff-filter=d HEAD -- '*.rs' | xargs rg -n '<pattern>'
```

A hit is a place to read, not a verdict. Each item ends with where its rule is explained.

## Correctness (block the change)

1. **A task or child outlives its owner.** A `tokio::spawn` whose handle is dropped, with no channel or token that ends
   it. Detection: `^\s*tokio::spawn\(` → `async.md`, *Owning tasks*.
2. **A loop's `select!` drops a non-cancel-safe future** (`read_exact`, `write_all`, `send`, `lock`) or keeps progress
   inside the future. Detection: `select!` → `async.md`, *Cancellation safety*.
3. **A stop branch without `biased;`**, or a stop that waits without a bound. Detection: `cancelled\(\)` → `async.md`,
   *Stopping*.
4. **Blocking work in async**: `std::fs`, rusqlite, a CPU loop on a runtime thread. Detection: `std::fs::|Connection::`
   in a file with `async fn` → `async.md`, *Blocking work*.
5. **An error both logged and returned.** Detection: `inspect_err\(.*!\(` and an `error!`/`warn!` just before a
   `return Err`/`?` → `errors.md`, *Log or return*.
6. **A retry that is not idempotent**: a write without a key or `ON CONFLICT`, a side effect before a fallible step with
   nothing recorded. → `errors.md`, *Idempotency*.
7. **A stored or contract type changed** without a migration and without a test reading the old shape; a
   `deny_unknown_fields` or a missing `#[serde(default)]` on a type read across versions. Detection:
   `deny_unknown_fields|#\[serde` in `crates/model`, `crates/wire` → `types-and-data.md`, *Data read across versions*.
8. **A secret can reach a log**: a derived `Debug` on a type holding a token, key or password; `#[instrument]` without
   `skip_all` on a function taking one. Detection: `derive\(.*Debug` near `token|secret|password|key` →
   `processes-and-secrets.md`, *Secrets*.
9. **A child process** started around `procgroup`, with the parent's whole environment, or with stdin left open.
   → `processes-and-secrets.md`.
10. **A `_` arm on an enum we own**, hiding the next variant. Detection: `_ =>` → `types-and-data.md`.

## Design (fix in the same change)

11. **The concept now has two owners**: a type copied rather than imported from `crates/model`/`crates/wire`, a rule
    implemented a second time, a second path to open the DB or start a process.
12. **Logic that should be pure sits in the shell**: a decision inside an `async fn` next to I/O, so testing it needs a
    fake. Move it into a function of values.
13. **A trait with one implementor**, or a trait declared by the crate that implements it rather than the one that uses
    it. Detection: `^pub trait` → `SKILL.md`, *Design*.
14. **State as `Option`s and `bool`s**, where an enum would make the bad combinations unwritable. Detection: several
    `: Option<` or `: bool` fields in one new struct → `types-and-data.md`.
15. **A `String`, `i64` or `bool` standing for a domain concept** in a new signature (a path, a duration, a mode).
16. **A global**: a new `static`, `LazyLock`, `OnceLock` or thread-local holding a dependency. Detection:
    `^\s*(pub )?static |LazyLock|thread_local!`.
17. **`.clone()` added to satisfy the borrow checker.** Detection: `git diff -U0 | rg '^\+.*\.clone\(\)'` →
    `compiler.md`.
18. **`tokio::sync::Mutex` or `Arc<Mutex<_>>`** where one owning task and a channel would do. Detection:
    `tokio::sync::Mutex|Arc<Mutex` → `async.md`, *Locks*.
19. **`#[from]` on a broad foreign error** (`io::Error`) where the variant needs which file or call. Detection:
    `#\[from\] (std::)?io::Error` → `errors.md`, *Context once*.
20. **An `unbounded_channel` without a comment** saying why its producer is bounded. Detection: `unbounded_channel`.
21. **A new type beside an old one**, an old path re-exported, a deprecated function kept for callers: the change is
    not whole.
22. **`#[non_exhaustive]` on a workspace type.** Detection: `non_exhaustive`.

## Tests

23. **A test that would pass if the code returned nothing**, restates a constant, or mirrors the implementation.
24. **A mock or a trait made for a test.** Detection: `mockall|automock|impl .* for Mock|Fake`.
25. **An assertion on error text** or a bare `is_err()`. Detection: `is_err\(\)|to_string\(\)\.contains`.
26. **Time in a test**: a `sleep` used to wait for something. Detection: `thread::sleep|time::sleep` in a test →
    `async.md`, *Tests*.
27. **A test outside the rails, stored data, the contract or a main path** that adds runtime: question whether it
    belongs (`AGENTS.md`, *Few tests*).

## The diff itself

28. A comment that says *what* instead of *why*, a doc that narrates the change ("now uses X"), a TODO that is really a
    defect, a "guidelines followed" note.
29. Code created by this change that lost its last caller (`rg` the name: zero callers → delete).
30. The checks ran: `cargo clippy -p <crate> --all-targets -- -D warnings` and `cargo nextest run -p <crate>` for each
    crate touched.
