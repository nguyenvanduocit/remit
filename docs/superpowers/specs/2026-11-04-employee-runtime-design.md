# Employee runtime: every Claude session runs outside the core, and a person drives one as an AI does

Status: design, approved by the owner 2026-11-04. Builds on `docs/superpowers/specs/2026-11-03-human-employees-design.md`
and replaces the parts of `docs/superpowers/specs/2026-11-03-paseo-bridge-design.md` named in *What this replaces*.

## Intent

The owner's words (2026-11-04), in order:

- "Move the AI employee part out of the core, make it something managed on its own, a peer of the human employee."
- "The system's roles stay separate, like the other employees."
- "A person can go on talking with their AI, we do not care much: only their AI represents them, and only when that AI submits
  or talks to us is it official. The transcript is still collected, but it has no effect in the chain and on other employees."
- "If we want a human to carry on an AI employee's work, their Paseo has to work remotely, directly in the folder we set up for
  the AI": a Paseo provider that talks over the network to the Claude Code we spawn on the server.
- "Two cases, the Chair decides: if the AI employee is doing too poorly or is stuck, hand off to a human; if a human is needed
  from the start, have the human do it. A human is not always an engineer: sometimes we hire a human only for a decision, one
  question; they click their task, answer, done. Anything more goes through ask."
- "The user's own setup is not needed: everything runs on our runtime. That is how we get the data, the transcripts, everything
  we need to improve our AI employees."
