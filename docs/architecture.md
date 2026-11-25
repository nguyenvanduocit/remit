# Remit — architecture in diagrams

Five pictures of the system as it is, each drawn from the code it names. `docs/design.md` is the text they illustrate and wins
where the two differ; why each part is built this way is in `docs/adr`. The code still reads `remit` (see `docs/design.md`).

## 1. One binary, two processes

The server is one Rust binary (`crates/remit`) that serves HTTP on TCP and the link (WebTransport, `/link`) on UDP at the
same `-addr`. Every peer on the link is one session, handed by the contract its hello asks for to the channel that serves
it. The binary starts itself again as the AI runtime, which holds every Claude Code session (ADR-0058). The bundled plugins
run in-process; a plugin the owner applies runs as a child process speaking the link on its stdio (ADR-0034, ADR-0072).
Everything it keeps is one SQLite file.

```mermaid
flowchart LR
  owner([Owner's browser<br/>client core, crates/client-web])
  person([A mentor's browser<br/>mentor's client core, crates/client-web])
  outside([Forges, Sentry, scripts])
  client([Outside agent<br/>with a client key])

  subgraph server["remit (the server)"]
    web["Web app<br/>embedded web/dist"]
    link["/link<br/>crates/link-quic"]
    rpc["wire::app<br/>crates/rpc"]
    rtlink["wire::runtime<br/>crates/runtimelink"]
    door["/hooks/&lt;name&gt;<br/>crates/door"]
    mcp["/mcp and /mcp/client<br/>agents' and client keys' tools"]
    orch["Orchestrator<br/>crates/orch"]
    bundled["Bundled plugins<br/>crates/bundled"]
    store[("SQLite<br/>crates/store")]
  end

  subgraph runtime["remit runtime (child)"]
    sessions["Claude Code sessions<br/>crates/airuntime"]
  end

  subgraph plugins["ext/ (children)"]
    plug["Applied plugins over the link on stdio<br/>crates/plugin, crates/plugin-sdk"]
  end

  forge[(GitHub / GitLab<br/>via gh, glab)]
  sources[(Outside sources<br/>StarRocks, Mongo, Datadog …)]

  owner --> web
  owner <--> link
  person <--> link
  link --> rpc
  link --> rtlink
  outside --> door
  client --> mcp
  rpc --> orch
  door --> orch
  mcp --> orch
  orch --> store
  orch <--> rtlink
  rtlink <-.dials back on the link.-> sessions
  sessions -- MCP tools --> mcp
  orch --> bundled
  orch --> plug
  orch --> forge
  orch -- the gate --> sources
  bundled --> sources
  plug --> sources
```

| Box | Where |
|---|---|
| routes `/healthz`, `/link.json`, `/mcp`, `/mcp/client`, `/hooks/<name>`, the web app | `router`, `crates/remit/src/serve.rs:239-246` |
| the link at `/link`, each session to `rpc` or `runtimelink` by its hello | `crates/link-quic/src/lib.rs:21`, `crates/remit/src/channels.rs` |
| the runtime is the server's child, keyed per start | `crates/remit/src/runtime.rs`, ADR-0058 |
| subcommands `runtime`, `emit`, `export` | `crates/remit/src/main.rs:36-54` |
| bundled plugins `amplitude`, `forge`, `github`, `gitlab`, `googleanalytics`, `intercom`, `script` | `plugins`, `crates/bundled/src/lib.rs:27-37` |
| an applied plugin as a child process | `RemotePlugin`, `crates/plugin` |

## 2. A chain's life

A chain's status is one of five (`ChainStatus`, `crates/model/src/model.rs:13-21`), changed only by the pure step in
`crates/orch/src/step.rs`. Being archived is a separate mark on a done chain (`archived_at`), not a status.

```mermaid
stateDiagram-v2
  [*] --> queued: a request, a routine or an event starts it
  queued --> running: start (playbook and title chosen, a slot is free)
  running --> waiting: the Chair's turn ends holding a check-back,<br/>or waiting on people's replies
  waiting --> running: its subscription fires, a message arrives,<br/>or the owner says check back now
  running --> paused: pause (owner or Assistant)
  waiting --> paused: pause
  paused --> running: resume
  paused --> waiting: resume while a check-back is held
  queued --> done: finish
  running --> done: finish (after the goal judge, or the owner)
  waiting --> done: finish
  paused --> done: finish
  done --> running: the owner's message reopens it
  done --> [*]: delete (withdraws what has not landed)
```

