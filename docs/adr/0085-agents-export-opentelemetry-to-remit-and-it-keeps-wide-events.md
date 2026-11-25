# 0085. Agents export OpenTelemetry to Remit, and it keeps wide events; a context is Claude Code's own count

Status: accepted, 2026-11-12. Extends ADR-0084 (what Claude Code does to a session) and ADR-0058 (sessions run in the AI
runtime).

## Context

The owner judges agents by what they cost, how fast and how reliably they work, and what they spend their context on.
Remit saw little of it:

- **The context panel guessed.** `orch/src/transcript.rs` parsed Claude Code's undocumented transcript records and counted
  characters / 4. On the Assistant's session it showed 55k where the model's own usage said 87k, and it missed the hooks'
  added context. Claude Code 2.1.322 answers a control request, `get_context_usage`, with the count its `/context` shows:
  each part's tokens (system prompt, tools, deferred tools, MCP, agents, memory files, skills, messages), the memory files,
  skills, agents and MCP tools in it, and the conversation's tool calls by tool. Its parts add up to the last request's
  real input tokens (checked on a live session: 18,324 both).
- **The owner's own instructions reached every agent.** Claude Code reads every `CLAUDE.md` from its working directory up
  to `/`; a session under the server user's home read `~/.claude/CLAUDE.md` as the project's (7,206 tokens on the
  Assistant), although its settings layers are `project` only.
- **What a session does over time was not recorded**: each model request's tokens, cost, latency and time to first token,
  each tool run's duration and outcome, API errors and retries. Claude Code exports all of it as OpenTelemetry (logs
  `claude_code.api_request`, `tool_result`, `tool_decision`, `api_error`, ...; metrics `claude_code.token.usage`,
  `cost.usage`, ...; with `CLAUDE_CODE_ENHANCED_TELEMETRY_BETA`, spans `claude_code.interaction` → `llm_request`, `tool`
  → `tool.execution`). The stream carries the agent's words and calls, not these measures.

## Options

1. **Remit is the collector (chosen).** The server takes OTLP/HTTP JSON at `/otlp/v1/{logs,metrics,traces}` beside
   `/mcp`, with the card's own token, and keeps every record as one wide event in a database of its own.
2. **An OpenTelemetry Collector beside Remit**, exporting to a store Remit queries. A second process and config to ship
   and run on every install, and a record that names no card unless each session is told who it is and trusted to say so.
3. **The transcript, parsed better.** Still an undocumented format, still a guess at tokens, and no latency or cost.

## Decision

- **A context is Claude Code's count.** Each time a session goes to rest the AI runtime asks `get_context_usage` and the
  core keeps the newest per card (`session_contexts`, migration 0102); the Session panel shows it, asking the running
  session afresh when there is one. The transcript estimator is deleted. The card's meter (`ctx_used` of `ctx_size`) is
  the same count against the window Claude Code compacts in (`maxTokens`, 400k for a session that compacts at 367k),
  not the model's largest window a turn's `modelUsage` names (1M).
- **What Remit gave a session is recorded as it starts it**, never rebuilt from today's settings: each start of a
  session's process stores the spec it sent (skills, tools, settings layers, allowed and deny rules, Remit's tools,
  capabilities, principles) per card (`session_launches`, migration 0103), and a new session's first message is added as
  it is sent. A part with nothing recorded shows as no data. The header's model, effort and mode are what Remit asked
  for; what ran is what Claude Code reported (its `init`, and each request's `model` and `effort` in telemetry).
- **A session reads its repository's instructions only.** Every session starts with `claudeMdExcludes` naming the
  instruction files of every directory above its repository (the nearest directory up from its `cwd` holding `.git`, or
  `cwd` itself).
- **Every session exports telemetry to Remit.** Its process starts with Claude Code's telemetry on (logs, metrics and
  spans, `http/json`) pointed at the server's `/otlp` with the card's bearer token, the same token `/mcp` takes. Tool
  details are exported; prompt and response text is not (the stream holds it).
- **A record is a wide event.** One row per log record, metric data point or span: when, the card and home from the token
  (never from the payload), the session, the kind, the name, a number (a metric's value, a span's duration), the trace
  and span ids, and every attribute (resource, scope, record, span status and events) flattened into one JSON object.
  The account's identity attributes (`user.*`, `organization.id`) are dropped: the card names its account. This is the
  OpenTelemetry data model with no schema of Remit's over it, so a new attribute or event needs no change, and an agent
  reads it with SQL.
- **It lives in `telemetry.db`**, beside the store's database, kept 90 days. The pure `telemetry` crate decodes OTLP and
  sums a card's events; `store::Telemetry` opens, writes and reads the file, as only the store opens a database. Its volume grows with every request and tool run; it stays out of the owner's database, its snapshots and the
  backups before a migration.
- **The Session panel shows a card's telemetry**: requests, tokens, cost, latency (p50, p95, time to first token), errors
  and each tool's runs, failures and time.

## Consequences

- The context panel's numbers are Claude Code's, exact, for every session that has rested once.
- An agent's cost, speed and failures are measured per request and per tool, which the coach, the analyst and duels can
  read next.
- A token leaked from a session can post telemetry to its own card, as it can call its tools.
- `telemetry.db` is not in a snapshot or an export; losing it loses measures, not work.

## Revisit when

- **Agents read telemetry.** An MCP tool (the coach's, the analyst's) that queries it, with its own capability.
- **Volume outgrows SQLite.** The wide-event shape moves as is to a columnar store.
- **Other systems export to Remit.** The door takes a client key's records into the same table.
