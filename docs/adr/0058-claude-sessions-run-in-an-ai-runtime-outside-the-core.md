# 0058. Claude sessions run in an AI runtime outside the core

Status: accepted, 2026-11-04; its Socket.IO transport superseded by ADR-0072 at its cutover. Changes ADR-0040 (the accounts' logins live in the runtime, which signs them in and out for the core)
and ADR-0004 (the runtime runs as the server's OS user with the same HOME, and the Claude config dirs are its). Source: the owner's
words of 2026-11-04, "Move the AI employee part out of the core, make it something managed on its own, a peer of the human
employee", and the spec `docs/superpowers/specs/2026-11-04-employee-runtime-design.md` (*Intent*, *Why*, *Rails*, *Lifetimes*).

## Context

The core ran every Claude Code process itself. `agent.Launcher` (`internal/agent/agent.go`) was already its one session port: the
orchestrator starts, prompts, steers, cancels and closes sessions through it and nothing else, and `agent.ClaudeLauncher`
(`internal/agent/claude.go`) implemented it in the core's own process. The spec's *Why*: one session path for every employee, so a
person carrying on an AI's work is a new card on the same path, not a second system; and every session, whoever drives it, is
recorded on our side. A session that lives in the core also ties its life to the core's: a crash of one Claude Code process tree
shares the core's process, and the core cannot kill a session that will not die without reaching into its own children.

## Options

1. **Sessions stay in the core.** Nothing moves; a person's session would need a second path beside it, and a stop that a session
   ignores has nothing to kill but the session's own group.
2. **A runtime per session.** Each session is a process of its own that dials the core. Isolation per session, but every session
   pays a process, a connection and a login of its own, and the accounts' logins would be spread over them.
3. **One runtime, the core's child, behind `agent.Launcher` (chosen).** The core starts `remit runtime` (the same binary) and
   every session opens there over one `/runtime` namespace; `runtimelink.Launcher` implements the port the orchestrator already
   uses, so the orchestrator changes only where the runtime's death is new to it.

## Decision

- **The runtime is the core's child.** `run` mints a key (`runtimelink.NewKey`) once per core start and starts `remit runtime`
  under `runtimelink.Launcher.Supervise`, its settings (the key among them) as one JSON line on its stdin, never in an argument or
  the environment. Stdin stays open as the core's lifeline: when it closes the runtime closes its sessions and exits, so a core that
  dies, even by SIGKILL, takes its runtime with it. The first runtime the core starts decides whether it can run agents: one that
  exits before its hello fails the core's start with its last words. Once a runtime has said hello, every later one that exits
  (before its own hello or after) is started again after a backoff (1 s, doubling to 30 s, reset after a minute up) and counted
  in `/healthz` (`runtime.restarts`).
- **`/runtime`.** The runtime dials the core back on `/runtime` (`runtimelink.Mount`), presenting the key, compared in constant
  time; a runtime of another contract number is refused. The contract is `web/src/rpc/runtime/define.ts`, both ends generated.
  Every wait is two calls, and no call waits on Claude: an open's end is an `opened` entry, a turn's end a `turnEnded` entry, a
  kill's end the `exit` entry, a permission's answer `permissionAnswered`, a sign-in's end `accountLoginEnded`. The generated
  transport runs one call of each method at a time per connection, so a call that waited on Claude would hold every other
  session's call of that method. Every call carries the id its caller minted, so a replay acts once; a session's entries are
  numbered, sent again after a dropped socket from what the core acked, and handed to the session's handler on a goroutine of
  its own, never on the call that brought them.
- **What lives where.** The core keeps account choice, failover, the breaker and every rail. The runtime holds the accounts' config
  dirs and logins (`agent.ClaudeAccounts`), signs accounts in and out for the core (`runtimelink.Accounts`), and serves the
  transcripts Claude Code writes (`runtimelink.Transcripts`, `airuntime.TranscriptDir`). `cmd/remit/runtime.go` is the one
  place a `ClaudeLauncher` or `ClaudeAccounts` is built.
- **Stop.** `endRun` deletes the run's MCP token first, cuts the turn, then `Close`s the session: `sessionKill` in the runtime, which
  kills the session's process group, and `Close` waits for its `exit` entry. A runtime whose exit does not arrive within `KillWait`
  is killed whole by the supervisor: it is
  stopped, every process under it (read from `ps -A -o pid=,ppid=`) is killed with its group, deepest first, then the runtime, and a
  new one is started. A stop never waits on the runtime's cooperation.
- **A runtime that dies under a live core.** Every session it held ends with an error wrapping `agent.ErrHostRestarted`. The
  orchestrator's `hostRestarted` stops the card as a server restart does ("the AI runtime restarted"), and a turn it cut or
  background work it lost resumes at once with `--resume`, by the step a server restart takes (`resumeAfterRestart`, parked after
  `maxResumes` in a row); an idle card resumes when its next message reaches it.

## Consequences

- The core starts no Claude Code process; `/healthz` is unhealthy while no runtime is connected.
- One more process on the machine, and one more hop for every event of a session (loopback).
- A runtime killed for one hung session restarts every session it held; they resume, at the cost of their turns in flight.
- A core restart restarts the runtime; turns resume as after a server restart.

## Revisit when

- **A runtime on another machine.** The code then reaches it by git from the core and skills are fetched by content hash; today
  both read the same disk.
- **Per-session sandboxes.** The seam is `runtimelink.Process`: a supervisor that starts one sandboxed runtime per session, or a
  runtime that starts each session in one, implements it.
- **Mutual authentication of core and runtime.** The seam is `Mount`'s key check. Today `/runtime`'s key is only a shared secret on
  loopback: there is no TLS, and the runtime does not authenticate the core. No limit bounds the sessions one runtime holds.
