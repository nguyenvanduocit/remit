# Types and data

## Core question

**Which values must never exist, and where does outside data become trusted?** A type answers the first; one boundary
answers the second.

## Make the illegal state unrepresentable

- **States are enum variants carrying their own data**, not a struct of `Option`s and `bool`s:

  ```rust
  // not: struct Run { started: Option<Time>, ended: Option<Time>, failed: bool, error: Option<String> }
  enum Run { Waiting, Running { started: Time }, Ended { started: Time, ended: Time, outcome: Outcome } }
  ```

  For state that is stored or changes at runtime (a chain, a card), the enum is the tool. Typestate
  (`Conn<Open>`) is for a value moved through steps inside one function's flow, never stored.
- **A newtype holds an invariant only if its constructor is the only door**: a private field, a fallible
  `TryFrom`/`FromStr`/`parse`, no infallible `From<String>`, and serde through the same door (below). A newtype with a
  public field or a `From<String>` holds nothing; it only renames.
- An id becomes a newtype when two kinds of id get swapped at a call site (most ids here are still `String`: a new
  newtype replaces the `String` everywhere in one change, not beside it). Paths are `PathBuf`, durations `Duration`,
  times the codebase's `Time`, never `String`/`i64` stand-ins.
- A parameter that means something is a type: `Visibility::Hidden`, not `true`; a `bool` argument is a flag in disguise.
- **Match exhaustively on enums we own.** List the variants (group them with `|`); a `_` arm turns the next variant
  into a silent default instead of a compile error. Do not put `#[non_exhaustive]` on workspace types: every sibling
  crate counts as foreign and must add `_`.
- `Result<Option<T>, E>` for "found / absent / failed"; never a sentinel (`-1`, `""`, `0`).
- Private fields by default. A `pub` field is right only on passive data with no invariant (a contract record).
- `Deref` only for smart pointers and transparent wrappers, never to borrow another type's methods.

## Parse at the boundary, once

Outside data (HTTP body, a tool's arguments, a child's output, a stored row, a config file) is parsed into the domain
type where it enters. Inside, the type is trusted and nobody re-checks.

- `#[serde(try_from = "Raw")]` (and `into = "Raw"` for writing) routes serde through the validating constructor, so
  `Deserialize` cannot build what `new` refuses. A plain `#[derive(Deserialize)]` on a guarded newtype is a back door.
- A wire struct full of `Option`s is resolved once into a domain type with the options decided, at one function.
- `Option<Option<T>>` with field-level `#[serde(default, deserialize_with = ..)]` when a patch must tell "omitted" from
  "set to null" (`serde_with` is not a dependency; a ten-line function is enough).

## Data read across versions

Stored records and the contract (`crates/model`, `crates/wire`) are read by code of another version: an older server
reads a newer record after a rollback, a client reads a newer server's push.

- **Stored data migrates forward once**, in a numbered migration (`crates/store/migrations/NNNN.sql`). Runtime code
  reads one shape. No `alias`, no tombstone fields, no "old or new" branches in readers.
- **A field added later** gets field-level `#[serde(default)]`, not container-level (which hides typos in names).
- **`deny_unknown_fields`** is for input we refuse to guess about (a config, a tool's arguments). A type read across
  versions must accept fields it does not know.
- **An enum read across versions** needs a home for a variant it does not know. `#[serde(other)]` works only on a unit
  variant (`Unknown`); keeping the raw value to round-trip it needs a hand-written `Deserialize`.
- Serde's buffering modes (`flatten`, internally tagged, untagged) cannot carry `u128`/`i128`. `deny_unknown_fields`
  on a struct that flattens another works on the outer struct but not on the flattened inner one, and is meaningless
  next to a flattened map (the map takes every unknown key). Internally tagged enums cannot hold tuple variants or
  newtypes over primitives. Untagged enums pick the first variant that parses and report errors poorly: prefer a tag.
- Changing a stored or contract type means a test that reads the old shape (`AGENTS.md`: stored data and the contract
  are what tests exist for).

## Ownership in signatures

- Borrow to read (`&str`, `&[T]`, `&Path`, `&T`); take `T` when the function stores or consumes it, so the caller
  decides whether to clone. Never borrow and then clone inside.
- `mem::take(&mut x)` / `mem::replace` move a value out of `&mut` without a clone, including an enum state transition
  (`let old = mem::replace(&mut self.state, State::Waiting);`).
- A function that consumes an argument and can fail returns the argument inside the error when the caller may retry
  with it.
- A `move` closure inside a method captures what it names; bind a field to a local first
  (`let id = self.id.clone(); move || use(id)`) so it does not keep `self` alive.
- Drop order is behaviour: struct fields drop in declaration order (top first), locals in reverse (last declared
  first). A field that must be dropped before another is declared above it: a transaction above the connection guard
  it runs on.

## Naming

- Conversions: `as_x` is free and borrows, `to_x` costs, `into_x` consumes. Getters have no `get_`. `iter`, `iter_mut`,
  `into_iter`.
- One word order per kind of thing across the workspace (`ParseError` vs `ErrorParse`: pick the existing one).
- Full domain names over short ones re-exported under a module (`EntranceKey`, not `entrance::Key` here and `Key`
  there): one concept, named the same everywhere, findable with `rg`.
- Derive only what is used. `Clone`, `PartialEq`, `Default`, `Serialize` each make a promise; add one when a caller
  needs it. A type is never both `Copy` and an `Iterator`.
