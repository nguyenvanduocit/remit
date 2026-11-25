# Human employees: engineers work in Remit as employees, through their own tools

Status: design, 2026-11-03. Source: the owner's words of 2026-11-03, and a council of five reviews (the builder, the runtime,
the human engineer, the AI worker, the Chair) whose findings are folded in. Builds on ADR-0032 (employee), ADR-0033 (memory),
ADR-0048 (the Chair is a seat), ADR-0038 (a repository is named by its remote), ADR-0025 (a repository declares its checks),
ADR-0009 (client keys), ADR-0026 (the owner key), ADR-0049 (signals), ADR-0050 (merge through one door).

## Intent

The owner has a team of engineers. They contribute to Remit's chains as **employees, exactly as an AI employee does**: a
Chair gives them work, talks to them and has what they hand in reviewed; what they learn and how they work is recorded the way
an AI employee's is. Each engineer keeps the tools they already use (Claude Code, Paseo, Orca, a terminal): Remit offers a
**protocol**, and a plugin on the engineer's side speaks it.

The owner's words that set the shape:

- A human here is "entirely in the role of an employee/agent in the system". The owner stays the owner; asking the owner "is
  really asking me".
- Employees work with each other through merge requests: "the frontend agent works with the backend agent because I submitted
  an MR, and the Chair announces it, or the MR is updated, and the FE employee knows and carries on". "Design that signal well."
- Besides MRs, the engineer uses the tools an employee already has.
- The plugin sends the whole transcript and the answers up; the server stores them as it does for AI employees; the final
  answer is the result, as today. Safety of the transcript comes later.
- The owner can change Paseo. The network (how an engineer reaches the server) is devops' work.

## Goals

1. An engineer is an `Employee`, given work, reviewed and retired like any other.
2. A person runs through the same session interface an AI runs through, so every rule from a turn to a report to
   `accept_work` holds for them unchanged.
3. One protocol, on the Socket.IO server the browser uses, lets any tool on the engineer's side take work, use the worker's
   tools, stream its transcript and hand in a result.
4. A person hands in what an AI worker hands in: a report and, for code, commits that land in the chain's worktree. From there
   the path is today's: review and QC on the change, then the Chair's `submit_mr`.
5. Employees coordinate through MRs: an MR's versions, checks, verdicts and state are signal readings.
6. The Chair plans with what a person really is (slow, sometimes away, with a load across chains) and judges them by the same
   gate as an AI.

## Non-goals

