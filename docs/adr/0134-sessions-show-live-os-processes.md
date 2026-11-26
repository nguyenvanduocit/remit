# ADR-0134 · Sessions show live OS processes

Status: Accepted

## Context

When a card says it is running a command, the owner needs to see what is running and where the host is spending CPU and memory. Claude Code's tool telemetry records invocations and durations. The OS owns the current process state.

## Options

- Ask the orchestrator's host for descendants. This measures the wrong host when the session runs elsewhere.
- Read the process tree where the session runs. The runtime can use the PID held by its Claude Code connection to scope the read.

## Decision

The owner's read-only `card_processes` call resolves the card's live session and asks its runtime for a snapshot. A subagent reports its parent's tree with a shared-session label. The runtime returns the executable name, PID, tree depth, OS state, CPU as a percentage of one logical core, resident bytes and elapsed time. It samples at most once per second per session, carries CPU only after two samples of a process, and discards a snapshot if its root begins to be reaped during the read.

The Session tab polls while visible. It holds only the displayed snapshot. The call is available through the signed-in owner API.

## Consequences

A snapshot includes live descendants of the managed root, including jail helpers. Short-lived commands between samples and processes reparented outside the tree are outside this view. Executable names provide command identity while arguments and environment stay on the host.

## Revisit when

If the owner needs shell command lines, correlate the session's existing tool events with OS processes and define a safe display policy at the runtime read boundary before returning any arguments. If reparented processes need to stay visible, extend that boundary to query the jail's membership on each platform.
