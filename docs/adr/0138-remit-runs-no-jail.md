# 0138. Remit runs no jail

Status: accepted, 2026-11-27. Source: the owner's decision of 2026-11-27 ("drop the jail: Remit's users are individuals;
if they harm themselves, that is theirs to bear"), asked again for the programs that hold a source's credential and
confirmed ("drop all of it"). Supersedes ADR-0021. Narrows ADR-0018, ADR-0025, ADR-0029, ADR-0044, ADR-0077 and ADR-0116
where they run a program in a jail.

## Context

`crates/jail` wrapped four kinds of child: an agent's session (`hidden_with`: Remit's secret files hidden, the server's
binary read-only; on Linux user and mount namespaces, on macOS a Seatbelt profile), a source's program, script or stdio
MCP server (`run`, `spawn`: the system read-only, a scratch directory the only path it writes, its only way out a unix
socket to the egress gate or the injecting proxy), a repository's `.remit/checks`, and a release's steps. A host that
could not isolate refused to start a session; there was no unisolated fallback.

Remit is run by one person, on their own machine, as their own OS user. The jail guards that person against agents they
started on their own behalf. It costs a per-OS adapter (namespaces, Seatbelt, a re-exec helper in the `remit` binary), a
class of failures on hosts without unprivileged namespaces, and programs that cannot see the user's own toolchain (a
node under the home directory, `jail_has`).

## Options

1. **Keep the jail everywhere.** What the owner decided against.
2. **Drop the session's jail, keep the one around programs holding a credential.** Asked; the owner chose to drop it all.
3. **Run no jail.** Every child runs as a plain child process of the user.

## Decision

Option 3. `crates/jail` is deleted with every call to it and the `remit` binary's helper entry.

What stays, because it is correctness and not isolation:

- A child runs in its own process group and ends with everything it started when it exits, times out or is dropped
  (ADR-0069, `crates/procgroup`).
- A program still has a deadline and a cap on the output kept.
- A child's environment is the one it is given (`crates/childenv`): nothing of the server's environment leaks in by
  accident, a credential a program needs is put in that program's environment alone.
- A program that reaches a connection still goes through the egress gate or the injecting proxy, now on a loopback port
  set in its proxy variables: it is how a credential is injected and how a call is recorded, not a wall.
- The ask and deny rules on sessions (`crates/chains/src/policy.rs`) stay: they are a tripwire, as they always were.

## Consequences

- An agent's session can read the owner key, the database and every file of the user.
- A program can reach the network directly, around the gate; a credential in its environment can be sent anywhere.
  "No agent sees a credential" holds only as far as redaction of what a program prints.
- A repository's checks and a release's steps run with the user's files and network.
- Remit runs on any host where `claude` runs; nothing refuses to start for lack of isolation.

## Revisit when

Remit serves someone other than the person whose machine it runs on, or a credential that is not the user's own is
given to a program. The seam is the one runner every child goes through: isolation returns as a wrapper there.