- Interaction uses `socketrpc-gen` because it handles both directions; the Go client it lacks is being added in
  `~/projects/archive/socketrpc-gen` (owner's package), on a copy of the zishang520 Socket.IO library the project now owns.

Standing decisions this keeps: Claude Code is the only agent, through our own stdio client (ADR-0006, ADR-0007). It runs on a
personal computer; Kubernetes is never required. Remit does not depend on Paseo: no server code names it.

Why: one session path for every employee, so a person carrying on an AI's work is a new card on the same path, not a second
system; and every session, whoever drives it, is recorded on our side, which is the data that improves the AI employees.

## Goals

1. **The core runs no Claude session.** Every session (hired workers, the Chair, the Assistant, intake, the goal judge, the
   account health probe) runs in the **AI runtime**, a separate process the core talks to over `/runtime`.
2. **A session is driven by its card's employee and its lead.** The lead's messages reach every session through the core's
   message queue, as today; a person's session is also prompted by that person from Paseo. A card's kind comes from its employee, as today
   (`internal/orch/person.go:38-45`); a card never changes employee.
3. **Three shapes of work for a person, chosen by the Chair:**
   - **Decision**: a card holding one question. The person opens it in Paseo and answers; the answer is the card's report.
     No session, no code, no Claude.
   - **Work**: the Chair hires a person for a job; the runtime opens a session in the card's worktree that the person drives.
   - **Takeover**: the Chair hands an AI employee's work to a person (the AI keeps falling short, or is stuck). The AI's card
     ends as handed over; the person gets a new card in the same worktree, joined to it by a `handoff` edge, whose session
     starts from a handover brief written for a person. **Hand back** is the same move toward an AI employee.
4. **Only what the session hands in is official.** In a person's session, the person's prompts are theirs; the chain sees only
   what the session does through its card's tools (`hand_in`, `ask_lead`, the plan, the capabilities the hire holds). In a
   decision card, the person's answer in Paseo is the act.
5. **Everything is recorded.** Every session's timeline is stored as card events with its employee, model and account, people's
   sessions included, and every handover is a Decision carrying the Chair's reason.
6. **Generated clients on both sides.** `/runtime` and `/worker` are `socketrpc-gen` contracts; the AI runtime uses the
   generated Go client, the Paseo plugin the generated TypeScript client. No hand-written copy of a contract type.
7. **Local first.** On one machine the core starts the runtime itself; worktrees, transcripts and skills are on the same disk.

## Non-goals

- Agents other than Claude Code; ACP; A2A.
- A person's own machine doing Remit work: no local Claude, no code, skills or secrets synced to a laptop, no transcript
  uploaded from one.
- Kubernetes-specific operation. A pod runs the same two processes the laptop runs.
- More than one runtime, or a runtime on another machine (seams below).
- A person in the Chair's seat.
- Security hardening beyond keys and the existing jail (seams named under *Revisit when* of the ADRs).

## Anti-goals

- A person's conversation with their session changing the chain, a lead's view, or another employee's work.
- A stop that needs the runtime's cooperation to take effect.
- Two session paths (some sessions in the core, some in the runtime).
- A person seeing the product, chain, Chair, agents or other employees: their session's brief, tools and the messages they get
  name only their own work and *the lead* (P1's rule, `docs/superpowers/plans/2026-11-03-human-employees.md:19`).

## The shape

```
            ┌──────────────────────────── core (remit) ─────────────────────────────┐
            │ chains · cards · queue · decisions · gates · memory · events · policy      │
            │ accounts: choice, failover, outage breaker · git (worktrees, pushes)       │
            │ MCP tools at /mcp · rails · auth                                            │
            │ RuntimeLauncher (agent.Launcher over /runtime) · PersonLauncher (decisions) │
            └───────────▲───────────────────────────────────────────────▲───────────────┘
                 /runtime (socketrpc-gen, Go client)          /worker (socketrpc-gen, TS client)
            ┌───────────┴─────────────────────┐          ┌───────────────┴──────────────────┐
            │ AI runtime (remit runtime) │          │ Paseo plugin on a person's host  │
            │ Claude Code sessions, jail mask │          │ tasks, decisions, ask; provider  │
            │ account logins (config dirs)    │          │ that drives hosted sessions      │
            └─────────────────────────────────┘          └──────────────────────────────────┘
```

### The core keeps the session port it has

`agent.Launcher.Start(ctx, Spec, Handler) (Session, error)` (`internal/agent/agent.go:122-124`) stays the one port. The core
gets a `RuntimeLauncher` that implements it over `/runtime`: `Start` opens a session in the runtime, `Session.Prompt / Steer /
Cancel / Close` become calls, and the runtime's events arrive as the `Handler`'s `OnUpdate`, `OnPermission`, `OnInit`, `OnTask`,
`OnWorking`, `OnExit` (`agent.go:17-34`). `Orchestrator.launch` (`internal/orch/session.go:288-319`), intake
(`intake.go:353`) and the health probe (`health.go:369`) keep calling `o.launcher.Start`; what changes is which launcher the
binary wires. `ClaudeLauncher` (`internal/agent/claude.go:26`) and `internal/claudecode` move to the runtime side unchanged.
`PersonLauncher` (`internal/agent/person.go:42`) remains for decision cards only.

### Accounts: the core decides, the runtime holds the logins

Choosing a card's account, failover to another account, the wait when none is free (ADR-0005) and the outage breaker stay in
the core, where they are tied to a card's run (`internal/orch/accounts.go:135-160`, `failover.go`, `health.go`). The runtime
holds what `internal/agent/accounts.go` holds today, each account's config dir and login, starts a session on the account the
spec names, and reports what Claude says about an account (at its limit until, signed out) as session events the core's
failover and breaker already read. Signing an account in or out is a `/runtime` call the core makes for the owner's
`accountAdd` / `accountRemove` (`internal/orch/account_signin.go:29`).

### `/runtime`: the core and the AI runtime

The core is the server; the runtime connects with a runtime key, re-sends it on every reconnect, and announces itself.

Core → runtime (calls with replies):

| Call | Carries | Reply |
|---|---|---|
| `sessionOpen` | `agent.Spec` as wire types (card, cwd, model, account, mode, MCP servers with the run's URL and token, allow, deny and ask rules, tools, skills, plugins, read-only paths, system append, resume id) | the session id, or why it could not start |
| `sessionPrompt` | session id, text, a client message id | accepted (the turn's output comes as events) |
| `sessionSteer` / `sessionCancel` | session id | done / steered or not |
| `sessionKill` | session id | killed (SIGKILL of the session's process group); `Session.Close` is this call, as closing a Claude session kills its process group today (`internal/procgroup/procgroup.go`) |
| `permissionAnswered` | session id, request id, decision | ack |
| `sessionTranscript` | session id | the transcript the card screen's context view reads (`internal/orch/cardcontext.go:179` reads the file today) |
| `accountSignIn` / `accountSignOut` | account | the login link, and later the code |

Runtime → core:

| Call | Carries |
|---|---|
| `runtimeHello` | runtime id, build commit, contract version, the sessions it holds (id, card, last event seq, working or idle) after a reconnect |
| `sessionEvents` | session id, from-seq, ordered entries: init, update, task, working, turn ended (stop reason or failure), context used, account state, exit |
| `permissionAsked` | session id, request id, the tool call; answered later by `permissionAnswered` |

Rules:

- **Every wait on a person is two calls.** Generated calls time out (`--timeout 60000`, `Makefile:11`), so a permission
  question is asked and answered by two calls, never one held open.
- **Events are ordered and resumable per session.** Each carries a seq; the core acks the highest it stored; the runtime keeps
  what is not acked and resends from the core's seq after a reconnect. Calls of one method are handled in order (the
  socketrpc-gen Go server's rule, agreed with its agent on 2026-11-04).

### `/worker`: the core and a person's Paseo

P1's sign-in (Google device flow, `wk_` keys), feed, cursor and offers stay as planned. What a person does:

| Act | Shape | How |
|---|---|---|
| accept, decline (with ETA) | every shape | Paseo popup or Work list |
| `answer` | decision | Paseo; the answer is the card's report and ends the turn |
| `ask` | decision | Paseo, to the lead |
| attach, prompt, steer, cancel | work, takeover | the Paseo provider; the core checks the card is this person's and relays to `/runtime` |
| answer a permission prompt | work, takeover | the Paseo provider, once the time limit has passed undecided (*A person's card*) |
| `hand_in`, `ask_lead` | work, takeover | the session's tools, when the person tells it to |

The plugin registers a Paseo provider (`server.registerProvider`, https://paseo.sh/docs/plugins/providers.md) whose sessions
are hosted: `session.open` attaches to the card's runtime session through `/worker`, prompts and steering go up, the timeline
comes down as `session.turn` and `timeline.item` events. The runtime never accepts a connection from a person: the core is the
one entrance.

### A person's card

- **Turns.** In an AI employee's session, a turn's end is the report (`internal/orch/session.go:522`), as today. In a person's
  session, a turn's end is nothing: the person may talk with the session for many turns. The card's report is the session's
  `hand_in`. One function in the core owns this rule, keyed on the card's kind.
- **The lead's messages go into the session** (the owner's decision, 2026-11-04). The card's queue delivers a new instruction,
  a send-back after `accept_work` or the answer to `ask_lead` as a prompt, between turns, as for an AI employee; the person sees
  each in the session's timeline in Paseo, marked as from *the lead*. A person is not asked for a plan sync, as today
  (`session.go:503`).
- **The session lives as long as the card.** It stays open after `hand_in`, through a send-back, and closes when the card is
  accepted, stopped or handed over.
- **Tools and memory.** A person's card holds what any hire holds: the card's tools and the capabilities the Chair grants, with
  the person brief in place of the agent brief; `remember` and `recall` use that person employee's memory.
- **Permission prompts:** see *Permission prompts*, the same ladder as every session's.

## Permission prompts

Every session's permission prompts go up one ladder (the owner's decisions, 2026-11-04): the Chair decides; a Chair that
cannot decide sends it to the owner; when nobody has decided within the time limit, the session's driver decides: the person
for a person's session, the Chair for an AI employee's.

1. `permissionAsked` reaches the chain's Chair as a message; the Chair answers with `answer_permission(request, allow | deny,
   why)` or passes it up with `escalate_permission(request, why)`, which opens a `permission` item in the owner's inbox.
2. The time limit runs from the ask, whatever step it is at. When it passes undecided, a person's Paseo shows the prompt (the
   provider's permission capability) and the person answers it; for an AI employee's session, the Chair is told the decision
   is now its own and answers with `answer_permission`.
3. The first answer wins and the others are refused; the owner's inbox item closes when anyone answers. The event names who
   decided (Chair, owner, person) and why.
4. Until it is a person's turn to decide, Paseo shows that their session waits for an approval.

A session with no Chair above it (the Chair's own, the goal judge's, the Assistant's) asks the owner first; when the owner has
not decided within the time limit, the session decides its own prompt (the owner's decision, 2026-11-04). The time limit is a
setting the owner changes by telling the Assistant; it starts at 30 minutes (the owner's choice). Deny rules are unaffected: they hold in every permission mode
(`internal/agent/agent.go:91`). **Secrets are always refused** (the owner's decision, 2026-11-04): a prompt that names a secret
(Remit's own keys and database, `.env`, credentials, `.ssh`, as `internal/orch/policy.go:36` and `:280-283` name them) is
refused by code before the ladder; nobody, the Chair, the owner or the session itself, may allow one. The Chair's brief, which says every agent's prompts go to the owner directly
(`internal/orch/prompts.go:39`), changes with this.

## Takeover and hand back

The Chair calls `hand_over(card, employee, why)`. The core, in one transaction of the card's line:

1. records a Decision `handover` with the Chair's reason (the dataset of where AI employees fall short);
2. asks the leaving session for a handover note with one turn and a time limit; on failure, the note is what the core holds:
   the card's plan, draft, last report and the worktree's `git status`;
3. ends the leaving card as handed over and closes its session (its transcript stays on that card, for the owner);
4. hires the new employee on a new card in the same worktree, with a `handoff` edge from the old card and a brief built for that
   employee: for a person, the person brief (no chain, Chair, product or other names), the task, and the handover note;
5. offers the card to the person, who accepts or declines as with any offer. A decline returns to the Chair with the reason.

Hand back is the same call toward an AI employee, with the person's session writing the note.

## Rails

- **Pause.** No turn starts on a paused chain, whoever drives: the core holds the queue as today and refuses a person's prompt
  relayed from `/worker` with the reason, shown in Paseo.
- **Stop.** The core revokes the run's MCP token (every act and tool call of that session is refused from that moment), calls
  `sessionKill`, and records the stop. If the runtime does not confirm in time, the core kills the runtime process and starts it
  again; the other sessions resume with `--resume`.
- **Authentication.** A runtime key (as client keys, ADR-0009) and `wk_` device keys (P1). Both revoke at once.
- **Who acted.** A relayed prompt, a handover and a stop carry their actor (person, Chair, owner) in the event, as acts do today.

## Lifetimes

- **The runtime is the core's child.** The core starts `remit runtime` and restarts it if it exits; a core restart, a
  release included, restarts the runtime with it (the owner's decision, 2026-11-04). Running turns resume with `--resume` and
  `ContextLost` repeats the brief, as after a server restart today (`internal/orch/recover.go:26`).
- **A reconnect without a restart** (the socket dropped, both processes alive) loses nothing: `runtimeHello` lists the sessions
  the runtime holds, and events resend from the core's seq.
- **On the pod** pm2 runs one process as today; `/healthz` reports the runtime's state beside the core's.

## Disk and data

- **Worktrees** are on the core's disk; the runtime, on the same machine, works in them directly. The core stays the one
  writer of every repository's refs and the one that pushes.
- **Transcripts.** Claude writes them in the runtime's data dir (today `<dataDir>/claude/projects`,
  `cmd/remit/main.go:277,285`). The timeline of every session is stored by the core as card events with employee, model
  and account; the raw files stay on the runtime's disk and are kept.
- **Skills and plugins** are directories on the same disk, named in the spec as today.
- **Secrets** never leave the core. The runtime holds only the Claude accounts' logins.

## What this replaces in the bridge design and its plan

- The shim's tagging of acts as the person's own (P1 global constraint "a person's acts are made by the person, never by their
  AI", plan index line 18): in work and takeover cards the session's tools are the acts.
- P2 whole: a person starting work with their own agent, binding a local session, uploading its transcript, forwarding the
  lead's messages to it (they go into the hosted session), the tools proxy. A person's work runs in a hosted session.
- Slice 3's bundles from a person's machine (`04-code-handin.md`): the person's session commits in the worktree on our disk.
  Author rewriting on the push ref remains.
- `internal/workerclient`: replaced by the generated Go client.

The build plan is `docs/superpowers/plans/2026-11-04-employee-runtime.md`; the human-employees plan's slices 3–6 follow it.

## Decision records

- **ADR-0058**: Claude sessions run in an AI runtime outside the core, behind `agent.Launcher`; the core keeps account choice
  and failover, the runtime the logins. Changes ADR-0040 (logins move to the runtime) and ADR-0004 (the runtime's HOME).
  *Revisit when:* a runtime on another machine (code by git from the core, skills fetched by content hash), per-session
  sandboxes, mutual authentication of core and runtime.
- **ADR-0059**: a person drives a hosted session on a card of their own; the Chair hands work over with a new card; a decision
  card has no session; every session's permission prompts go Chair, owner, then the driver after a time limit. Extends ADR-0051.
  *Revisit when:* an ask rule the owner wants answered only by himself; a person in the Chair's seat; the person's own tools reaching a hosted
  session (a remote terminal, a file view).
- ADR numbers 0053–0056 stay with the plan's slices.

## Order of work

1. `socketrpc-gen` v9 ships the Go client, with the Socket.IO library it now owns (proven by a Go-client-to-Go-server test with
   namespace, auth, reconnect and the connect race). The same change in Remit moves the server to that library's
   `servers/socket` and deletes `third_party/socket.io-socket` and its `replace` (`go.mod:89`), once the library carries
   Remit's `_doConnect` fix and `connect_ack_test.go` (`third_party/socket.io-socket/FORK.md`; asked of its agent
   2026-11-04).
2. `/runtime` and `RuntimeLauncher`; every session moves out at once, with the runtime as the core's child and the stop rail.
3. P1 re-cut: sign-in, feed, offers, decision cards with `answer` and `ask`.
4. A person's work in a hosted session: the relay, the person turn rule, `hand_in`, the provider.
5. The permission ladder for every session: the Chair's two tools, the time limit, its brief.
6. `hand_over` and the Chair's brief on when to use each shape.
7. Slices 4–6 as planned, slice 3 reduced to author rewriting.

## Left to the builder

Field names, the event entry type shared by `/runtime` and `/worker`, time limits (handover note, kill confirmation),
the runtime key's storage, and how
the provider maps timeline entries to Paseo's item types.
