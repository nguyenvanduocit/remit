# 0077. An outside MCP server is an extension: Remit is its client, and every call passes the gate

Status: accepted, 2026-11-11; its waits on the owner, but a secret act, are an agent's (ADR-0087). Extends ADR-0034 (an extension is a source somebody else wrote) and refines ADR-0018's sentence
"a foreign MCP server is no separate concept and is not trusted more than a script". Source: the owner's goal of 2026-11-11,
"absorb the best of what we learned from Codex", and `docs/research-codex/notes/tools-skills-plugins.md` (idea 3).

## Context

The customer is the owner, who asks for a system he uses to be reachable by his agents. Most vendors now ship an MCP server for
their product; Remit reaches none of them. A session holds exactly one MCP server, Remit's own (`crates/orch/src/access.rs:118-131`),
and the Assistant's manual has to answer "Bring in my MCP servers" with "has no answer in the library"
(`crates/builtin/skills/app/references/playbooks-agents-library.md:19`). Each outside system costs a plugin written for it
(ADR-0034, ADR-0037).

Codex shows what a full MCP client is: stdio and HTTP transports, OAuth sign-in, a per-server allow and deny list of tools
(`references/codex/codex-rs/config/src/types.rs:1212-1216`) and a per-tool approval policy (`config/src/mcp_types.rs:29-35`,
`AppToolApproval { Auto, Prompt, Writes, Approve }`). HIGH. Codex gives those servers to the agent directly; Remit's law does not
allow that: an agent never holds a credential, and every call to an outside system passes one gate that decides on a grant,
records the call, redacts and cuts the answer (ADR-0018, invariants 1-4; `crates/grant/src/lib.rs:1-5`).

What Remit has to build on (HIGH, read):

- An extension is a source whose tools are the ones it described when the owner was shown it, called through the gate, served as
  `<extension>_<tool>` (`crates/source/src/extension.rs:1-3,678`), proposed by a Chair with `propose_extension` and applied by the
  owner from a change item that shows everything it is (`crates/orch/src/extensions.rs:1-9`).
- A source's credential variables are sealed when `secret` (`crates/source/src/source.rs:111-126`); a missing one is asked for in a
  `credential` inbox item with a write-only field (`crates/model/src/inbox.rs:15`).
- A program runs in a jail with the connection's credential in its environment and egress to its addresses only, form T2
  (`crates/source/src/program.rs:1-9`).
- An effect is ranked read, reversible, compensable, irreversible by the platform, never by the tool's author
  (`crates/grant/src/lib.rs:13-25`); a grant above read waits for the owner (ADR-0018).
- `rmcp` 3.9.0 serves Remit's own MCP tools (`crates/orch/Cargo.toml:35`, `crates/clientmcp/Cargo.toml:14`); its client features are
  used today in tests only (`crates/clientmcp/Cargo.toml:20`, `crates/orch/Cargo.toml:65`, dev-dependencies). The crate also offers
  `transport-child-process` and `auth` (its `Cargo.toml`, features at lines 141 and 67).

## Options

1. **Hand the session the server** (`Spec::mcp` gains the outside server; Claude Code connects to it). Least code. The
   credential sits in the session's MCP config, which the agent's own shell can read; no grant decides, nothing is recorded, a write
   is as easy as a read. It breaks invariants 1-3 of ADR-0018.
2. **A bundled `mcp` plugin per server** (the research proposal). It reuses the plugin path, but a plugin's tools are declared in
   its static manifest (`wire::plugin::Manifest::tools`, `crates/wire/src/plugin.rs:155-171`) while a server's tools are known only
   after Remit connects with the credential; and it puts a plugin process between Remit and a protocol Remit can speak itself.
3. **One plugin written per vendor** (today). Each is reviewed code with typed effects; each costs a change to Remit, and the
   vendors' own servers stay out of reach.
4. **An MCP server is an extension source of its own form: Remit is the MCP client, the server's tools are pinned and served
   through the gate like an extension's.** One concept (an extension, a source somebody else wrote), one more form of reaching it.

## Decision

Option 4. Patterns: a gateway (Remit is the one MCP client and re-serves what it trusts), the anti-corruption layer (a server's
tool becomes a Remit tool with an effect Remit ranks), zero trust (the server is outside code whatever it claims). Mental models:
inversion (what would guarantee a leak: a credential or an unpinned description reaching an agent), second-order thinking (a
server that changes its tools after the owner approved it).

### Where it lives

- `crates/source` gains the form `mcp`: a source whose program speaks MCP instead of the plugin protocol. Its record is an
  extension's (`extensions`, change-managed) with `form: mcp`, the transport, and for stdio the command line and its arguments, or
  for HTTP the variable that holds the URL. The core speaks MCP through `rmcp`'s client; no plugin process sits between.
