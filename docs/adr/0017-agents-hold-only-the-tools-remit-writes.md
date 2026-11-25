# 0017. Agents hold only the tools Remit writes, and a source has named connections

Status: accepted, 2026-10-30; narrowed by ADR-0029: a source may also be a program a chain wrote.

## Context

ADR-0015 gave agents native tools for the outside world (`internal/source`). The Crew kept a second, older concept beside them:
a library of MCP servers imported from the machine's Claude Code configuration. Measured on the live database (2026-10-30):

- The library held three servers (`mongodb-staging`, `mcp-server-starrocks-dev`, `repowise`); `mongo_*` and `starrocks_*` already
  replaced the first two, no agent definition named one, and `mcp_secrets` held nothing.
- The owner could not see the native tools: the "MCP servers" page listed only the library. He could not configure them either:
  their credentials came from environment variables and env files, so setting one meant editing a file on the host instead of
  telling an agent.
- A foreign server is a black box (ADR-0015), and the way to give it a secret (an environment variable of the session's
  process, an imported header) put the value beside the agent.

The owner decided: one concept, native tools; the Crew shows them; credentials are typed into the write-only field of an inbox
item; a source can have any number of named connections.

## Options

1. **Keep both**: the library stays for tools nobody wrote natively. Two concepts, two credential paths, and the rails that guard
   a server's program (it runs as the server's OS user before any tool call).
2. **Native tools only, one connection per source**: the simplest page, but a staging and a production MongoDB, or two Grafanas,
   cannot both be read.
3. **Native tools only, named connections per source** (same code, another set of credentials). **Chosen.**

## Decision

- Agents hold only tools Remit writes. The library of MCP servers, its importer, its sign-in check, its "use the value on this
  machine" answer and the rail that held a server's program are deleted, with the stored data (migration 47). Extending what agents
  can do means writing a source: one file in `internal/source` and one line in `kinds`. An agent that lacks a tool says so with
  `send_feedback`; the owner asks the Chair or the Assistant to write the source.
- **A connection** is a source, a name (`^[a-z][a-z0-9-]{0,31}$`) and a description every agent reads in `list_sources`. `default`
  is the connection the process environment and env files serve, stored values first; a named connection never reads the
  environment, so `prod` cannot fall back to another place's variable. Every tool takes a `connection` argument (the kernel adds
  it once); an empty one resolves to the sole configured connection, and with several the error lists `name: description`. Every
  answer starts with `connection: <source>/<name>`.
- **A source lists every variable it reads** (`Provider.Vars`, with what each is for, secret, optional, address) and keeps its own
  rule of readiness (`Missing`); a test holds every name `Missing` can return to be in `Vars`. The Tools page says *configured*
  (the variables are present), never that a call works.
- **Credentials are typed by the owner** into the item the Assistant or a Chair raises with `request_source_credentials` (names only;
  plain variables first; no duplicate items; a new named connection needs a description). A secret is sealed (`secrets.Seal`, the
  source, connection and variable as associated data) and shown as `set`; a plain variable (an address, a user) is stored as typed
  and shown. The item states where a secret will go. The owner may answer Remove the stored value, or Remove this connection.
  A value that no longer opens under its own key is cleared and asked again; a missing key clears nothing.
- **The Tools page** (Crew tab, `/tools/:sourceId`) lists the sources with their tools and connections, live (`sourceUpdated`).
  It edits nothing: the owner tells the Assistant.
- **Capabilities:** `data`, `observe` and `customers` give a chain the tools of their sources, and every connection of them; `sources`
  (the Chair that gives sources their connections and credentials) adds `request_source_credentials`. `list_sources` goes to every
  agent of a chain holding one of the four, and to the Assistant.

## Threat model (STRIDE over the boundaries this change crosses; OWASP LLM02, LLM06)

| Threat | Control |
|---|---|
| Spoofing: an agent enters or reads a credential | Only the owner's answer stores a value (`SetCredential` refuses every actor but the owner); no agent tool takes or returns a value; the two tools take names only |
| Tampering: a plain variable (an address) is changed so a token goes elsewhere | Only the owner's answer writes a value; the item names the address a secret goes to; the page shows plain values |
| Information disclosure: a secret in the contract, an event, a log, a transcript | `ToolSource` carries no secret value; tool results and errors pass `RedactText`; sealing uses the existing key and its backup rule |
| Information disclosure: the key or the database leaks | Unchanged from ADR-0004's accepted gap: an agent runs as the server's OS user; a read-only credential at the source is the second line |
| Denial of service: the inbox fills | Items only on request, one per connection and variable |
| Spoofing: an agent reads production when it meant staging | **Weaker than before, accepted by the owner (2026-10-30)**: a chain holding a capability holds every connection of its sources. Controls that remain: every answer names its connection, descriptions say what a connection holds, the guards allow only reads, the owner gives a connection a read-only credential, the personal-data masks apply to every connection. A rule that scopes connections adds one field to the connection |
| Elevation: a foreign program runs on the machine | The path that wrote a server's program is deleted |
| Prompt injection from a tool result asking for a credential | The tools stay read only, the result is evidence (ADR-0015), and an agent never holds a value to hand anywhere |

## Consequences

- ADR-0015's first consequence (a foreign server stays importable) no longer holds: no foreign MCP server exists in the code, the
  contract, the pages or the stored data. `repowise` is gone with the library.
- Adding a source is still one file and one line; it appears on the Tools page, is asked for in the inbox and is given to chains by
  capability with no other change.
- Migration 47 drops `mcp_servers` and `mcp_secrets`, creates `source_connections` and `source_values`, removes `mcp` from stored agent
  and card documents, and deletes `mcp` changes and credential items; run twice it ends the same. The sealed-key bookkeeping
  (`SealedKeyIDs`) reads `source_values`.
- `McpServerState` stays: it is what Claude Code reports a session loaded (Remit's own server), not a library record.

## Revisit when

- A tool is needed that Remit cannot write (a browser, a design tool): a new decision on how it is held, with its own rails.
- A rule must scope a connection to a playbook or an agent: one field on the connection, and the check in `Set.Add`'s resolver.
- The accepted weakening (staging and production readable by the same chain) produces a wrong-place reading in a chain's transcript.