- How an engineer's machine reaches the server. Devops provides a URL that carries WebSockets; the plugin takes it as config.
- Several humans with owner rights, roles, or a member list. The owner key stays the only way in as the owner (ADR-0026).
- Filtering, redacting or pausing transcripts (owner's decision: everything goes up; the seam is named in decision 9).
- Measuring or ranking engineers. Raw history the store holds is shown to the Chair (decision 4); no score is computed.
- A form or settings screen. The owner hires and manages by telling the Assistant; a person states their own availability
  through their plugin.
- A person in the Chair seat. ADR-0048 already leaves room for it.

## Anti-goals (a failure even if it works)

- A second path to an MR beside the Chair's `submit_mr`, or any path past the approval gate (`internal/orch/submit.go:357-369`).
- An act that should be the person's made by their AI: a report, a verdict, accepting work, or a Chair message reaching their AI
  without them.
- A lighter review for a person than for an AI. **Same gate, different plan**: the gate is equally strict; the Chair plans
  knowing a person may answer in a day.
- Remit code that depends on Paseo. Paseo is one plugin among others.

## Decisions

### 1. `ask_human` becomes `ask_owner`

Once engineers are employees, "human" no longer names one person. The tool that asks the owner is `ask_owner`; "owner" is the
word the code, prompts and docs use (`internal/orch/tools.go:342`, `prompts.go:39`). "Operator" is not used: it names whoever
runs the server (ADR-0026, `cmd/remit/agents.go:95`). The rename covers every occurrence (`rg -l ask_human`: 16 files when
written), the `withdraw_question` wording, the inbox label (`agentstanding.go:446`), and stored records that name the tool (a
numbered migration rewrites them).

### 2. A person is an employee of kind `person`

- `Employee` (`internal/model/employee.go:48`) gains `Kind`: `agent` or `person`. A migration sets `agent` on every stored
  employee.
- The owner hires a person through the Assistant, as a change item of kind `employee` (ADR-0032), with title `worker`, tenure
  `standing`, a scope and `ReportsTo`. No agent creates a person employee.
- Retiring a person keeps their record, memory and transcripts, as for any employee (ADR-0032).

### 3. A person runs through `agent.Launcher`

Remit starts every agent through `agent.Launcher` (`internal/agent/agent.go:120`). A person is a launcher of its own,
`person`, so a person's card is a card with a run, and the existing path (queue, turn, report, plan sync, rework,
`accept_work`, tools scoped by the run's token, `session.go:766-776`) works for it unchanged:

| Session | For a person |
|---|---|
| `Start` | returns at once and binds the plugin when it connects (a slow bind must not fail the card under `Start`'s timeout, `session.go:243-256`) |
| `Prompt` | delivers the turn's message to the plugin and returns only when the person sends `reply`; the turn has no deadline |
| `Steer` | pushes a message into the open task |
| `OnUpdate` | the transcript, as an event kind of its own: the report is the turn's message text (`internal/orch/stream.go:89-105`), so only `reply` produces message text, never the engineer's AI |
| `Close` | detaches the plugin |

Because `Prompt` returns only on `reply`, the end of a turn in the engineer's own AI never reaches the Chair. A person's run is
exempt from what assumes a machine:

- the silent-turn cut (`cutSilentTurns`, `schedule.go:452`) and the stall rule (`schedule.go:255-262`);
- the restart rule that marks every running card stopped (`recover.go:106`): a person's run survives a release, and the plugin
  rebinds when it reconnects;
- the employee lease (`lease.go`): a person may hold cards in several chains at once; their memory writes are serialized by the
  store's revisions;
- account binding, parking when accounts run out, and provider failover (`accounts.go`, `failover.go`);
- `atWork` (`schedule.go:347`) and the wait rules (`wait.go:207-209`, `283-286`): a person's open turn does not count as work
  that holds a slot or blocks the Chair from waiting. A chain whose only open turns are people's releases its slot
  (`step.go:497-499`) and shows as waiting on those people, by name.

The card status `waiting` keeps its meaning (no provider can run it, `model.go:330-332`); a person's card is `running` while
their turn is open.

### 4. A Chair puts a standing employee to work, and sees what it needs to plan

Today `Hire` always creates a new employee (`tools.go:104`, `lifecycle.go:586`). Built for every standing employee:

- `HireInput` (`tools.go:23`) gains `employee` (an active standing employee) and `role` (worker, `reviewer` or `qc`, which
  today comes from the definition's `Signs`). With a person, `cwd`, `tools`, `skills` and `model` are refused.
- `hire_agent`'s listing and `list_agents` show, for each person: kind, whether their plugin is connected, open cards in every
  chain with their age, the availability they stated, and their last cards with outcome (accepted, sent back) and duration.
- Delivery state of each message to a person's card: queued, delivered to the plugin, seen by the person.

### 5. The worker channel and how a device signs in

- **A namespace of its own.** The browser's `Broadcast` pushes every change to every connection (`internal/rpc/broadcast.go:20`),
  and the channel is fixed as `web` (`rpc.go:35`). Workers connect to a Socket.IO namespace `/worker` that receives only its own
  cards' traffic, served by a handler set generated from a contract file of its own beside `web/src/rpc/define.ts` (`make gen`).
- **Who acts.** A worker call acts as its card: `model.ByAgent(cardID)` with `Via: "worker"`, so every rule that checks a card's
  own actor holds (`session.go:510`, `change_item.go:46`, `objective_tools.go:320`). The worker key only lets the connection in,
  as the person employee; each call names a card that employee holds.
- **Device pairing.** The engineer runs `remit-worker login <url>`; the server returns a short code and the device waits. The
  owner's inbox shows "a device asks to sign in as <name> · code <code>"; he approves there or tells the Assistant. The server
  issues a key (`wk_` plus 256 random bits) straight to the waiting device, which keeps it in the OS keychain. One key per
  device, sealed, listed, rotated, paused and revoked as client keys are (ADR-0009, `internal/store/client_key.go`); revoking
  closes that device's socket. Pairing requests are rate-limited and codes expire in 10 minutes.
- **Contract version.** The handshake names the worker contract version. An incompatible plugin is refused with the version it
  needs, never by the web bundle rule (`rpc.go:90-93`).

### 6. The protocol

Names follow A2A's task model (`a2a-protocol.org`, v1.0.0). The transport is ours: in A2A the side doing the work serves the
endpoint.

**Down.** A card's messages are already ordered rows in its queue (`outbox.go`): the protocol pushes those, and the event id is
the cursor a reconnecting plugin resumes from. No log beside the queue.

| Event | Carries |
|---|---|
| `task.offered` | chain, card, role, the brief written for a person (paths relative to the repository root), deliverable and criteria, the worker rules the report follows (`prompts.go:175-186`), inputs, repositories |
| `task.cancelled` | why, and the grace window to hand in what is done |
| `message` | a message to the card (a Chair route, an answer to `ask_chair`, the owner), with a "since your last turn" delta: new commits on its repositories and the MR readings that changed |
| `report.result` | `accept_work`: accepted, or sent back with feedback |
| `turn.closed`, `turn.reopened` | a pause or a stop cut the person's open turn (with why); a resume opened it again on the same message |
| `commits.result` | the outcome of a hand-in (decision 8) |

**Up.** Every call carries a request id and acts once.

- **Person acts** (decision 7): `accept` (with the person's ETA), `decline` (with why), `forward` (a message to their AI),
  `reply`, `verdict`.
- **Before accepting**, the person may `ask_chair` about an offer.
- **The worker's tools**, from the code that builds an AI worker's: `update_plan` (`tools.go:484`), `read_check` (`:507`),
  `ask_chair` (`:539`), the chain's capability tools (`remember` and `recall`, `memory_tools.go:56-64`), `send_feedback`
  (`feedback.go:94`). The plugin may expose them to the engineer's AI as MCP tools.
- `transcript.append`, `draft`, `commits`, `dispute`, `availability`.

### 7. What only the person does

The person's AI may draft; these are the person's own acts, made through a command the model cannot run
(`disable-model-invocation: true`, "Claude can't invoke the skill on its own", `code.claude.com/docs/en/skills`), a button in
the Paseo plugin, or the CLI:

- **`accept` / `decline`** an offer. An offered task waits; nothing starts on their machine before they accept.
- **`forward`**: a Chair or owner message lands in the plugin's inbox and reaches the person's AI only when they forward it. A
  Chair never types into an engineer's session.
- **`reply`**: what reaches the Chair, ending the turn in the same `o.report` an AI's turn end calls (`session.go:500`), plan
  sync included. `draft` (callable by the AI) prepares it and follows the worker report rules: each criterion met or not, with
  evidence and a confidence label (`prompts.go:176`). A reply may carry decisions the person made outside the system ("agreed
  with the customer on Slack to drop v1"); each becomes a chain `Decision` record a later worker can cite.
- **`verdict`**: a person reviewer's or QC's `record_verdict`, pinned to the worktree head the person read; refused when the
  worktree moved since.

### 8. Code moves as bundles; one writer per repository

The path stays today's: workers commit in the chain's worktree and never push (`tools.go:624`); review and QC record verdicts on
the change; the Chair's `submit_mr` checks approvals, pushes and opens the draft MR.

- `task.offered`, and a `message` whenever the branch moved, carry a **git bundle** of the chain's branch. The engineer needs no
  push rights and no forge account.
- `commits` uploads a bundle of the person's commits on top of the head they started from, in chunks smaller than one socket
  frame (the server closes a frame over 1,000,000 bytes, measured 2026-11-03). Remit verifies it, checks it descends from
  the worktree's head, and fast-forwards the worktree. A bundle that does not descend is refused with the new head.
- **One writer per repository, enforced.** A hand-in while another card has a turn open on that repository, or while the
  worktree has uncommitted changes (`gate.go:55-58`), is refused with the reason; the Chair sequences writers.
- **Authors.** `submit_mr` rewrites commit authors for the forge's push rules by rebasing in the worktree (`repo.go:412-438`,
  `submit.go:353`), which would change the ids the person's next bundle descends from. The rewrite moves to a temporary push ref;
  approvals stay valid because they key on the change's content (`gate.go:67`). Each original author is kept as a
  `Co-authored-by:` trailer, and the MR description names the employees in the version's `by`.

### 9. The transcript

- A person's card stores the whole bound session as transcript events (decision 3), from its first turn: their prompts (`By` the
  employee), their AI's text and tool calls. The card screen shows it as an AI card's own.
- Remit cannot prove a transcript is what happened. Review, QC and the judge rule on the change and the report.
- **Revisit when** the owner asks for transcript safety: filtering, redaction, a visible pause, and a log of who read a person's
  transcript go between `transcript.append` and the store, one function with the plugin's events as input.

### 10. The MR signal carries versions, checks and verdicts

Today `chain:<id>.mr.<repo>` (`internal/orch/live.go:25`) holds `iid`, `url`, `state` and `decidedAt`, and a reading is written
only when the state changes: the merge-request sensor writes what the forge says (`senseMR`,
`internal/orch/merge_request_sense.go:49`) and the shipped subscription's `fireRecordMR` (`:89`) is its one writer. The reading
becomes:

| Field | Meaning |
|---|---|
| `iid`, `url`, `state`, `decidedAt` | as today |
| `head` | the SHA the MR stands at on the forge |
| `version` | the pushes `submit_mr` made to this MR, 1 for the first |
| `checks` | `pending`, `passed` or `failed` for `head`, with the failing check's name |
| `verdicts` | the latest verdict per role on the change at `head` |
| `by` | the employees whose accepted reports make up the version |

A reading is written when any field changes, each with its cause. These fields are what a `subscribe` `until` condition reads.

### 11. Coordination through MRs

- Within a chain, the Chair learns of every report and `submit_mr` result and routes the work on ("backend!42 at v2 is the API,
  build on it"); a person receives it as a `message` with its delta.
- Across chains, a Chair may read another chain's `mr.<repo>` readings when both chains serve the same objective (ADR-0031);
  today a chain reads its own records only (`watch.go:152`). Only the Chair subscribes (`wait.go:193`).
- **Contract first.** When a person and an AI share a feature, the Chair's brief says: first hire the side that defines the
  interface (types, an OpenAPI file, fixtures), accept it, and hand it to the other side as an input, so both work in parallel.
  A change to the contract reaches the other side as a message from the Chair.

### 12. Time and accountability

- **Offer.** Not accepted within the hire's `deadline` (default 24 hours) → the Chair is woken with "not taken up".
- **ETA.** `accept` carries the person's ETA; passing it wakes the Chair with "overdue". The Chair may reassign, or escalate to
  the person's `ReportsTo`.
- **Availability.** The person states it through the plugin ("away Friday", "full this week"); the Chair reads it (decision 4).
- **Ending.** `finish_chain` is refused while a person holds accepted work; the Chair cancels with a reason, and the person has a
  grace window to hand in. A hand-in after the window is refused with the reason.
- **Dispute.** A person who disagrees with a send-back or a verdict calls `dispute` with their reasons. It goes to the Chair's
  `ReportsTo` (the owner's inbox when the Chair reports to him), and the work waits on its answer.

### 13. Memory

A person has the memory every employee has (ADR-0033): `remember` and `recall` read and write that employee's own memory
(`memory_tools.go:50-51`). The decisions a reply carries (decision 7) are what other workers can cite. A team memory that every
employee reads is the next ADR, named here and not built.

### 14. Prompts

- Worker rules: "never send messages to people" (`prompts.go:185`) becomes "to people outside the chain". An input from a
  person's card is marked as a person's.
- Chair rules: same gate, different plan; hire a person for what an AI cannot do (credentials or devices they hold, taste, a
  customer relationship, deep context); put a person on the review or QC gate only when human judgement is the point, after the
  AI review and QC pass; contract first (decision 11); briefs for people name paths relative to the repository root.

### 15. What the owner sees

- The Crew tab shows person employees in the tree, marked as people (text and an icon), and whether their plugin is connected.
- A person's card screen shows the transcript, the draft, offers and acceptances, replies, hand-ins and disputes, live.
- Pairing requests and disputes reach him in the inbox. Hiring, revoking a device and retiring are said to the Assistant.

## The plugins

- **Claude Code**: `/remit:accept`, `/remit:forward`, `/remit:reply`, `/remit:verdict` (model invocation
  disabled), an MCP server with the worker tools and `draft`, and hooks that send the bound session's transcript. Pushed events
  reach an interactive `claude` through a channel (`code.claude.com/docs/en/channels-reference`, research preview). A channel
  does not reach a session driven through the Agent SDK: tested 2026-11-03 with Claude Code 2.1.312 in `--input-format
  stream-json` mode, the test server's two `notifications/claude/channel` messages never reached the model while its tool worked.
- **Paseo**: Paseo runs Claude Code through the Agent SDK (`paseo.sh/docs/claude-code.md`). A Paseo plugin holds the connection,
  binds a Paseo agent to the accepted task, streams its timeline as transcript, shows the inbox, and sends a forwarded message as
  a follow-up turn (lifecycle hooks and follow-ups, Paseo plugin reference). It shows the draft and the person's buttons.
- **CLI** (`remit-worker`): `login`, `tasks`, `accept`, `watch`, `forward`, `draft`, `reply`, `verdict`, `commits`,
  `dispute`, `availability`, for any other tool.

The plugins live in this repository, each in its own directory, built against the generated worker contract.

## Constraints

- No server code names Paseo (AGENTS.md).
- Server-driven: the plugin holds no state Remit needs, and may be dropped and reconnected at any time.
- Every call up acts once on replay; every event down is ordered and resumable.
- A rail change (a credential kind, device pairing) names, in its ADR under *Revisit when*, the control not built: transcript
  safety, scopes finer than one employee, binding a key to device hardware.

## Records

- ADR: *A person is an employee who runs through a launcher of its own* (decisions 2, 3, 6, 7, 9, 12).
- ADR: *A Chair puts a standing employee to work* (decision 4).
- ADR: *A device signs in as a person by the owner's approval* (decision 5).
- ADR: *An MR's versions, checks and verdicts are readings, and chains of one objective read each other's* (decisions 10, 11).
- ADR-0032 and ADR-0026 updated where they say the owner is the only human and the owner key the only credential.
- Migrations: `Employee.Kind`; stored records naming `ask_human`.

## Order of slices

Each slice ships whole: tests, docs, migration, the owner's screen looked at.

1. `ask_owner`.
2. Person employee, the `person` launcher with its exemptions, the `/worker` namespace with device pairing, the protocol for a
   task with no repository, the CLI. A person can take a task, reply, be accepted or sent back, and dispute.
3. Bundles, one writer per repository, authors on a push ref, person verdicts.
4. The MR signal fields, cross-chain reading, the Chair's prompt rules.
5. The Claude Code and Paseo plugins.

## Left to the builder

- The exact event and call shapes in the worker contract, within the lists above.
- The plugin's local configuration (server URL, keychain entry).
- The grace window's length and the delivery-state details.

## Calls made in the design, not by the owner

- Device pairing over a key the owner hands out.
- An offered task waits for the person; a message reaches their AI only when they forward it.
- A person's verdict and reply are their own acts; their AI drafts.
- Code moves as chunked bundles; engineers need no push rights; authors survive as `Co-authored-by`.
- Disputes go to the Chair's `ReportsTo`.
- Cross-chain MR reading between chains of one objective; only the Chair waits.
- Offer deadline of 24 hours by default.

Assumption: the transcript goes up whole with no pause, because the owner decided "for now send every transcript; safety
later" and did not take up the pause when asked; the pause is named under decision 9's *Revisit when*.
