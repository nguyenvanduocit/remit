# 0018. Agents act on connections through grants and one gate

Status: accepted, 2026-10-30; a grant may be held by a repository, for its release (ADR-0116); its two effects, read and write, refined into four by ADR-0027; a foreign MCP server is an extension of form `mcp` by ADR-0077; its waits on the owner, but a secret act, are an agent's (ADR-0087).

## Context

ADR-0015 and ADR-0017 gave agents native tools for reading the outside world and made a connection the one owner of a
credential. Three things are left without a law:

- **Skills tell agents to run programs.** 104 of the 118 skills in the live library (`select name, doc from skills`, counted by
  `curl`, `kubectl`, `aws `, `.sh`, `npx`, `infisical`, `bash`) describe a CLI or a script with a token, which an agent holding only
  Remit's tools cannot follow. Batch work (a loop over 300 shops) and work with no typed tool (`kubectl`) have no path.
- **"Agents never see a secret" is true of the tool path only.** Every agent's shell runs as the server's OS user (ADR-0004).
  Measured on 2026-10-30: `~/.remit/sources.env` holds a production `MONGODB_URI`, and the server process environment holds
  `BYTEBASE_SA_KEY` and `AWS_*`. A shell of the same user reads both.
- **Rules are per case.** "Read only" is enforced one way for SQL, another for Mongo, and not at all for a program; a chain that holds
  a capability holds every connection of its sources (ADR-0017, accepted weakening); a write has no path.

The owner asks for one standard that holds for every form of use (typed tool, CLI, script, MCP server), every effect (read,
write) and every source added later.

## Options

1. **Typed tools only** (status quo): the safest, and each CLI-only system is a new Go file. Batch work and ad hoc analysis stay out
   of reach.
2. **Put the credential in the agent's environment**: immediate, and every prompt injection can print it. Read-only becomes the
   agent's promise.
3. **A law with one gate**: the agent holds a *grant*, never a credential; every call passes one gate; typed tools and programs are
   adapters behind it; the credential exists only inside the executor. **Chosen.**

## Decision

### The law

> An agent never holds a credential. It holds a **grant**: the right to ask an **executor** of Remit to act on a **connection**
> with a stated **effect**. The credential exists only inside the executor.

Four invariants. A way of acting that breaks one is not offered.

1. **Sealed.** A credential value lives only in the executor: the backend, the credential-injecting proxy, or the jail of a program.
   It is never in an agent's context, its environment, a file it can read, an event, a log or a transcript.
2. **Least grant.** What a chain may do is its set of grants `(source, connection, effect)`. A grant is data the server stores and
   `decide` (pure) reads; it is never a sentence in a brief. The Chair asks for grants by connection *name*; it never handles a value.
   A grant of an `effect` above `read`, or of a connection the owner marked protected, raises a change item (`change_item.go`) and
   waits for the owner.
3. **One gate.** Every call an agent makes to an outside system passes `gate` (`internal/source`): it resolves the connection,
   asks `decide`, applies the deadline, redacts, cuts the answer, and records what was asked, by whom, on which connection. Forms are
   adapters behind it; a form with a path around the gate does not exist.
4. **Honest effect.** Every call carries an effect class (`read`, `write`) and the strongest enforcement its form has; where the
   enforcement is weaker than code, the connection says so in data (`enforced`: `code`, `credential`, `rail`), and `list_sources`
   shows it. No tool description claims more than its enforcement.

### Forms: one model, three tiers of exposure

A **form** is how an act is expressed. The tier is how near the credential comes to the code the agent wrote. A source declares the
forms it supports; the gate uses the lowest tier available.

| Form | Where the credential is | Read-only is enforced by |
|---|---|---|
| **T0 typed tool** (ADR-0015) | the backend | the tool's code (`sqlguard`, `mongoguard`, verb) |
| **T1 program behind an injecting proxy** | the proxy, in the backend; the program holds none | the proxy: base address from the connection, allowed verbs and paths, redirects refused |
| **T2 program with environment in a jail** | the jail's environment | the credential's own rights (the owner supplies a read-only credential), the jail's egress, redaction |

A program is a CLI, a script or an MCP server on stdio: one concept, `run`, with the connection it acts on. A foreign MCP server is
no separate concept and is not trusted more than a script.

### Where the executor runs

- **A jail** is a new user, network and mount namespace (`unshare -Urnm`; unprivileged user namespaces are available on this host,
  measured 2026-10-30). Its only network path is the proxy's socket; its only files are its work directory and read-only system
  paths. A T2 jail's egress is the connection's `Address` and nothing else.
- **An agent session** runs in a mount namespace that hides the paths holding Remit's secrets (the key file, the database, the
  source env files). This closes ADR-0004's gap on the host side that needs no root; the session keeps the network it needs to reach
  its model provider.

### What changes in the code