| Edge | Where |
|---|---|
| start | `ChainCmd::Start`, `crates/orch/src/step.rs:551` |
| pause, resume | `ChainCmd::Pause`, `ChainCmd::Resume`, `pause_to`, `resume_to`, `crates/orch/src/step.rs:579-604`, `:1192-1243` |
| wait at a turn's end | `turn_ended_to`, `crates/orch/src/step.rs:1007` |
| wake | `wake_to`, `end_wait`, `crates/orch/src/step.rs:1153`, `:1298` |
| finish, reopen | `ChainCmd::Finish`, `finish_to`, `reopen_to`, `crates/orch/src/step.rs:647`, `:1244-1277` |
| archive, delete only a done chain | `ChainCmd::Archive`, `ChainCmd::Delete`, `crates/orch/src/step.rs:675-720` |

## 3. Signals: every sense writes readings, subscriptions act

Whatever Remit learns, from outside or about itself, is a reading of a signal, stored in one log. Each reading is judged at once
against the subscriptions whose pattern matches its key; a subscription that fires hands its firing to the processor its action
names (ADR-0047, ADR-0049).

```mermaid
flowchart LR
  subgraph senses["Writers of readings"]
    doorIn["Door /hooks/&lt;name&gt;<br/>and remit emit"]
    poll["Sensor kind poll<br/>reads a source"]
    script["Sensor kind script<br/>runs a chain's script"]
    pluginSense["Plugin sensor kinds<br/>Sense or Poll"]
    facts["Facts Remit<br/>records about itself"]
  end

  emit["ingest<br/>crates/orch/src/ingest.rs"]
  log[("Readings log")]
  judge["signal::evaluate<br/>pure, crates/signal"]
  subs[("Subscriptions<br/>pattern + CEL when")]

  subgraph processors["Processors, by action"]
    agentActs["start · wake · tell · hire<br/>(an agent acts)"]
    codeActs["run · end · deadline<br/>(code acts)"]
    pluginProc["plugin:&lt;id&gt;.&lt;name&gt;"]
  end

  doorIn --> emit
  poll --> emit
  script --> emit
  pluginSense --> emit
  facts --> emit
  emit --> log
  emit --> judge
  subs --> judge
  judge -- firings --> agentActs
  judge -- firings --> codeActs
  judge -- firings --> pluginProc
```

| Box | Where |
|---|---|
| built-in sensor kinds poll and script | `KIND_POLL`, `KIND_SCRIPT`, `crates/model/src/sensor.rs:17-18`; `crates/orch/src/sensor.rs` |
| a plugin's kind is sensed by its plugin | `crates/orch/src/plugin_pipeline.rs` |
| `ingest` stores a reading, the one writer of the log | `crates/orch/src/ingest.rs:62` |
| `tick_signals` judges matching subscriptions | `crates/orch/src/signals.rs:199` |
| the pure judgement | `evaluate`, `crates/signal/src/evaluate.rs:100` |
| processors by action, plugin processors | `processor_of`, `crates/orch/src/processing.rs:120` |

## 4. From an agent's change to users

No change lands unchecked: the gates its chain names (a reviewer and a QC, the QC alone for a chore, and security on a
rail) approve the exact patch id before the version leaves `judging` (`crates/shipping/src/gate.rs:1-6`). Remit lands every
approved change itself: no agent merges and there is no merge request (ADR-0114). The merge queue of each repository and
base combines a batch onto the base, checks the combined tree in the jail and pushes it, never forced; the forge's
pipeline on the pushed commit is read after, and a breach is reverted by code. What landed is released by the
repository's own `.remit/release/`, run by code with no turn (ADR-0116), and a landing makes the goal judge look again
(ADR-0039).

```mermaid
sequenceDiagram
  autonumber
  participant Eng as Engineer card (worktree)
  participant Gate as Reviewer + QC cards
  participant Chair
  participant Core as Orchestrator
  participant Forge as GitHub / GitLab
  participant Judge as Goal judge card

  Eng->>Chair: report: change ready
  Chair->>Gate: hire with the engineer as input
  Chair->>Core: land
  Core->>Core: commit, run language and .remit/checks in a jail, record the landing (judging)
  Gate->>Core: record verdicts on the patch id
  Core->>Core: every required gate approved: ready in its repository's queue
  Core->>Core: combine a batch onto the base, check the combined tree in the jail
  Core->>Forge: push the batch to the base (never forced)
  Core-->>Chair: [land result]: landed, or returned with why
  Core->>Forge: the forge plugin's landing sense reads the pipeline on the push
  alt the push broke a green base, or a stage's guardrail breached
    Core->>Forge: revert at the head of the queue, by code
  else the checks stand
    Core->>Core: release by the repository's .remit/release/ (steps in the jail, bake, live or taken back)
  end
  Chair->>Core: finish_chain
  Core->>Judge: hire the goal judge
  Judge->>Core: verdict per key result
  Core-->>Chair: finished, or what is left
```

