# 0073. A subagent is a standing employee at work, and a workflow is a script Remit replays from its journal

Status: accepted, 2026-11-10; its goja engine superseded by ADR-0075. Built by `docs/superpowers/plans/2026-11-10-subagents-and-workflows.md`. Source: the owner's words of
2026-11-10, "implement both layers now": (1) Claude Code's own subagents and its Workflow tool are adopted inside Remit, so a subagent
Claude spawns is one of our standing employees at work and the owner sees it; (2) Remit runs workflow scripts itself, durably, so a
Chair writes a script (loops, fan-out, verify) instead of driving each step with one tool call per turn. Supersedes ADR-0032's *one
live turn per employee* (the lease). Builds on ADR-0052 (a Chair puts a standing employee to work), ADR-0058 (sessions run in the AI
runtime behind `agent.Launcher`), ADR-0062 (every seat is an AI employee), ADR-0063 (the core is the only writer of an employee's
memory) and ADR-0064 (a wait is a subscription that keeps a schedule). Lands in Go; ADR-0072's cutover carries it into Rust. Read
against the working tree and Claude Code 2.1.322 on 2026-11-10.

## Context

The customer is the owner. Today a Chair moves its chain one tool call per turn: hire, wait for the report, judge, hire the next.
A known shape of work (ten reviewers in parallel, a loop of fix and verify until the tests pass, a pipeline that waits for a signal
between two steps) costs the Chair a turn for every step and every report. Inside its turns, Claude Code can already fan out on its
own, with subagents and a script, and Remit hides all of it from the owner.

What Claude Code does (2.1.322, its extracted bundle; confidence per line):

