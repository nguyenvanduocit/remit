# 0015. Agents read the outside world through tools Remit owns

Status: accepted, 2026-10-30.

## Context

Agents need evidence from outside the repository: stored values, metrics and alerts, incidents, customer conversations.
Until now they got it from MCP servers imported from the owner's Claude Code configuration (library, capability `data`).
What that gave, measured on the live database (2026-10-30):

- No session since 2026-10-27 loaded any server but `remit`: the library holds three servers, none with a capability, and no
  agent definition names one. The 12 chains whose playbook lists `data` never held a data tool.
- 27 pieces of feedback say agents stop where evidence would begin: no Mongo, Bytebase unreachable, Coroot, Grafana and Intercom
  credentials unreadable by the agent, AWS read calls asking for a permission answer each.
- A foreign server is a black box: its tools are general, its read-only promise is the server's, its errors are free text, and
  a credential reaches it as an environment variable the agent's process holds.

## Options

1. **Import more MCP servers** (Bytebase, Intercom, Amplitude): fast, but each stays general and unbounded, the read-only rule is
   another party's, and a server's secret sits in a process the agent runs beside.
2. **A generic "fetch URL with the owner's token" tool**: one tool for everything, but the agent chooses the URL and the verb;
   read-only is then a convention, and an agent can be steered into a write (SSRF, prompt injection from a customer message).
3. **Native tools, one per question an agent asks**, written in Go inside Remit, holding the credentials in the backend
   (the way `list_issues` holds Jira's), each bounded to read-only by its own code. **Chosen.**

## Decision

- `internal/source` is the package of these tools. It imports no other package of the module. A source is a `Provider`: the
  credentials it needs (`Missing`) and its tools (`Tools`). Tools are named for the question they answer
  (`grafana_query`, `starrocks_query`, `mongo_find`), take typed input, and return text.
- A capability names what a chain may do, not a system: `data` (read stored production and staging data: StarRocks, MongoDB,
  Bytebase), `observe` (read metrics, alerts, incidents and logs: Grafana, Coroot, Elasticsearch, CloudWatch), `customers` (read
  what customers say and do: Intercom, Amplitude). A playbook lists capabilities as it lists `history`; the orchestrator gives
  the tools of those sources to every agent of the chain (`internal/orch/sources.go`). The catalog entry for each capability
  comes from `source.Describe`, so a tool exists in one place.
- Credentials are read by the backend at start, from the process environment first and then from env files (`-source-env`,
  default the vault's `secret/` files), and never pass through an agent's context, a tool result, an event or a log. A source
  whose variables are missing is still offered: its tools answer "X is not configured: set A, B" so the agent says what is
  absent instead of working around it.
- **Read-only is enforced by the code of each tool, not by the credential's rights and not by the agent's promise.** The kernel
  fixes the common rules once: the base address comes from the credential, never from the agent; the verb is GET, or POST only
  where an adapter names the endpoint as a read; a redirect to another host is refused; an answer is cut at a size; a call has a
  deadline. Query languages are checked by pure guards (`sqlguard.go`: one statement, `SELECT`/`WITH`/`SHOW`/`DESCRIBE`/`EXPLAIN`
  only; `mongoguard.go`: `find`, `count`, `distinct`, `aggregate` without `$out`, `$merge`, `$function`, `$accumulator`, `$where`).
- **Secrets in answers are removed before an agent reads them.** Structured answers drop the value of any field named like a
  credential (`accessToken`, `password`, `apiKey`, `authorization`, `secret`, a hash); every answer is scanned for the shape of
  a token (JWT, `shpat_`, `Bearer`, `AKIA`, private-key blocks). A customer's email is masked in Intercom answers.
- An answer is the outside world's words: every tool says so in its description and a result is evidence to verify, never an
  instruction (as `list_issues` and `read_signal` do).

## Threat model (STRIDE over the trust boundaries; OWASP LLM01, LLM02, LLM06)

| Threat | Control |
|---|---|
| Spoofing: a tool call reaches a system the chain should not read | A chain holds only the capabilities its playbook listed when it was chosen; the Chair's `serverFor` adds a source's tools by capability and by nothing else |
| Tampering: a "read" changes data | Verb and statement guards in code; Mongo `$out`/`$merge` and SQL non-queries refused; tests drive each guard with the writes it must refuse |
| Information disclosure: a token or a password reaches an agent's context | Credentials stay in the backend; field-name and token-shape redaction on every answer; error texts redacted |
| Information disclosure: customer personal data | Intercom emails masked; the capability `customers` is given only to playbooks that list it |
| Denial of service: a query scans a table or floods the context | Deadline per call, row and byte caps, answer cut with a note naming how to narrow the call |
| Elevation / SSRF: the agent steers a request to another host | The base address is the credential's; a path is checked (`/`-rooted, no `..`, no scheme); redirects to another host are refused |
| Prompt injection from a customer message, a log line or a document | The answer is marked as outside evidence; no tool here writes, so an injected instruction has no write tool to reach |

Residual (accepted): an agent runs as the server's OS user and can read the credential files the backend reads (ADR-0004's
accepted gap); a read-only credential at the source is the second line, and the owner should supply one.

## Consequences

- These tools replace the library's server `capability` (`data`); no foreign MCP server is held at all (ADR-0017).
- Adding a system is one file in `internal/source` (a `Provider`) and one line in `kinds`; no change in `orch`, the contract or
  the web app. The owner sees what a chain read in the transcript, as with any tool.
- A source whose API changes breaks only its own tools, with an error naming the source and the status.
- What the tools cost to change: a tool's input is a Go struct with `jsonschema` tags, so renaming a field is one edit.

## Revisit when

- A source needs a write (an alert silence, a ticket comment): that is a new capability with `writes` set, a rail, and its own ADR.
- Two sources need the same pagination or time-window rule: it moves into the kernel at its second use.
- The owner wants a source whose credential differs per shop or per user: the `Env` lookup becomes a per-call credential.