- **stdio**: the command runs as a program in the jail (T2): the connection's sealed variables in its environment, egress to the
  connection's address variables only, the server's own environment never passed. One process per connection, started at its first
  call and again after it dies, as `plugin::RemotePlugin` does.
- **HTTP** (streamable): the backend connects, with the connection's secret sent in the header the record names. The credential is in
  the executor only, as for a typed tool (T0).
- A session's MCP servers do not change: Remit's own, and nothing else. A session reaches an outside server's tools
  through Remit.

### Tools, effects and drift

- On the first connect of a connection, Remit lists the server's tools and pins each: name, description, the hash of its input
  schema, and its effect. The pin is an event on the source, and `list_sources` shows it.
- **Effect**: a tool whose `readOnlyHint` is true is `read`; every other is `irreversible`. A hint is the author's claim, so it can
  only lower a tool to read; the owner (or the Assistant for him) may rank a tool lower than irreversible in the change item, which
  is the platform ranking it. A grant above read waits for the owner, as for every source.
- Tools are served as `<source>_<tool>`, each call through the gate: grant, deadline, record, redaction, cut.
- **Drift**: when a later listing adds a tool, removes one, or changes a pinned tool's description or schema, the changed and new
  tools are not served; a change item shows the difference, and the Assistant may apply it (no secret is in it). A removed tool is no
  longer served.
- Only tools. A server's resources, prompts, sampling and elicitation requests are refused: Remit declares none of those client
  capabilities.

### How the owner asks for it

- "Bring in my MCP servers" goes to the Assistant. `list_importable_mcp_servers` reads, on the server, the MCP servers configured for
  the machine's Claude Code, and answers each one's name, transport, command or URL, and the names of its environment
  variables and headers, never a value. `propose_mcp_server` (also open to a Chair whose playbook lists `extensions`, beside
  `propose_extension`) raises one change item per server: what it runs or reaches, its variables, and which of them are secret.
- On the owner's Apply the source exists. A secret value found in the machine's configuration is copied by the server into the
  connection's sealed variables, never through an agent; a secret not found is a `credential` item with a write-only field, the one
  value the owner types. A Chair then asks for grants on the connection as for any source.
- `remove_extension` removes it.

### Built in two slices

1. The `mcp` form over stdio and over HTTP with a header credential; pinning, effects and drift; the import and propose tools; the
   manual's answer to "Bring in my MCP servers".
2. OAuth sign-in for the HTTP servers that require it: Remit is the OAuth client (`rmcp`'s `auth`), the owner signs in from a `login`
   inbox item as for an account (ADR-0040), and the tokens are sealed in the connection and renewed by the executor.

## Threats

An outside MCP server is code and text Remit did not write, reached by every agent that holds a grant on it.

| Threat | Control |
|---|---|
| A tool description carries instructions to the agent (tool poisoning) | Descriptions are pinned at first connect and shown by `list_sources`; agents read tool text as data, as they read source answers |
| The server changes a tool after it was trusted (rug pull) | Drift: a changed or new tool is not served until a change item is applied |
| A tool claims read-only and writes | The hint can only lower to read; the owner's read-only credential is the enforcement of record, and the connection says so (`enforced: credential`, as a program's tool does at `crates/source/src/connection.rs:500`; ADR-0018 invariant 4) |
| The credential reaches an agent | stdio: only the jail's environment; HTTP: only the backend's request; never in a session's config, an event or a transcript; answers are redacted |
| A stdio server reaches elsewhere or reads the host | The jail: egress to the connection's addresses, no server environment, its own work directory |
| The server asks the client for a model call or for input (sampling, elicitation) | Refused: Remit declares neither capability |

## Consequences

- Any system with an MCP server is reachable without a change to Remit, under the same grants, records and refusals as a typed
  tool.
- A server's tools are trusted no further than their pin and their effect; a server that renames its tools often costs the
  Assistant a change item each time.
- A stdio server runs on the server's machine in a jail: one more process per connection in use.
- The extension concept now has two program forms (the plugin protocol and MCP) and the injecting proxy form: one record, one
  gate, one owner's Apply.

## Tests (written first, main path first)

- source: a fake stdio MCP server's tool is called through the gate with a grant and refused without one; its secret is in the
  jail's environment and not in the answer; a tool without `readOnlyHint` needs an irreversible grant.
- source: a listing whose tool's description changed serves the other tools and not that one, and raises a change item.
- orch: `list_importable_mcp_servers` answers names and variable names and no value; Apply copies a found secret into the sealed
  variables and asks for a missing one in a credential item.

## Revisit when

- A per-tool approval policy finer than the effect (Codex's `Prompt`, `Writes`, `Approve`): a field on the pin.
- A server's `notifications/tools/list_changed` should be read live instead of at each connect.
- Pinning a stdio server's package by hash (an `npx` or `uvx` command names a version, not a file).
- Resources or prompts are asked for.
- Security hardening (a session user per server, OAuth dynamic client registration rules, a review of what a description may say):
  threat model when asked.
