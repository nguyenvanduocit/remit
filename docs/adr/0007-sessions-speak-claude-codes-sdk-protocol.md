# 0007. Sessions speak Claude Code's SDK protocol, through a client of our own

Status: accepted, 2026-10-28. Source: the owner's request of 2026-10-28, the Agent SDK's type definitions
(`@anthropic-ai/claude-agent-sdk` 0.3.309, `sdk.d.ts`), Claude Code 2.1.309 run by hand, and a review of the Go clients
published for it.

## Context

Every agent runs on Claude Code (ADR-0006). Remit reached it through two layers: `claude-agent-acp`, a Node process
that wraps the Agent SDK in the Agent Client Protocol, and `coder/acp-go-sdk` on the Go side. ACP was there to speak to
more than one agent. With one agent left it is a translation both ways: Claude Code's messages became ACP updates, and
what Remit needed that ACP has no word for came back through extensions (`_meta` for options, `_claude/sdkMessage`
for the session init and background tasks, `_session/steering` for messages put into a running turn). The pinned Node
package (`runtime/`) had to be installed beside the binary.

The owner asked to drop ACP and speak to Claude Code as the Agent SDK does, signed in with the machine's own login.

The protocol the SDK speaks is the CLI's own: `claude --input-format stream-json --output-format stream-json
--permission-prompt-tool=stdio`, newline-delimited JSON on stdio, with control requests both ways (initialize, interrupt,
set_model, set_permission_mode from the client; can_use_tool and its withdrawal from the CLI). Run by hand on 2.1.309
with no key or token in the environment, it uses the login of its config dir.

## Options

1. Keep ACP. Rejected: the owner's request, and a Node process and a protocol in between for one agent.
2. An official Agent SDK (TypeScript or Python) in a sidecar process. Rejected: a second runtime to install and a second
   protocol between it and Go, the cost option 1 already had.
3. A published Go client. Reviewed for code quality and how well each handles the protocol, not popularity. The best of
   them still ends a session silently on a line that is not JSON, can hold a control response behind a queue of messages,
   does not pass the tool_use_id of a permission request, ignores a withdrawn request, and gives no access to the raw
   messages Remit reads (task messages, session state). Others drop final messages, have data races, or are
   abandoned. Each would need a fork.
4. A Go client of our own, `internal/claudecode`, that speaks only the protocol and knows nothing of Remit, with
   `internal/agent` turning its messages into Remit's terms.

## Decision

Option 4.

- `internal/claudecode` starts one `claude` process (`-agent`, default `claude` on the PATH) and speaks the protocol: it
  reads lines of any size, skips and logs a line that is not JSON, answers control requests on the reading goroutine so
  a slow handler never holds one up, gives a permission request a context that a withdrawal ends, and reports why a
  process ended before it initialized.
- `internal/agent` is the one place that reads Claude Code's messages. It hands the orchestrator `agent.Update`s (text,
  thinking, tool calls with their kind and status, the plan, usage), permission requests as the tool call they are for,
  the session init and background tasks. A turn ends at the result that names the last message sent into it
  (`user_message_uuids`), so a message steered into a running turn (priority `now`) is answered by the same turn.
- Sessions are created with an id of their own (`--session-id`) and resumed with `--resume`; one Claude Code no longer
  has is replaced by a new session given the card's brief again (`ContextLost`), as before.
- `runtime/` and `coder/acp-go-sdk` are removed.

## Consequences

- One process per agent, no Node runtime to install: the machine needs the `claude` CLI, signed in.
- Remit follows the CLI's protocol, which the SDK's type definitions describe but Anthropic does not publish as a
  stable contract. A Claude Code update can change it; the fake CLI in the tests is checked against the real one by hand.
- Permission requests are allowed or refused; ACP's other options (always allow) were never offered to the owner.

Enforced by: TestASlowHandlerDoesNotHoldUpAControlResponse, TestAWithdrawnPermissionRequestEndsItsContext, TestASteeredMessageJoinsTheRunningTurn, TestAMissingSessionIsReplacedAndSaysSo, TestSessionsStartOnClaudeCodeInTheAgentsConfigDir

## Revisit when

- Anthropic publishes a Go Agent SDK, or documents the protocol as a stable contract.
- A Claude Code release changes the protocol in a way the client cannot follow with a small change.