- **Subagent types are per session.** The SDK's `initialize` takes `agents`, a map of name → `{description, prompt, tools,
  disallowedTools, model, effort, skills, mcpServers (inline stdio or http allowed), hooks, memory, isolation, maxTurns}`; a custom type
  shows in the session's `init.agents` (probe, HIGH). Remit sends no `agents` (`initialize`, `internal/claudecode/options.go:125-137`).
- **What sessions are offered today.** In the dev database, 384 of 431 stored session inits list the subagent tool (named `Task` in
  `init.tools`) and `Workflow`; their `agents` are the built-ins `claude`, `claude-code-guide`, `Explore`, `general-purpose`, `Plan`,
  `statusline-setup` and `web-fetch` (`sqlite3 ~/.remit-dev/remit.db`, table `session_inits`, HIGH). Remit denies neither tool
  (`denyTools`, `internal/orch/policy.go:36-50`; `options`, `internal/agent/options.go:30-55`, HIGH).
- **The stream says what a subagent does.** `system/task_started` carries `task_id` (equal to the subagent's `agent_id`),
  `tool_use_id`, `subagent_type`, `is_backgrounded`, `spawn_depth`, `task_type` (`local_agent` or `local_workflow`) and `prompt`;
  `task_progress` its usage and last tool; `task_updated` its status; `task_notification` its end (`status`, `output_file`, `summary`,
  `usage`). Every message a subagent writes carries `parent_tool_use_id`, `agent_id` and `subagent_type` (with `initialize`'s
  `forwardSubagentText: true`), and a `can_use_tool` request from a subagent carries `agent_id` (probe). A `stop_task` control
  request ends one task (MEDIUM: read in the bundle, not yet driven by Remit).
- **Remit drops it.** `streamEvent`, `assistant` and `toolResults` return on any message with a `parent_tool_use_id`
  (`internal/agent/claude.go:339`, `:362`, `:403`, HIGH). `sdkMessage` parses the task fields already
  (`internal/agent/agent.go:137-170`); `taskLog` keeps background tasks only (`internal/agent/tasks.go:21-90`).
- **A subagent acts as its parent.** It inherits the parent's MCP servers, so today it calls `mcp__remit` with the parent card's
  token (MEDIUM, Claude Code's docs). The token is minted per session and names one card (`newRun`, `internal/orch/session.go:62`;
  `CardForToken`, `:642`; the server entry, `internal/orch/access.go:104`). MCP calls carry no agent id: the `x-claude-code-agent-id`
  header goes to the model API only (`chunk-jshe6j36.js`, LOW that nothing else carries it).
- **The Workflow tool.** A plain-JS script run in Node's `vm` inside Claude Code's process (`chunk-yw1by7x4.js`), `eval` removed, with
  `agent(prompt, {schema, agentType, isolation, model, effort, label, phase})`, `parallel`, `pipeline`, `phase`, `log`, `budget` and
  `workflow()`; `Date.now` and `Math.random` throw, so a resume replays `journal.jsonl` (one result per finished agent, the matching
  prefix cached). Launch answers `{status: "async_launched", taskId, taskType: "local_workflow", runId, transcriptDir, scriptPath}`;
  resume is `Workflow({scriptPath, resumeFromRunId})` in the same session. `agentType` resolves from the session's types; every spawn
  passes a plugin's `agent.spawn` hook, which may deny it (`chunk-913dnra7.js`: "Workflow agent spawn denied by a plugin"). At most 16
  agents at once and 1000 per run. Settings `enableWorkflows`, `disableWorkflows` and `CLAUDE_CODE_DISABLE_WORKFLOWS` gate it. No Remit
  transcript has called it (MEDIUM: names checked in the bundle this session, semantics from the probe).
- **Both die with the parent's process.** Subagents and workflow runs live in the process of the card that started them, on its
  account.

What Remit does that this meets:

- **A Chair hires.** `Hire` with `HireInput` (`agent` or `employee`, `role`, `brief`, `deliverable`, `criteria`, `inputs`, `why`,
  `internal/orch/chair_hire.go:19-35`); only the Chair hires (`:74`). A standing employee is put to work within the Chair's ceiling
  (ADR-0052).
- **A chain changes through one pure `step`** (`internal/orch/step.go`) applied by one writer per chain; every entry to running
  passes `admit` (`internal/orch/admit.go`); a Chair's wait is a subscription and the chain then holds no session and no slot
  (`internal/orch/wait.go`, `timewait.go`). A Chair's session closes whenever its chain waits.
- **One live turn per employee.** `turnLeases` (`internal/orch/lease.go`) holds an employee's turn from `take` to the turn's end;
  ADR-0032 gave the reason: "two turns of one employee never run together, so its memory has one writer at a time". Since ADR-0063 a
  session reads its memory through `search_memory` and never writes it; the core alone admits, from accepted cards. The lease's
  reason no longer holds.
- **The rails.** Pause cuts a chain's turns and ends its sessions (`pause`, `internal/orch/lifecycle.go:467`); `StopCard` ends a card
  for good (`:378`).

**PoC run for this record (HIGH).** goja (`github.com/dop251/goja` v0.0.0-20261108153707, Go 1.27.3) ran
`(async () => { const r = await Promise.all([agent("a"), agent("b")]); return await agent("verify " + r.join(",")) })()` with `agent`
a Go function returning a promise that resolves from a journal or stays pending: with an empty journal the run ends with calls 0
and 1 pending; with 0 and 1 recorded, call 2 is pending; with all three, the script fulfils with call 2's result. `Date.now` and
`Math.random` replaced by functions that throw do throw.

## Options

### A. How many engines

1. **Claude Code's engine only.** Offer subagents and Workflow, observe them, and build nothing else. A run dies with its process,
   and a Chair's process ends every time its chain waits; a script cannot wait for a signal or the owner, and a restart loses it.
   Fails half the goal.
2. **Remit's engine only, Claude Code's denied.** One engine, durable. It refuses the owner's first layer and fights the framework:
   models are trained on Claude Code's subagents, and a worker that fans out inside its own turn gains nothing from a durable run.
3. **Both, one entity.** Claude Code's engine for work inside one turn, adopted and shown; Remit's for work that spans turns, waits or
   restarts. A run of either is one `Workflow` with an `engine` field. **Chosen.**

### B. What a subagent is in Remit's model

1. **A card of the employee its type names, bound to the parent's process** (`Card.Spawn`). It has a stream, a status, a stop, a
   record and a place in the chain's tree, as every card does. **Chosen.**
2. **A launcher kind of its own**, as ADR-0051's person launcher was. A launcher starts a session, and Remit starts none here:
   Claude Code does, in the parent. A launcher whose `Start` starts nothing is a fake.
3. **A sub-stream inside the parent card.** Cheapest, and the owner would see the work but not who did it: the employee's record,
   its memory and the reporting tree would never hear of it. Fails "one of our standing employees at work".

### C. Who a subagent is when it calls Remit

1. **The parent's token (today).** It acts as the parent. Wrong identity.
2. **A token per spawn.** Types are fixed at `initialize`, before any spawn, and MCP calls carry no agent id: not possible today.
3. **A seat token per (parent session, employee)**, given in each type's inline `remit` server, and a narrow tool surface that needs
   no single card: the employee's own reads and the parent's reads within its ceiling. **Chosen**; option 2 is its *Revisit when*.

### D. The JS engine for Remit's workflows

1. **goja** (pure Go, ES2015+ with promises and async functions, no cgo; PoC above). The module already builds with no cgo
   (`modernc.org/sqlite`, `go.mod:23`). **Chosen.**
2. **QuickJS** through cgo, or compiled to WASM under wazero: complete ES2023, a memory limit; cgo breaks the pure-Go build, WASM adds
   a runtime and a marshalling layer for every verb.
3. **Node or Bun in the jail**, the script talking to the core over a socket: full compatibility with Claude Code's `vm`, and a
   process, a protocol and a reconnect per evaluation for what is a pure function.
4. **otto**: ES5 only, no promises. Out.

### E. How a script survives days and restarts

1. **Journal replay** (Temporal's model, and Claude Code's own): the script is re-evaluated from the start against the recorded
   results of its calls; the first call without a result is issued. The engine is a pure function of (script, args, journal).
   **Chosen.**
2. **Snapshot the VM.** goja cannot serialise a running VM; a snapshot would also tie stored data to one engine's internals, which
   ADR-0072's port would have to read.
3. **Compile scripts to a state machine.** A language of Remit's own; the scripts written for Claude Code would not run.

### F. Where a run and its steps live

1. **`workflows` and `workflow_steps`, tables of their own, carried in `ChainView` with a notification**, as a decision is: a run grows
   to 1000 steps (AGENTS.md: what grows without bound is an entity). **Chosen.**
2. **Fields on the Chain or the Card.** Unbounded growth inside a document every push carries.
3. **Events in the Chair's stream.** The run's state would be derived by folding a stream, in two places.

### G. The lease

1. **Keep it, and refuse a spawn of a busy employee.** `parallel` of one type, the most common script, could never run.
2. **Keep it for sessions, exempt spawns.** Two rules for one employee's turns, kept for a reason that no longer holds.
3. **Delete it.** An employee may be at work in several cards at once; the core is its memory's only writer (ADR-0063). **Chosen.**

## Decision

### 1. One entity: `Workflow`, two engines

`model.Workflow` is one run of a script in a chain: `ID` (`w_`), `Home` (the chain), `Card` (whose hands it is: the card that called
`Workflow` or the Chair that called `run_workflow`), `Engine` (`claude` | `remit`), `Name`, `Script` (its text), `Args`, `Status`
(`running`, `waiting`, `paused`, `done`, `failed`, `stopped`), `Phase`, `Result`, `Error`, `ResumedFrom` (a run whose recorded
prefix it reuses), `Claude *ClaudeRun{TaskID, RunID, ScriptPath, TranscriptDir}` (engine `claude` only), `CreatedAt`, `EndedAt`,
`Rev`. `model.WorkflowStep` is one call the script made: `Workflow`, `Seq`, `Verb` (`agent`, `hire`, `wait`, `ask_owner`, `accept`,
`send_back`, `log`, `phase`), `Label`, `Phase`, `Input` and its hash, `Status` (`pending`, `done`, `failed`), `Result`, `Card` (the card
the step put to work), `At`, `EndedAt`. Tables `workflows` and `workflow_steps` (migration 97). `ChainView.Workflows` carries each run
with its steps; a change pushes the run with its steps at its `rev`. A run's steps are owned by the run: the cards they put to work
are ordinary cards, and a card knows nothing of the step (the step names the card).

### 2. Layer 1: Claude Code's subagents are standing employees

- **The types a session is offered** (`subagentTypes`, one function in `orch`):
  - every card: `general-purpose` and `fork`, which run as the card's own employee (a copy of itself at work in a child card);
  - a Chair and the Assistant: also every active standing AI employee whose ceiling is within the card's, each under its handle
    (`employeeHandle`, the one name both engines resolve: its name and number, e.g. `ana-12`);
  - a judge: none (`Task`/`Agent` and `Workflow` denied: its verdict is its own).
  Each type is the employee's loadout as `library.Access` gives a worker card: its instructions as `prompt`, its tools (narrowed to
  the parent's), skills, model and effort, an inline `remit` server with the seat token, no `memory`. The other built-ins
  (`claude`, `claude-code-guide`, `Explore`, `Plan`, `statusline-setup`, `web-fetch` and whatever a later Claude Code adds) are
  excluded by the subagent tool's allowlist, `Agent(<offered types>)`, in the session's tools. The set is fixed at the session's
  start; an employee hired after it is offered from the parent's next session.
- **A spawn is a child card.** On `task_started` with `task_type` `local_agent`, in one change: a card of the employee the type maps
  to (origin `spawned`, the parent's home, the employee's loadout, name from the spawn's description, brief from its prompt, status
  `working`), `Card.Spawn{Parent, Task, ToolCall}` binding it to the parent's process, and a `hire` edge from the parent. Spawns
  nest: a subagent's spawn is a child of the subagent's card. A `task_started` whose type is not offered is answered with
  `stop_task` and recorded on the parent card as refused: the allowlist is the wall, this is the check behind it.
- **What it does shows on its card.** `agent` routes every message, stream delta, tool call, plan and usage with an `agent_id` to
  `Handler.OnSubagent(agentID)`, the handler of that child, and a `can_use_tool` request with an `agent_id` goes up the child card's
  permission ladder (`permission.go`), shown on the child card. The parent card shows the spawn as a tool call that links to it.
- **It ends with its report.** `task_notification` ends the child card: `completed` → done, with the summary (and the result file's
  text, which the runtime reads) as its output; `failed` → failed; `stopped` → stopped. Its usage is the card's.
- **Stop and the rails.** `StopCard` on a child card sends `stop_task` to its parent's session (`agent.Session.StopTask`). When the
  parent's process ends (pause, stop, a restart, a failure) every child still at work ends `stopped`, saying why; a child is never
  resumed, and the parent's recovery message names the children that were cut. Pause and stop stay what they are: deterministic code
  on the parent's process.
- **What a child card is not.** It holds no session of its own, so the rules that assume one leave it alone: no account choice or
  failover (the parent's), no silent-turn cut (the parent's), no slot, no mailbox: a message to it is refused with "it runs inside
  <parent>: talk to <parent>". One predicate, `spawned(c)`, says so. It is never accepted itself and signs no gate; ADR-0063 admits
  it to its employee's memory when its parent card is accepted, because its work was part of what was accepted.
- **Who it is.** The seat token names (parent session, employee); `CardForToken` becomes `CallerForToken`, whose answer says whether
  the caller is a card or a seat. A seat is served the employee's own tools (`search_memory`, `my_work`, `read_my_card` of its newest
  child card, `send_feedback`) and the parent's read tools within the employee's ceiling; it is refused hiring, routing, asking,
  accepting, submitting and finishing, which are its parent's. Seat tokens end with the parent's session.
- **Claude Code's Workflow tool is a workflow of engine `claude`.** On `task_started` with `task_type` `local_workflow`, a
  `Workflow{Engine: claude}` is made for the card, with the script from the tool call's input and `Claude` from its launch answer;
  each agent it spawns is a child card as above, and a step of verb `agent` naming it, labelled and phased as the stream says.
  Claude Code owns its journal (`journal.jsonl` in its `transcriptDir`): Remit stores the script and the ids, never a copy of the
  journal. When the process ends under a running `claude` run, the run is `stopped` and the card's recovery message names the call
  that resumes it, `Workflow({scriptPath, resumeFromRunId})`; a launch naming that `resumeFromRunId` continues the same record.
- **Who may use it.** Workers and the Assistant may; a Chair may not (`Workflow` is denied to it): a Chair's session closes every
  time its chain waits, which would cut its run. A Chair's script is a Remit workflow.

### 3. Layer 2: Remit runs workflow scripts

- **The engine is pure.** Package `internal/workflow`: `Advance(script, args, journal) Outcome` evaluates the script in a fresh goja
  VM against the journal and returns the calls it made that have no result yet (each with its `Seq`, `Verb`, `Input`), or its result,
  or its error. No I/O, no clock, no store. It is the functional core; `orch` is the shell.
- **The API** is Claude Code's, so a script written for one runs in the other where it uses only the common part: `agent(prompt,
  {schema, agentType, model, effort, label, phase, isolation})`, `parallel`, `pipeline`, `phase`, `log`, `args`, `budget()` (always
  unlimited: Remit records no cost and caps none, ADR-0010) and `workflow()`. Remit adds four verbs, each the Chair's own action:
  `hire(input)` (`HireInput` as `hire_agent` takes it; `inputs` may name earlier steps), `wait({at} | {key, condition, until})`
  (`subscribe`'s input), `askOwner(question)` (`ask_owner`), and `accept(card, notes)` / `sendBack(card, feedback)` (`accept_work`).
  A script that calls one of them runs only in Remit.
- **`agent()` is a hire that judges itself.** It puts the employee `agentType` names to work (the Chair's own employee when it names
  none) with the prompt as its brief and returns its report. With `schema`, the deliverable is "a JSON value matching the schema",
  the core parses the report's last JSON value against it and sends the card back once with the parse error before the call fails;
  the value is the result. The card is accepted when the call returns: the script verifies, as it would in Claude Code. `hire()` is the
  typed form: it returns `{card, report}` and leaves the card awaiting the script's `accept` or `sendBack`.
- **Determinism.** A script is plain JS in one file. `Date.now`, `Math.random` and `new Date()` with no argument throw; `eval`,
  `Function` and timers do not exist; nothing reads a file, the network or the environment. The verbs are the only effects. Each
  call is journaled by its order (`Seq`) with its verb and the hash of its input; on replay a recorded call whose verb or hash
  differs fails the run with "the script diverged at step k". The script is fixed for the run; a changed script is a new run with
  `ResumedFrom` the old one, reusing its steps up to the first that differs, as Claude Code's resume does. An evaluation that runs
  longer than 2 seconds without returning (a loop that never calls a verb) is interrupted and fails the run.
- **The shell.** `orch` starts a run (`run_workflow`, or a routine), and advances it in the transaction that records any of its
  steps' results: a card's report, a wait's firing, the owner's answer. It calls `Advance` and applies the new calls in that same
  transaction through the paths the Chair's tools use (`hiringOf` and the brief, `waitFor` with a subscription whose action is the new
  `workflow` kind naming the run and step, the inbox question, `AcceptWork`). Each hire is a decision by the Chair whose why names the
  run and step. A run's result or failure reaches the Chair as `[workflow <name> ended …]` with its result: the Chair takes no turn
  while its run works.
- **Waits, admission and `step`.** `step` reads a chain's running workflows beside its waits. A chain whose workflows have only
  `wait` or `ask_owner` steps pending, and no card at work, is `waiting`: its sessions close and it holds no slot, as with `subscribe`.
  A firing that resumes a run is a scheduled wake and passes `admit` as one (a slot); the owner's answer is his act (none).
  `finish_chain` is refused while a run of its chain is running, waiting or paused.
- **Pause and stop are rails.** Pausing the chain pauses its runs: no call is issued while it is paused, its cards are cut as today,
  and on resume the next `Advance` issues what is due. `StopWorkflow` (the owner's call, and the Chair's `stop_workflow`) stops the
  run in one change: its cards at work stopped, its subscriptions ended, its question withdrawn, the run `stopped` with who and why.
  An engine-`claude` run is stopped with `stop_task` on its task.
- **A routine may be a script.** `StartSpec.Script` (and `Args`): the chain the routine starts begins with the run, and its Chair takes
  its first turn when the run ends.

### 4. How the Chair chooses

The Chair's brief (`prompts.go`) gains one rule, and a built-in skill `workflows` (`internal/builtin/skills`) carries the API, the
determinism rules and worked patterns (fan-out and judge, fix-and-verify loop, wait for a signal between steps). The rule:
step by step when each next step needs your judgement of the last report; `run_workflow` when the shape of the work is known before it
starts (a fan-out, a loop until a check passes, a pipeline, a wait for a signal or the owner between steps). Workers are told the
same of Claude Code's Workflow for fan-out inside their own turn.

### 5. The lease is deleted

`turnLeases`, `heldLease` and their calls in `outbox.go` go. An employee may be at work in several cards, in several chains, at once.
The hiring rule that prefers an idle worker (`HireInput.Why`) still reads "no card open".

### 6. ADR-0072

This lands in Go now. The Rust `orch` crate does not exist yet (ADR-0072 Wave 4); the cutover's last `git diff <ported-from>..HEAD`
pass of each unit carries it over, migration 97 included in the generated migrations. The stored contract is the API, the journal and
the two tables, not goja: the Rust port picks its own engine (rquickjs or boa), and the port's check is that every stored run replays
to the same pending calls.

## Threats

| Threat | Control now |
|---|---|
| A subagent acts with its parent's identity (spoofing) | The seat token in its type's inline server; seat tools never hire, route, ask or finish |
| A session spawns a type Remit did not offer (a built-in, a renamed one) | The `Agent(<offered>)` allowlist; `stop_task` on any `task_started` outside it |
| A seat token outlives its session | Revoked when the parent's session ends, as a card's token is (`endRun`) |
| A script escapes its VM to files, the network or the environment | goja with no `require`, no host objects but the verbs; the verbs pass the Chair's own checks (`hiringOf`, ceiling, capabilities) |
| A script loops forever or diverges | A 2-second evaluation interrupt; the divergence check on replay; both fail the run and tell the Chair |
| A report steers the script (injection) | Results are data the script reads; the script's code is fixed when the run starts |
| A run creates cards without end | Not capped (ADR-0010); the owner stops the run, the chain, or the routine |

Threat modelling (`aio-threat-models`) runs when the owner asks for hardening.

## Consequences

- The owner sees every subagent as a card of a named employee under the card that spawned it, and every script as a run with its
  phases and steps, its cards under it, a stop beside it.
- A Chair with a known shape of work takes two turns (start the run, judge its result) where it took one per step.
- An employee's record counts its spawned cards; its memory learns from them through its parent's acceptance.
- Two engines run the same API. A script that uses Remit's verbs does not run in Claude Code; one that uses Claude Code's
  `isolation: "remote"` does not run in Remit (refused at the call).
- One employee may now run in several cards at once; work that relied on its turns queuing behind each other no longer does.
- `agent.Handler` gains `OnSubagent`; `agent.Session` gains `StopTask`; `agent.Spec` gains `Agents`; `agent.PermissionRequest` gains
  `AgentID`; `model.Card` gains `Spawn`; `model.Subscription`'s action gains the kind `workflow`; `model.StartSpec` gains `Script` and
  `Args`; `ChainView` gains `Workflows`. Cards are documents: `Spawn` needs no migration.

## Tests (written first, main path first)

- `TestAWorkflowAdvancesFromItsJournal` (`internal/workflow`): empty journal → calls 0 and 1 pending; recorded → call 2; all → result.
- `TestAWorkflowThatDivergesFromItsJournalFails`, `TestAWorkflowCannotReadTheClock` (`internal/workflow`).
- `TestASessionIsOfferedItsStandingEmployeesAsSubagentTypes` (`internal/orch`): a Chair's `agents` are its employees within its
  ceiling plus `general-purpose` and `fork`; a worker's are the last two; a judge has none.
- `TestASpawnIsAChildCardOfItsEmployee` (`internal/orch`, fake launcher): `task_started` makes the card, the edge and the binding;
  the subagent's messages land on it; `task_notification` ends it with its report.
- `TestStoppingAChildCardStopsItsTask`, `TestAChildCardEndsWhenItsParentsProcessEnds` (`internal/orch`).
- `TestASubagentsPermissionRequestIsAskedOnItsCard` (`internal/orch`).
- `TestASeatActsAsItsEmployeeNotItsParent` (`internal/orch`): `search_memory` reads the employee's project; `hire_agent` is refused.
- `TestAChairsWorkflowHiresAndHandsBackTheResult` (`internal/orch`): `run_workflow` with `parallel` of two `agent()` calls makes two
  cards; their reports advance the run; the third call is made; the Chair is told the result.
- `TestAWorkflowWaitingOnASignalHoldsNoSlotAndSurvivesARestart` (`internal/orch`).
- `TestPausingAChainPausesItsWorkflowAndStoppingItEndsItsCards` (`internal/orch`).
- `TestAnEmployeeWorksInTwoCardsAtOnce` (`internal/orch`): replaces the lease's tests.
- `TestClaudeCodesWorkflowIsARunOfEngineClaude` (`internal/agent`, `internal/orch`, from a recorded stream).
- `WorkflowPanel.test.ts`, `AgentTree.test.ts` (`web`): a run shows its phases, steps and cards and a Stop; a child card sits under
  its parent with no composer.

## Revisit when

- **Claude Code's Workflow survives its process and waits on outside events**: Remit's engine retires; the `engine` field and the
  tables stay, and running `remit` runs finish first.
- **MCP calls from a subagent carry its agent id** (or `initialize` can mint per-spawn servers): a token per child card, in place of
  the seat; the seam is `CallerForToken`.
- **A session must see employees hired after it started**: a types reload in the runtime (`initialize` is once per process today);
  the seam is `subagentTypes`.
- **The owner asks for a cap** on cards per run or at once: `workflow.Limits`, read where the shell applies calls.
- **A script must not hold the core's CPU or memory**: evaluate in a jailed child process (`internal/jail`), the same `Advance`
  across a pipe; goja has no memory limit.
- **Scripts are reused**: a playbook names scripts the Chair runs by name, signed by the owner's Apply. Until then `workflow()`
  fails its step with "name a script inline; Remit keeps no saved workflows yet".
- **Per-session effort is built (ADR-0070)**: `agent()`'s `effort` becomes the card's effort. Until then it is accepted, not
  applied, and said on the hire's why and on the step in `list_workflows` ("effort <x> not applied").
- **A card can have a worktree of its own** (a branch per card, merged back into the chain's before `submit_mr`, removed with
  the chain's): `agent()`'s `isolation: "worktree"` gets one. Until then it is refused at the call, since the chain's agents
  share its one worktree and parallel writers would collide.
- **The owner wants a spawn judged**: a verdict on a child card, signing a gate of its own.