| Step | Where |
|---|---|
| verdicts tied to the patch id, `MAX_GATE_ROUNDS` | `crates/shipping/src/gate.rs:1-39` |
| `land`: commit, checks, the landing recorded `judging`; `approve` makes it `ready` | `crates/shipping/src/land.rs:225`, `:575` |
| repository checks in a jail | `crates/shipping/src/repochecks.rs`, ADR-0025 |
| the queue: batch (`MAX_BATCH`), combine, check, push through the `Lander` port | `crates/shipping/src/landing.rs:15`, `:42`, `crates/shipping/src/queue.rs:82`, `crates/shipping/src/direct_push.rs:20` |
| the forge's checks on a push and the revert of a breach | `verdict_on`, `crates/shipping/src/landing.rs:98`; `sync_landings`, `crates/shipping/src/landing_checks.rs:90`, every `LANDING_SYNC_EVERY` (`crates/pipeline/src/automation.rs:71`); `crates/shipping/src/revert.rs:31` |
| stages of a canary | `crates/shipping/src/stage.rs`, `crates/rollout` |
| competing attempts and `pick_attempt` | `crates/shipping/src/round.rs:1-6`, `:369`, ADR-0115 |
| the release: the procedure's pure core, its runner, the install on this server | `crates/shipping/src/release.rs`, `sync_releases` and `revert_release` in `crates/shipping/src/release_run.rs:206`, `:821`, `crates/shipping/src/install.rs:91` |
| the goal check, and its recheck on `fact:chain.landed` | `crates/shipping/src/goal.rs`, `crates/shipping/src/recheck.rs`, `sub_hire_goal_recheck` (`crates/pipeline/src/automation.rs:148`) |

## 5. Crates

Which crate depends on which inside the workspace, by group, with the edges that show its layers (`cargo metadata`; dev
dependencies and the build tools `tsgen` and `claudehooks` left out). `orch` is the application layer; the
channels sit above it, and the pure cores (`signal`, `grant`, `rollout`, `outcomes`, `library`) depend on nothing but
`model` and `skills`. The client cores (`replica`, `client-web`, `remit-client`) reach the server only through `link` and
`wire`.

```mermaid
flowchart TB
  main["remit"]

  subgraph channels["Channels"]
    rpc["rpc"]
    door["door"]
    clientmcp["clientmcp"]
  end

  subgraph linkLayer["The link"]
    linkquic["link-quic"]
    wire["wire"]
    link["link"]
  end

  orch["orch"]

  subgraph runtimeSide["AI runtime"]
    runtimelink["runtimelink"]
    airuntime["airuntime"]
    agent["agent"]
    claudecode["claudecode"]
  end

  subgraph pure["Pure cores"]
    signal["signal"]
    grant["grant"]
    rollout["rollout"]
    outcomes["outcomes"]
    library["library + skills"]
    workflow["workflow"]
  end

  subgraph edges["Outside the process"]
    source["source"]
    plugin["plugin + plugin-sdk + bundled"]
    forge["forge"]
    jail["jail + egress + procgroup"]
    openrouter["openrouter"]
  end

  subgraph clients["Client cores"]
    replica["replica"]
    clientweb["client-web"]
    remitclient["remit-client"]
  end

  store["store + secrets"]
  model["model"]

  main --> channels
  main --> orch
  main --> runtimelink
  main --> airuntime
  main --> linkquic
  channels --> orch
  rpc --> linkquic
  orch --> agent
  orch --> pure
  orch --> edges
  orch --> store
  orch --> wire
  runtimelink --> agent
  runtimelink --> linkquic
  airuntime --> runtimelink
  agent --> claudecode
  claudecode --> jail
  source --> grant
  plugin --> wire
  linkquic --> link
  wire --> link
  clientweb --> replica
  replica --> wire
  remitclient --> linkquic
  store --> model
  pure --> model
```

| Edge | Where |
|---|---|
| every dependency above | `cargo metadata --format-version 1 --no-deps` (each package's `dependencies`) |
