# ADR-0135 · Force kill targets one process instance

Status: Accepted

## Context

The owner uses the Session tab to see which OS command is stuck. They need to end a specific command while leaving its session in control of the work. A numeric PID can be recycled while an action menu is open.

## Options

- Signal the displayed PID. A stale selection could kill a replacement.
- Stop the whole session. That remains available for the jail's supervisors and the managed root process.
- Open an OS process handle for each eligible row and use it to signal exactly the shown instance.

## Decision

The runtime pins eligible descendants before sampling their rows and issues an opaque token for each live process instance. The owner API resolves that token in the card's own session and records the authenticated actor, target and reason in Activity before requesting a signal. Shared subagent views direct the owner to the parent session for control.

The runtime rechecks ancestry, liveness and eligibility before sending SIGKILL. The managed root and Remit's supervisor executable belong to the existing Stop action. `cosca::process::Process` signals through a held pidfd on Linux and an audit token with a kernel-checked PID version on macOS.

## Consequences

The operation targets one process. The session and OS handle any resulting exits. A stale or unverified target is refused. If the reply is lost, Activity still records the owner's intent.

macOS requires 14.4 or later. Linux needs `openat2` and pidfds for the action (kernel 5.6 or later). A runtime that cannot prove a process's identity or eligibility leaves that row read-only.

## Revisit when

If operators require a durable signal outcome across link failures, extend the runtime kill reply with a receipt keyed to the recorded owner action. If Linux deployment policy restricts pidfds or `openat2`, report that capability alongside the snapshot.
