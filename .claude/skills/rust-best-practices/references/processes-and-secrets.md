# Child processes, output, secrets, foreign code

Remit runs agents, their CLIs, checks and plugins as plain child processes of the user (ADR-0138); a program that
reaches a connection goes through an egress proxy. This is where a Rust mistake becomes a hang, an orphan, or a leaked key. Every entrance has one owner; use it.

## Core question

**When this child, or its children, misbehave or outlive us, what holds?**

## Starting a process

- Start it with `procgroup::command(name, args)` (`crates/procgroup/src/procgroup.rs:38`). It makes the child lead its
  own process group, and dropping the `Child` before it was reaped (a dropped future, a `timeout`) kills the whole group:
  the agent *and* the CLI it spawned. `Command::new` anywhere else is a `disallowed_methods` finding (clippy.toml), so
  it does not pass `cargo clippy -- -D warnings`.
- A child program that runs to its end (a source's program or script, a repository check, a release step) or a stdio
  server is a `procgroup::Program` (`crates/procgroup/src/program.rs`): `run(timeout, max_output)` or `start()`. It is
  the one runner and the seam where isolation would return; never build a second one beside it.
- Its environment is `childenv::of(parent, extra)` (`crates/childenv/src/childenv.rs:3`): an allowlist of what the
  child inherits, then the caller's own `extra`. Never pass the server's environment through whole: it holds keys.
  `env::set_var` is `unsafe` in edition 2024 and `unsafe` is forbidden: the environment is built, never mutated.
- stdin is `Stdio::null()` unless we write to it. A child that sees an open empty pipe may wait on it forever.
- What the child runs is the owner's or an agent's configured command, never text from a request.

## Reading its output

- Output is unbounded. Keep a bounded head and a ring-buffer tail, count what was dropped in between, and **keep
  draining** after the cap, or the child blocks on a full pipe and never exits. Slice with `.get(..n)`, count with
  `saturating_*`.
- A grandchild that inherited stdout keeps the pipe open after the child exits. Reading "until EOF" then hangs.
  `procgroup` bounds that wait with `WAIT_DELAY` and reports `Error::WaitDelay`; reuse it rather than writing another
  read-to-end.
- Read stdout and stderr concurrently (both pipes can fill). Text from a child is not UTF-8 until checked:
  `String::from_utf8_lossy` for display, a parse error for protocol.
- A line-delimited protocol (stream-json from Claude Code) is parsed per line at the boundary into a typed event; a
  line that does not parse is an error event with the raw line, not a skip.

## Stopping a process

Close stdin (the polite stop for a CLI), wait a bounded grace period, then kill the group. Clear listeners, then drain
tasks, then drop resources. A stop that waits forever on a stuck child is not a stop: pause and stop are rails.

## Egress

- The gate and the injecting proxy listen on a loopback TCP port; the runner sets it as `HTTPS_PROXY` and `HTTP_PROXY`,
  with `NO_PROXY` empty. They record a call and inject a credential; they do not confine, and a program can go around
  them (ADR-0138).
- An egress decision on a host name resolves it and refuses any address that is not public (loopback, private,
  link-local, metadata ranges), allowlisted names included; then connects to the address it checked, so a second DNS
  answer cannot swap it (DNS rebinding).
- A change to any of this is a rail change: its ADR names the control not built yet (`AGENTS.md`).

## Secrets

- A type that holds a secret has a hand-written `Debug` (and no `Display`) that prints its name and not its value,
  ending in `.finish_non_exhaustive()`. A derived `Debug` on a struct with a token field leaks it into the first log line
  that prints the struct.
- `#[tracing::instrument(skip_all, ..)]` on anything that takes a secret, so it never becomes a span field.
- A secret at rest is sealed (`secrets::seal`, with the slot's name as `aad`); its plaintext lives only in the call
  that uses it. No agent ever sees one (`AGENTS.md`); a credential goes to an inbox item's write-only field.
- `childenv::secret_names` tells which names in an environment are secret: redact by that, not by a second list.

## Foreign code: plugins, MCP servers, sources

- Each plugin is its own fault domain: one that fails to load or panics becomes an inert record with its error, and the
  rest load. A panic is caught only at that seam.
- Text that reaches a model from outside is length-capped and its whitespace collapsed; an id from outside is
  restricted to `[A-Za-z0-9_-]` before it becomes a path, a key or a tool name.
- Every call out has a timeout, and its failure is a typed error the agent can read.