- `internal/grant` (new, pure, imports nothing of the module): `Grant`, `Effect`, `Decision`, and `Decide(grants, action)`.
- `internal/source`: the gate is extracted from `NewTool` (`source.go`) and every form is an adapter behind it; a `Provider` declares
  its `Forms` and, for a program, its `Run` (binary, environment taken from `Vars`, subcommands allowed).
- `internal/orch`: grants are stored per chain (`grants` table, numbered migration), asked by the Chair with `request_grant` and
  listed with `list_sources`; a write or protected grant is a change item. The capability catalog says which grants a playbook may ask.
- `internal/agent`: sessions start inside the hiding namespace; a program starts inside the jail through one function.
- Skills: written to one shape, "to do X use tool Y; when none fits, `run` on connection Z with effect E". Skills that say how to fetch a
  secret (`infisical`, `secret`, `codebase`) are removed from the library.

## Threat model (STRIDE; OWASP LLM01, LLM02, LLM06, LLM08)

| Threat | Control |
|---|---|
| Spoofing: a chain acts on a connection it was not granted | `Decide` at the gate; grants are rows, not text; a test drives each denial |
| Spoofing: staging read as production | Every answer starts with `connection: <source>/<name>`; protected connections need an explicit grant; a grant names one connection |
| Tampering: a "read" writes (a script calls a mutating endpoint) | T0 code guards; T1 proxy verb and path allow-list; T2 the owner's read-only credential, marked `enforced: credential`; a `write` effect is a change item |
| Repudiation: who ran what | The gate records chain, agent, connection, effect, form and the program's argv; a write also records the owner's approval |
| Information disclosure: a secret in an agent's context | Invariant 1; T0 and T1 never expose it; T2 only in a jail, with redaction of every output (`RedactText`) |
| Information disclosure: a script prints or encodes the secret (T2) | Accepted residual: redaction catches token shapes, not an encoded value. Controls: a read-only credential, egress limited to the connection's address, the output visible in the transcript. T1 is chosen wherever the source speaks HTTP |
| Information disclosure: an agent reads the key file, database or env files in its own shell | The session's mount namespace hides them; a test starts a session and fails if one is readable |
| Denial of service: a program loops or floods | Deadline, CPU and memory limit of the jail, answer cut, one program per grant at a time |
| Elevation: a program escapes the jail | User namespace without capabilities, no network but the proxy socket, read-only mounts; a failed escape test is a defect, and a connection without a read-only credential is never given a T2 form |
| Elevation: the Chair lifts a rail by granting itself or another card | `request_grant` only raises an item or records a read grant within the playbook's capabilities; a write or protected grant waits for the owner |
| Prompt injection asks for a credential or a bigger grant | The agent holds no value to hand over; a request for a grant above its capability is refused by code |

## Consequences

- ADR-0015's sentence "read-only is enforced by the code of each tool" holds for T0 only; the invariant now says which enforcement each
  form has (4). ADR-0017's accepted weakening (a chain holds every connection) is replaced by invariant 2. ADR-0004 is narrowed:
  a session starts with an allowlisted environment; with the key file, the database and its copies, the source env files, the vault's
  `secret/`, the client keys, the process manager's config (`ecosystem.config.cjs`) and its state directory (`~/.pm2`, whose dump files and
  daemon socket give the server's environment) hidden; and with the directory of the server's binary, and the checkout around it when it is
  that checkout's `bin/`, read-only (`cmd/remit/agents.go`, `protectedFromAgents`). The server's own checks of an agent's change (`go build`, `vet`, `test`, `git`, `sh -c` in `internal/orch/repo.go`) and the
  sessions take their environment from one allowlist (`internal/childenv`). Gaps that remain, each to be closed by its own unit: a session shares the server's user, so it can signal
  the server (no pid namespace) and can write what that user's cron and shell start outside the jail (the process manager's start
  script, the user crontab, shell startup files), which a list of paths cannot close and a separate OS user or a session whose whole
  root is the jail's does; the GitLab and SSH credentials (`~/.ssh`, `~/.config/glab-cli`) are readable by a session and by a check,
  because sessions run `glab api` and `git fetch` over ssh through library skills (13 and 12 of the 118 skills in the live library
  name them, and sessions have run both), so they move behind the gate with a GitLab connection kind, which does not exist yet.
- A new system is one `Provider`: its `Vars`, its tools, optionally its `Run`. The gate, grants, jail and proxy are not touched.
- Credentials the owner supplies for a T2 connection must be read-only; a connection with a wider credential is offered T0 or T1 only.
- A web page for grants is not built: the owner reads them in `list_sources`, the Tools page and the inbox, and changes them by
  talking to an agent.

## Revisit when

- An incident shows a secret leaving a jail: T2 is switched off for that source until its HTTP form (T1) exists.
- A second connection kind needs a rule more specific than `(source, connection, effect)`: grants become tuples with a relation
  (ReBAC).
- The host provides a stronger sandbox than namespaces (a microVM): the jail's start function is the one place to change.
