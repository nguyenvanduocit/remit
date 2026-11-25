# When the compiler pushes back

A borrow or lifetime error is usually a design question asked in compiler words. Answer the question; the fix follows.
After the third failed attempt at the same error, stop editing and re-read this table.

## Error → design question

| Error | Do not reach for | Ask instead |
|---|---|---|
| E0382 use of moved value | `.clone()` | Who should own this? Should the first use borrow, or should the value be split so each user owns its part? |
| E0505 / E0506 move or assign while borrowed | reordering until it compiles | Does the borrow outlive what it needs? End it in a block, or take the data out with `mem::take`. |
| E0499 / E0502 two mutable borrows, or `&` and `&mut` | `RefCell`, `unsafe`, cloning `self` | Is one struct doing two jobs? Borrow disjoint fields directly (`let a = &mut self.a; let b = &self.b;`) instead of through `&mut self` methods; or split the struct. |
| E0597 / E0716 does not live long enough, temporary dropped | `'static`, `Box::leak` | Should the caller own it? Return an owned value, or bind the temporary to a `let`. |
| E0106 missing lifetime | sprinkling `'a` | Does the output borrow from an input? If it does not, return owned data. |
| E0373 closure may outlive the function | `Arc<Mutex<_>>` everywhere | Should the task own its data (`move` with owned values), or should it not outlive the scope (`std::thread::scope`, a `JoinSet` awaited here)? |
| "future cannot be sent between threads safely" | `tokio::sync::Mutex`, `spawn_local` | Which `!Send` value is alive across which `.await`? The `note:` lines name both. Close its scope before the await (`references/async.md`, *Locks*). |
| E0038 "the trait `X` is not dyn compatible" | dropping `dyn` for generics everywhere | Is `dyn` needed? If it is: `where Self: Sized` on the generic or `Self`-returning methods, `#[async_trait]` for async methods. |
| E0277 trait bound not satisfied | adding the bound up the whole call chain | Should this function be generic at all? A concrete type often removes the chain. |
| E0004 non-exhaustive patterns | `_ => {}` | What should the new variant do here? Write its arm. |
| E0308 mismatched types with `as` denied | `as` | Is it lossless (`From`) or not (`TryFrom` and a typed error)? |

A type that fights every borrow (lots of `&mut self` methods touching overlapping fields, `Rc<RefCell<_>>` graphs) is
usually holding two concepts. Split along what each part owns.

## Edition 2024: behaviour that changed

The workspace is edition 2024 (`Cargo.toml`, `[workspace.package]`). Code and advice written for 2021 can be wrong here.

- **`if let` temporaries drop before `else`.** In `if let Some(v) = cache.lock()?.get(k) { .. } else { cache.lock()?.. }`
  the first guard is gone by the `else`, so it no longer deadlocks. Code that relied on a temporary living through
  `else` must bind it to a `let` first.
- **Tail-expression temporaries drop before the block's locals.** A temporary in a block's last expression that
  borrowed a local used to outlive it and fail to compile; now it compiles, and a guard created in the tail
  expression is dropped earlier than in 2021.
- **`-> impl Trait` captures every in-scope lifetime by default** (type parameters were always captured). When the
  returned value does not borrow an argument, say so with precise capturing, or the caller's borrow is held longer than
  needed: `-> impl Iterator<Item = u32> + use<>` in a function without type parameters, `+ use<T>` (leaving out only
  the lifetimes) in one with them. A type parameter cannot be left out of `use<..>`.
- **Let-chains are stable**: `if let Some(x) = a && let Ok(y) = parse(x) && y > 0 { .. }` replaces nested `if let`s.
- **Async closures** (`async || ..`) and the `AsyncFn*` traits are stable. They borrow from their environment where a
  closure returning an `async` block could not. Their returned future cannot be required to be `Send`
  (`references/async.md`, *Async traits*).
- **`unsafe` moved**: `std::env::set_var`/`remove_var` are `unsafe`, `extern` blocks are `unsafe extern`, attributes
  like `no_mangle` are `#[unsafe(no_mangle)]`. With `unsafe_code = "forbid"`, all of these are out of reach here.
- **`gen` is a reserved keyword**: a name `gen` must be `r#gen` (as `r#box` in `secrets::Sealed`).
- **The never type falls back to `!`, not `()`.** A generic inferred only from a diverging branch now resolves to `!`:
  `let x = if ok { make() } else { return Err(..) };` with `fn make<T: Default>() -> T` fails with `E0277: !: Default`.
  Annotate the type (`let x: Settings = ..`).

## Other current-Rust facts often gotten wrong

- `std::sync::Mutex` and `std::sync::mpsc` are neither deprecated nor slow (`mpsc` has used crossbeam's algorithm since
  1.67). `parking_lot` and `crossbeam` add a dependency and nothing this codebase needs.
- `std::sync::LazyLock` / `OnceLock` replace `lazy_static` and `once_cell`. A static is still a global: only for constant
  tables (`crates/capability/src/capabilities.rs`), never for a dependency.
- `#[expect(lint, reason = "..")]` (not `#[allow]`) is required here and warns when the finding goes away.
- `let-else` (`let Some(x) = y else { return .. };`) is the early return for an expected miss.
- `&T: Send` when `T: Sync`; `&mut T: Send` when `T: Send`; `Arc<T>` is `Send + Sync` only when `T: Send + Sync`;
  `Mutex<T>: Sync` when `T: Send`.
