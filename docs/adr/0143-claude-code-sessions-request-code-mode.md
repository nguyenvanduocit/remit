# 0143. Claude Code sessions request Code Mode

Status: accepted.

## Context

Claude Code can run several tool calls from one JavaScript program. Its headless CLI in 2.1.332 takes the opt-in from the explicit `--settings` overlay and recognizes the `sdk-cli` entrypoint for stream JSON clients.

## Options

1. Leave opt-in to each repository's Claude Code settings. The headless CLI requires the launcher's explicit opt-in.
2. Have Remit request Code Mode on every Claude Code session. Claude Code decides availability and reports refusals on the session stream.

## Decision

Use option 2. The Assistant and card agents request Code Mode at process start through their shared Claude Code launcher. In a supporting session, the agent can compose tools using Claude Code's JavaScript environment. Each dispatched tool still uses Claude Code's existing hooks and permission handling, including the fail-closed hooks Remit verifies during initialization.

When Claude Code refuses the request, it starts with ordinary tools and Remit shows the refusal in the session's activity. Builds predating the opt-in continue using their existing tool path.

Claude Code preserves the tool mode of an existing conversation on resume. The launcher requests Code Mode for new conversations and surfaces Claude Code's notice when a resumed conversation uses ordinary tools.

Code Mode and Claude Code's enhanced tracing beta are mutually exclusive in 2.1.332. The launcher keeps the standard OpenTelemetry export from ADR-0085 and leaves the enhanced tracing beta off.

## Revisit when

- Claude Code stabilizes headless Code Mode. Replace the experimental setting with its supported interface.
- Claude Code exports enhanced tracing from Code Mode sessions. Re-enable the enhanced tracing export.
- A runtime reports that a composed call can escape a hook. Require Code Mode to affirm per-call enforcement at the existing initialization check before enabling it.
