# Remit: a small closed core, everything else a plugin

Remit is the evolution of Remit: the same codebase carried into the new design. The workspace, the binary and every code
name still read `remit` until they are renamed (`docs/superpowers/specs/2026-10-27-rename-design.md`). This
document is the direction of the product: the principles, the decisions the owner has made, and the work still to build.
`docs/design.md` says how the system is now; each decision's reasons are in `docs/adr`. The evidence behind the method is
`docs/archive/ai-run-company-blueprint.md` (invariants I1-I19, day-0 checks D1-D18, the known holes of section 7.2) and the
`docs/archive/research-*.md` files; the reviews of this design are `docs/archive/council/design-review-*.md`.

**Scope today (owner, 2026-11-02):** one person plus one more team, no customers. Everything that serves outside customers
(release chain, licensing, HSM signing, support, vendor liability) is **HOLD**: it is designed here and built when the first
customer is near.

**Labels.** **Built** = running code with tests. **Planned** = decided, not coded. **HOLD** = waits for a customer. **HIGH** = a
line of code read and cross-checked, **EXP** = a reader reported the `file:line` and nobody cross-checked it, **Proposal** = a
design of this document, not run. Not legal advice.

## 0. Principles

The owner's stance: in future everyone codes for themselves, but nobody builds a whole system alone; they add a forge or a
data source for signals. So **the core stays small and stable, and everything else is an extension that users, or their
agents, write without forking the engine.** This matches `AGENTS.md`: the owner says intent, agents act, an extension is
something an agent drafts and the owner applies.

1. **A change is the smallest one that fixes the root cause.** No new entity where an existing record does the job; one owner
   per concept.
2. **The platform sets the effect class; an extension only proposes.** A manifest that calls a write endpoint a read would
   pass the owner's Ask (blueprint I1), so the platform's table sets the class and the owner approves it.
3. **An extension is trusted code the owner applies**, drafted by the owner's own agent or supplied by the customer. There is
   no open marketplace. The controls that would confine an extension are seams today (section 3, K5).
4. **Only a rail or an entrance needs a full threat model** (section 6); the rest follows an existing pattern.
5. **Make it work first, then make it right, and leave room for the right** (`AGENTS.md`): each seam is named in its ADR under
   *Revisit when*.

**Extension points.** "Any system, any company" means these ten points, each a data record or an outside program and none a
branch per company.

| # | Point | A new company or system is | Status |
|---|---|---|---|
| 1 | Tenant | one **cell**: a process, a data directory, an OS user (`-data` gathers every path) | the cell is implicit; the allocator is Planned (K3) |
| 2 | Tool source | an **extension**: a `hashicorp/go-plugin` process with a manifest | Built (K5, ADR-0034) |
| 3 | Event source | a program that POSTs to a signal's door or is read on a schedule by a sensor | Built (ADR-0024, ADR-0030, ADR-0047, `poll`) |
| 4 | Kind of result | none: all work is files in Git and the result is a landing on the base branch | Built (K6, ADR-0114) |
| 5 | Forge | an outside adapter | the forge hosts the code: Remit pushes the base and reads a landed commit's checks through the `Forge` service the host lends a plugin's sensor (ADR-0114, Built); an outside adapter is three reads and a push, Planned when a third forge is in use |
| 6 | Model | Claude Code is the one harness, behind `agent::Launcher`; a backend per cell | decided (K7, ADR-0006); Planned |
| 7 | Law | retention, export, delete, AI notice | export Built; the rest Planned (K9) |
| 8 | Operating system | any host where `claude` runs; every child but a session starts through one runner (`procgroup::Program`) | Built: Linux and macOS (ADR-0138) |
| 9 | Sensor kind | a plugin that declares a kind in `Describe` and senses with `Sense` (a stream of readings) or `Poll` | Built (ADR-0049) |
| 10 | Processor | a plugin that declares a processor in `Describe` and runs it with `Process` on a subscription of Remit | Built (ADR-0049) |

**Honest limits.** Two places miss "any": a bank on Kubernetes (there is no
container image, and Remit runs no isolation between its agents and the host, ADR-0138) and sign-in (the owner has one owner key and, when set up, his Google account, no SSO and no two-person approval; a mentor works from
codes he has made for them, ADR-0060, ADR-0062). A system behind a VPN is solved by the
distribution model: the customer runs the cell inside their network. Irreversible effects stay unbounded (blueprint 3.8).

### Distribution (HOLD)

The owner decided: Remit is closed source and ships as a compiled binary; the customer runs it on their own systems and
builds their own plugins; there is no cloud of ours; updates apply automatically; no compatibility is promised between
versions ("upgrading may break it"). It follows that:

- **Each installation is a cell.** There is no control plane, no central service and no customer data returning to us (no
  phone-home, no telemetry; the only contact is the update check). Isolation between business units of one customer is that
  customer's matter, by running several cells.
- **Plugins are outside programs** because that is the only way a customer builds without the engine's source. The protocol
  between engine and plugin is therefore a public API with no compatibility promise: each release may change it
  (`sdk.ProtocolVersion` holds the place).
- **The customer supplies model credentials** (Claude, Bedrock, Vertex, an internal gateway), so their data never touches our
  infrastructure and a closed network works. Three duties come back to us: packaging and signing releases (and protecting the
  signing key, since auto-update makes it the one concentration of risk), keeping an update from silently breaking plugins,
  and supporting a customer we cannot see.
- **Our legal role is software vendor**, not data processor. Software is a "product" under EU Directive 2024/2853 (for
  products placed on the market after 2026-12-09), and the maker answers for updates and missing updates within its control
  (blueprint research `[R §1.2]`, MEDIUM). That is a question for our own counsel.

## 1. Where the system stands

**Held and built on** (HIGH unless marked): one writer per chain; `act` runs once per request id (`crates/orch/src/request.rs`);
`model.Change` records before and after in one transaction with `ApprovedBy`; one contract, `crates/wire`; migrations
carry a checksum; `grant::decide` is pure (`crates/grant/src/lib.rs`); every child but a session starts through one runner, `procgroup::Program`, the seam where isolation returns (ADR-0138); a source's
credential is typed by the owner into a write-only field and never enters an agent's context (ADR-0017); every Claude Code
session runs in the AI runtime, the server's child, behind the one session port `agent::Launcher` (ADR-0058); a mentor works in the
web app over `wire::app`, scoped to the cards they help, with a code the owner has made for them (ADR-0060, ADR-0062,
ADR-0089).

**Open gaps, each confirmed in code:**
- The gate (`pass`, `attempt`, `permit` in `crates/source/src/gate.rs`) covers source tools, `run`, `script` and sensors; it does
  not know a stop, and a refusal is `NoGrant` or `Rail` only (`Refusal`, `crates/grant/src/lib.rs:104-109`); `Stopped` and `Law` are not
  there.
- Remit runs no isolation (ADR-0138): a session, a source's program, a repository's checks and a release's steps run as
  the user, with the user's files and network. The session and the server share one uid, so a session can read what the
  server reads, the key file and the database included; the ask and deny rules (`crates/chains/src/policy.rs`) are a
  tripwire. This holds while one person runs Remit on their own machine.
- `crates/egress` listens on a loopback port a program reaches through its proxy variables, and a program can go around
  it; its allow-list has no wildcard and it cannot see inside a CONNECT tunnel
  (`crates/egress/src/egress.rs`): it stops a destination, not what is said to an allowed one.
- `events` has no retention: settled events are packed (`crates/store/src/pack.rs`), never expired; a daily `VACUUM INTO` snapshot exists (`crates/store/src/snapshot.rs`) and
  no restore code does (K1).
- `read_change` asks the writer to retype `before` by hand: there is no revert.
- A pause closes only the sessions that have background work (`crates/orch/src/apply.rs`); a `setsid` child survives.
- Names of signals, keys, skills and connections are unique across the whole installation, and configuration lives in flags.

**Build note.** `make test` needs GNU make 4 (the Makefile uses `--output-sync`); on macOS use `gmake`
(`/opt/homebrew/bin/gmake`) or `test-go` and `test-web`.

## 2. How the decisions below are carried

Each decision becomes an ADR (`docs/adr/NNNN-title.md`) when it is built. A number is written here only after its
file exists.

| A need | Reuses | Adds no |
|---|---|---|
| Stop | the `Paused`, `ChainPaused` and `CardStopped` states plus one pause record per cell | `stops` table |
| The owner's signature | applying a change item (`Change.ApprovedBy`) plus the hash of the signed content | `Signature` entity |
| Release | `vcs.revision`, which `/healthz` reports, recorded in each session at its start | `Release` entity |
| A refusal's reason code | the closed set on `grant.Refusal` | a second `Reason` |
| Cell configuration | flags are the root; a cell has its own data directory | `settings` table |
| Ledger of calls | the `source_calls` table | `gate_calls` |
| Effect class | the ordered `Effect` ladder on a tool | a second `Class` |

## 3. Decisions

### K1. Recovery: the owner skips backup and restore of orchestration data
Losing the SQLite file loses orchestration state (chains, decisions, connections, signals). The loss is bounded because the work
lives in Git on the remote (K6): what is lost is coordination context. The daily `VACUUM INTO` snapshot keeps five copies and
restoring by copying a file back works by hand. So there is no `/metrics`, no restore drill, and a downgrade (K11) is manual. Stop
(K2) and credentials keep their full recovery requirement.

### K2. Two OS users, a real stop, egress that records and counts (Planned)
- **Two users, two jobs.** The **cell's** user keeps one company from another (K3). The **session's** user differs from the
  server's, because the gap of ADR-0004 is that agent and server share a user; only the session's user closes it. Creating users
  needs root; the allocator (K3) does it.
- **A real stop.** One pause record for the cell, plus the existing pause states; `decide` reads them through a `StopView`
  (a function in `crates/orch/src/grants.rs`). `pause` closes every session in scope, revokes the MCP token and closes the
  egress path; with a session user, `kill` by user also clears a `setsid` child. On Linux a session gets `CLONE_NEWPID` and a
  fresh `/proc`.
- **Egress that records.** A new mode in `crates/egress`: allow every destination, record and count every connection. It is
  the measuring point and the cut point of the stop button. It does not read content (CONNECT is opaque bytes); blocking content
  belongs to the tool gate. Making it the only way out needs isolation back at the runner and around sessions (ADR-0138,
  *Revisit when*); sessions reach Remit's MCP over loopback, so that isolation must leave them that port.
- **A stop drill.** The server commands a stop and measures the time until the last byte leaves the proxy and the last process of
  the scope is gone; the owner alone triggers it; the result shows on the scorecard.
- **Still wrong afterwards:** macOS has no pid namespace, so a `setsid` child dies only by `kill` by user; on both OSes the model
  login file stays with the session.

### K3. Cell and members
- **Cell (Proposal, MEDIUM):** each installation, and each business unit a customer wants separate, is one cell: one process,
  one data directory (`-data`, `crates/remit/src/flags.rs`), no multi-tenancy in a process (a wrong `org_id` filter leaks data
  between companies, and an OS boundary makes F1 true by structure).
- **The allocator is a tool the customer runs** (Planned): a crate `tenant` (pure logic: names, ports, paths,
  launchd/systemd unit templates) and `remit tenant create|list|stop|upgrade|export|delete`. The customer upgrades by looping
  over their cells (`remit update` keeps `current` and `previous`).
- **One owner, no roles (Built, ADR-0026, ADR-0105):** the owner is let in by the owner key, or by Google through the server's Firebase project when it has one (ADR-0105), the key a file the
  ask rules name (a tripwire: a session runs as the same OS user and can read it, ADR-0138), handed over by the link the server prints (ADR-0022); one owner per server. Network protection is the
  deployer's job. The three alternatives were an unprotected port (sessions would approve their own grants and apply their own
  changes), an OS user per session with a uid firewall (K2, larger), and nothing. A mentor is not a user of the
  app: the owner has the Assistant make them a code per device, and that worker key reaches only the cards they help, in
  the web app (ADR-0060, ADR-0062, ADR-0089).
- **Cell configuration is flags.** No company name lives in code; built-in playbooks are written generally; a product's
  context (its customer, the owner's notes, its repositories and connections) is a project the owner keeps through the
  Assistant, and one cell works on several products at once (ADR-0108). A project is context, never a tenant: what must be
  kept apart is a cell. The owner's secret directories are `-secret-dirs`.
- **Not planned:** SSO for the owner and two-person approval, a container image, a remote executor.

### K4. The gate: wide coverage, an effect ladder, refusals with a reason
- **`grant::decide` keeps its pure signature.** A stop and a law combine with it in `crates/orch/src/grants.rs`, where
  `decide_as` lives. `Form` is `source`'s type and does not enter `grant`.
- **Effect ladder (Built, ADR-0027):** `Read < Reversible < Compensable < Irreversible`, declared per tool, **set by the
  platform's table**; the Chair does not grade itself. Data class and parameter origin (`outside`) are tracked per card.
- **Refusal (Built in part):** `grant::Refusal` is `NoGrant` or `Rail`; `Reason` is kept. `Stopped` joins with K2 and `Law`
  with K9. The blueprint's four codes are `STOPPED`, `UNLAWFUL`, `RAIL`, `PLATFORM_RECORD`; `NoGrant` is "least grant" of ADR-0018
  (still open, section 8).
- **Wide coverage (Built, ADR-0028):** Bash, WebFetch and WebSearch are capabilities `shell` and `web` that a chain's kind starts it with or its
  Chair gives it with `grant_capability` (a capability that writes it asks the owner for with `request_capability`); a chain without them has the tools denied. Repository checks and forge commands run as child programs of the server, with no isolation (ADR-0025, ADR-0138).
- **Ledger (Built):** the `source_calls` table (migration 53) records the call with its argv. The log files (`remit.log`,
  `agents.log`) are readable by a session (ADR-0138). A hash chain is deferred and named as debt.
- **The seal set** (human anchors, seeded-fault labels, canary positions) is **outside the cell's database**, where only the owner
  and the measuring job read; an agent with Bash under the server's user would read it with `secret.key` otherwise (Planned).
- **Control separate from data:** a role that reads outside content runs as an isolated reader with no tool but its reads, its output labelled with its source.

### K5. Extensions are plugin processes the customer or a chain supplies and the owner applies (Built, ADR-0034)
- **Anything-Plugin.** Tool sources, event sources, judges, agents and screens each become a plugin over time, and Remit changes
  itself through its own plugins. A plugin the owner applies is a child process speaking the link on its stdin and stdout
  (`crates/plugin`, `RemotePlugin`); a plugin that ships with Remit implements the same `plugin_sdk::Plugin` trait and runs
  in-process (`crates/bundled`). Rust authors use `crates/plugin-sdk` (`Definition`, `serve`).
- **Contract:** `wire::plugin` (`crates/wire/src/plugin.rs`), the methods of `plugin_sdk::Plugin`: `describe` (label, capability, credential variables, tools with schema and effect
  class, whether it polls, its interface, the sensor kinds, processors and subscriptions it ships), `call`, `poll`, `sense`, `process`, `asset`, `account`. It is a public API with no compatibility promise.
- **Process:** `plugin::RemotePlugin` runs one process per id, pins it by the SHA-256 of the file (checked before exec), passes
  no server environment, and starts it again at the next call after it dies.
- **Shipments (ADR-0037):** `describe` also carries the event signals (filled by the plugin's own poll) and the agent templates a plugin ships. Only a plugin that
  ships with Remit may ship them: `seed_event_signals` makes each signal, paused, and `seed_agents` each template, with what the code ships. Intercom is
  the first source that is one plugin whole: tools that read and write, the poll, the signal `intercom` and the agent `intercom-support`. `gitlab` and `github` ship their event signal and its rules as subscriptions (ADR-0049).
- **Gate:** a plugin's tool is an ordinary source tool: the manifest's effect class places it on the ladder, the chain's grant on
  the connection decides, every call is in the ledger, output is stripped of credentials. The credential travels with each call
  (`vars`), so a plugin holds no connection state.
- **Manifest** is a Change-managed record (`extensions`) copied from `describe` at proposal time with the pinned hash. An API behind
  a credential-injecting proxy (ADR-0029) is a manifest form (`auth`) with no program.
- **Authoring path:** the owner says "connect X"; a chain with the `extensions` capability calls `propose_extension` with the path of
  a built binary; the platform runs `describe` and posts a change item (hash, tools, variables, interface); on the owner's Apply
  the binary is copied to `ext/<id>/<sha12>-<file>`, its hash checked again, and the source exists. `remove_extension` is the way
  back. The customer supplies a binary by the same path; a chain writing source for the platform to build is the second path
  (Planned).
- **MCP servers (decided, ADR-0077; Planned):** an outside MCP server is an extension of form `mcp`, Remit its only client: stdio
  through the runner with the connection's sealed variables, or HTTP with the credential in the backend. Each tool is pinned at first
  connect (description, schema hash, effect: read when `readOnlyHint`, else irreversible) and served as `<source>_<tool>` through
  the gate; a tool that drifts is held back until a change item is applied. "Bring in my MCP servers" goes to the Assistant
  (`list_importable_mcp_servers`, `propose_mcp_server`); OAuth sign-in follows as a `login` item. A session still holds no MCP
  server but Remit's.
- **Context providers (decided, ADR-0112; Planned):** a plugin lists `contexts` in its manifest and answers one read,
  `context`. After `take` hands a card its message, the core asks every provider of the chain's capabilities (through the
  gate, as a `read`) at once under one 2-second deadline, gives the agent the blocks framed as data before the message,
  and records them as the card's `context_given` event, so the stream shows what the agent read; a provider that fails or
  is late is left out. Replies are never rewritten: what happens after a turn is `fact:turn.ended` and its processors. Remit's
  own recall for a turn (ADR-0117) moves onto this path.
- **Interface (Planned):** a plugin's `ui` is a Module Federation remote embedded in the plugin file, same origin, sharing the app's
  one Vue, mounted in slots the manifest lists. The web loader is the next change.
- **Services for plugins (seam):** a plugin reaches AI, threads and data through the application layer with the plugin as the
  actor, over the plugin's link; the manifest gains `capabilities` and one policy function. The controls that would confine
  a plugin are listed in ADR-0034 under *Revisit when* and built when the owner asks.
- **Built: a plugin reads a forge through a service the host lends** (ADR-0057, ADR-0114): the `Forge` port (`checks`),
  lent to the bundled `forge` plugin's `landing` sense for the commits its sensor names, Remit's landings
  (`crates/reach/src/forge_service.rs`). The push and the forge's login stay in core.
- **Deferred:** forges as outside processes. A forge is who Remit works as, a branch's head, a commit's checks
  and the push (the rails in `crates/chains/src/policy.rs` know only `glab` and `gh`, and `forbidden_path` only the CI of those
  two). It waits for a team that needs a third forge. ADR-0017 is narrowed by ADR-0029 and ADR-0034.

### K6. Git is the base of all work, and Remit lands it (Built, ADR-0114)
Whatever Remit does, the result lives in Git. A user without a remote or a Git system must get one. There is no `Result` entity
and no merge request: every kind of work ends as a **landing**, one commit on the repository's base branch that Remit's queue
made, checked on the combined tree and pushed (`model::Landing`, ADR-0114). The board, the archive lifecycle, the readings
`chain:<id>.landing.<repository>`, a webhook's event matching, admission's back-pressure, the Outcomes KPIs and the goal
judge's recheck on `fact:chain.landed` read the landing.
- **Entry condition:** each cell needs at least one remote on a supported forge (GitLab and GitHub through `-forges`,
  `crates/forge`), and the account Remit pushes as may push to each base it lands on (the forge's protection gives it a bypass
  or an allowed role). Without a forge no chain opens a repository (`OpenRepo` needs a clone). The install guide states both.
- **How a change lands:** the Chair submits a version (`land`); its gates (the kind's, security on a rail) judge its patch id;
  approved, it enters its repository's queue by itself; the queue combines a batch onto the current base, one commit each,
  runs the checks on the combined tree, bisects a failure, returns a conflict to its Chair, and fast-forwards the
  base. The forge's CI on the pushed commit and a stage's guardrail are read after; a breach is reverted by code. The owner
  reads the landing and its diff in Remit and overturns by telling an agent (`revert_landing`).
- **What Remit takes from concord** (measured 2026-11-18): the combined-result gate (the
  only layer that caught every semantic bug), a base whose history only grows and moves like a pull, the empty-change check,
  and the notice to the others whose work overlaps (`fact:repo.landed`). Not taken: jj, its board beside Remit's store, the
  function claims behind a barrier, and Mergiraf (27 right, 26 wrong of 159 ConflictBench merges).
- **Non-software work becomes files in the repo:** plans, content, configuration, number sheets, decision records, lists of
  operations to run. Approval is the verdicts on the version and the combined checks; the owner reads the result and
  overturns.
- **Effects on outside systems** (a refund, a budget change, a mail): two paths, chosen per kind of work. (a) **GitOps
  (Proposal):** the change is a file describing the calls, landing approves it, the landing's fact (`fact:chain.landed`)
  triggers an executor that makes the calls through the gate: the first real use of `EffectWrite`. (b) **Git as the record,
  not the approval:** real-time work (answering a customer) runs through the gate at once and only commits a log afterwards.
  Forcing every action through a landing is not feasible and conflicts with the stance of no approval gate on agents'
  initiative.
- **Built: a repository declares its checks** (ADR-0025): executables in `.remit/checks`, run as child programs with a deadline,
  after the language's checks, failing closed. A repository in a language Remit has no check for has deterministic checks that
  it names itself, and each domain brings its own (schema check, recomputed numbers, doc lint). The queue runs them on the
  combined tree.
- **A forge's merge request** stays possible behind the `Lander` port (a `ForgeMerge` adapter, not built) for a team whose
  forge must merge.
- **Forge is the one mandatory integration.** A company on Bitbucket, Gitea or Azure DevOps cannot use Remit until an outside forge
  exists (K5, deferred).
- **Limits of Git as the base:** large binaries and data (LFS or references), work that needs second-level latency (path b), and
  secrets (`forbidden_path` blocks the secret files, `crates/shipping/src/land.rs`).

### K7. Model (decided, ADR-0006; Planned)
- **One harness:** every agent, judges included, runs in Claude Code through `claudecode`, behind `agent::Launcher` in the AI
  runtime (ADR-0058). A second harness would be a runtime the core starts behind the same port, not a change to the orchestrator.
- **Judges are sessions:** they read the worktree, run checks and call Remit's tools. A judge **records an `Approval`** as the
  present reviewer does. A panel not measured as independent counts as one vote (blueprint I9); the by-model table of the analyst
  is where independence is measured.
- **Backend per cell:** a Claude account of a plugin sets a gateway, an API key, Bedrock or Vertex (ADR-0071); region and model
  pinned per cell remain Planned, and `childenv` strips the Bedrock variables of the server's own environment today (EXP).
- ADR-0010 is triggered by "runs for someone other than its owner": cost is read by the adopter from their own invoice, outside the
  system.

### K8. Verification (Planned, one part Built)
The closed `Finding.Blocker` set grows into reason codes (checked in `findings_error`, `crates/orch/src/gate.rs`) and an
`EvidenceRef` the platform issues. A gate's rounds are capped by `MAX_GATE_ROUNDS` (`crates/orch/src/gate.rs`) and at the cap become
an `InboxQuestion` (**Built**). The scorecard gains rows in `kpi_groups` of `crates/outcomes/src/kpi.rs` (M3 reopened after merge,
M5 p95, M7 from the stop drill). Seeded faults are **Built** (ADR-0102): a trial chain carries one fault its Chair plants, and the
report shows the share each of reviewer, QC and goal judge caught. **Deferred until the first numbers exist to measure:** human
anchors, pass^k replay (a `runs` store), the hash chain. These are named debt.

### K9. Minimal law (partly Built)
`remit export` hands the owner their data (**Built**). Planned: retention by class (`events` has none today), delete
per cell (`DeleteHome` is the model), an "this is AI" notice inserted by code into outgoing tools, a legal record the `officer`
signs. A signature is **applying a change item plus the hash of the signed content**, never a chat message alone, so a steered
agent cannot control what is signed. The full legal pack by region needs counsel; the most important missing content is the class
"significant decision about a person" (GDPR Art. 22), employee data for works councils, and the Vietnamese data-transfer file.

### K10. The owner's surface (Planned in part)
The exception queue sorts `attention_of` (`crates/orch/src/standing.rs`) by effect ladder then age, removes duplicates, keeps the
"Waiting for you" column and ADR-0019's counting rule (no new tab). A signature shows in the Activity of the agent that asked. The
stop-drill result and the scorecard show in Results. Each screen has a `web/verify/` page that is looked at, measured with
`getBoundingClientRect` and tabbed through. No form for configuration. The exception queue has a per-chain cap to protect the owner
(not a limit on what agents do).

### K11. Distribution: closed, auto-updating, no compatibility (decided; HOLD)
- **What ships:** a binary per OS and architecture, **signed** (macOS signed and notarized; Linux detached signature), with a
  checksum, an SBOM and release notes. `make build` embeds the web app and `remit update` installs to
  `~/.remit/releases/<tag>` with `current` and `previous`; that mechanism is the base for updates.
- **Install:** an installer the customer runs once as root to create the two OS users (K2) and the launchd/systemd unit. A container
  image for Kubernetes customers is its own item.
- **Auto-update (default on):** a cell checks a signed manifest on a static server of ours, downloads the new build beside the
  running one, **self-tests before activating**, then switches `current`. It is the cell's one contact with us and reveals IP and
  version; no cell identifier is sent and the static server keeps no access log (our commitment). The owner can postpone an update
  for at most N days (N and the security-patch exception are not set).
- **The largest concentrated risk (Proposal, MEDIUM):** closed source plus auto-update means we can run code on the agents of every
  customer, where their credentials and real actions are. A customer cannot check a binary, so all trust rests on our signing
  chain; a leaked key or captured release process (the SolarWinds and 3CX pattern) lets an attacker steer every customer's agents at
  once. Minimum: the key in an HSM and two-person signing; reproducible builds with published hashes; a cell verifies the signature
  and **refuses a lower version**; a transparency log. This needs its own threat model before the first customer's auto-update.
- **No compatibility promise.** The engine accepts only its own version's plugin protocol (no N-1). Migrations go forward only;
  downgrading is a manual restore of a snapshot and **loses everything after the update** (K1 holds the snapshot).
- **"Upgrading may break it" never becomes "breaks silently".** Four cheap rules: (a) an automatic snapshot before each update;
  (b) the new build runs a health check and `ext test` on installed plugins **before** `current` switches, and a broken plugin is
  disabled with a message in the owner's inbox naming the reason (the update still activates); (c) activate only when the cell is
  quiet (no chain running) or in a window the owner picks; (d) roll back to `previous` if health fails after activation.
- **Closed source means** the `self-development` playbook (ADR-0012) is ours alone; a customer extends through plugins, so the
  plugin protocol is published in full.
- **Support without sight:** no telemetry; `remit diagnose` writes a secret-stripped bundle the customer sends on their own;
  a disclosure policy and a security report channel.
- **Legal (counsel):** because we control updates, the maker answers for updates and missing updates under Directive 2024/2853
  (Art. 11(2), 4(5)(b), `[R §1.2]`, MEDIUM); closed source plus auto-update enlarges that share.

### K12. Subagents are employees at work, and workflows are scripts Remit replays (Planned, ADR-0073)
A Claude Code session is offered its standing employees as subagent types; a spawn is a child card of that employee, bound to its
parent's process. A workflow is one entity with an engine: `claude` (Claude Code's Workflow tool, inside a worker's turn) or
`remit` (the Chair's `run_workflow` and a routine's script, evaluated by a pure `Advance` replayed from its journal, so it waits for
days and survives restarts). The employee lease goes. Each
unit updates `docs/design.md` (How a chain runs, Employees, Routines) as it lands.

### K13. No agent waits on the owner, and Remit releases what lands (Built, ADR-0087, ADR-0116)
What stays the owner's are the secret acts (a credential, a sign-in code, seeing a key's value, a setting outside Remit) and
overturning. Every other item that would wait for him is routed to the Assistant (woken by `fact:owner.asked`), decided and
recorded; the inbox shows what agents did, with Overturn. No request to land or release exists, so none can be declined on
taste. Every repository releases what lands by its own procedure (ADR-0116): `.remit/release/` holds steps Remit runs in
code on the landed commit, reaching with a credential only the connections it names, through the injecting proxy (the credential
never in the procedure), under grants its repository holds and the Assistant decides; an optional `guard` and `back`. A
release runs after a landing's forge checks pass, one at a time per repository; a breach runs `back`, reverts its landings
(ADR-0114) and releases forward, with no turn. For Remit's own repository the procedure waits for the workflow's release of
the commit, and installing on this server (`install_release`, a detached `remit update`, guarded by `fact:remit.health`,
`previous` or forward for a release that migrates) is its last step. A change touching `.remit/rails` or
`.remit/release/` needs a `security` verdict. Running a backfill on a shared store waits for a source that writes.

### K14. Key results are the unit of scheduling, and capacity is measured (Built, ADR-0115)
No count paces work. A limit read from pressure (the accounts' warned windows and rate limits, the host's memory and load)
grows by one a Tick and halves under pressure, and is shared by key result: every served key result runs one chain before any
runs two, then the soonest horizon. A key result's share grows with each landing that holds and halves with a revert or a
return, and it takes no new start while its landings waiting in a queue fill its share, so work is released at the pace it
lands. Key results refine into key results (`parent`, `refine_key_result`; the owner's stay the top, the agents' are
recorded and overturnable); `propose_chain` deduplicates by the key result served; a key result with a `command` check may
run competing attempts a fresh judge picks among (`pick_attempt`). Runaway work is a key result whose last chains made no
progress; no count limits wakes, watches or proposals, and the entrance counts (client keys, events a day) stay.

## 4. Data

| Record | Content | Status |
|---|---|---|
| `source_calls` | the platform's ledger of calls (K4) | Built (migration 53) |
| `extensions` | extension manifests, managed by Change (K5) | Built (migration 60) |
| `Finding` fields | a reason code and `EvidenceRef` (K8; findings live in the approvals' JSON) | Planned |
| a pause record | one per cell (K2) | Planned |
| `projects` | one product's durable context; `Objective.project` names it (ADR-0108) | Planned (migration 0111) |
| `landings` | each version a chain lands, its batch, commit, checks and revert (ADR-0114) | Built (migrations 0119, 0123) |
| `releases`, `release_pauses` | each run of a repository's release procedure, and a repository's release pause (ADR-0116) | Built (migrations 0119, 0124) |
| `key_results` | key results out of `Objective`, with `parent` (ADR-0115) | Built (migration 0121) |

## 5. Interface (sketch, matching the real signatures)

```rust
// grant: the pure signature stays.
pub fn decide(grants: &[Grant], a: &Action) -> Decision
// crates/orch/src/grants.rs: stop and law combined around decide, beside decide_as.
fn decide_with(form: Form, capabilities: &[String], c: &ConnectionRef, stored: &[Grant], e: Effect, stop: &StopView, law: &LawView) -> Decision

pub enum Effect { Read, Reversible, Compensable, Irreversible } // set by the platform's table
pub enum Refusal { NoGrant, Rail }                               // Stopped, Law join with K2 and K9; Decision keeps reason
```

The extension protocol is `wire::plugin` (`crates/wire/src/plugin.rs`, K5).

## 6. Threat model (STRIDE) for the rails and entrances this design touches

| Part | Threat | Control |
|---|---|---|
| Egress | an opaque CONNECT proxy: HTTP/2 to an allowed host passes; the proxy is a single point *(T/I)* | state the limit; block content at the tool gate; a cell pause closes the proxy; measure traffic |
| An extension's effect | a manifest calls a write a read *(E)* | the platform sets the class, the owner approves; honest tool descriptions; credentials with the right scope |
| An extension | malicious code or a typosquat; secret leak; reading cell data *(S/I/E)* | drafted by the owner's agent and applied by the owner; hash pin; the data directory is not mounted; egress by list; secrets stripped from output; code read-only to sessions |
| The seal set | an agent reads seeded-fault labels or thresholds *(I/T)* | outside the cell's database and outside tool reach |
| Session and server on one uid | a session reads the database and the seal key *(I/E)* | the session user differs from the server's (K2); until it exists F1/F2 do not hold |
| The unauthenticated browser channel | an agent on the machine calls RPC like the owner: approves grants, applies changes, touches every rail *(S/E)* | one shared owner key in a file the ask rules name (a tripwire: a session can read it, ADR-0138), and the owner's Google sign-in (ADR-0022, ADR-0105); an OS user per session plus a uid firewall is the later hardening |
| A mentor's session on `/link` | a stolen worker key steers a card as that mentor; a code passed on over chat is read by someone else *(S/E)* | a key per device, compared in constant time, shown only to the owner, ended through the Assistant's `revoke_worker_key` or by the mentor's removal (ADR-0060, ADR-0062) |
| The Assistant deciding for the owner | outside text in a Chair's question (an MR comment, a post, a source's content) steers it to lift a stop the owner pulled or raise a ceiling *(T/E)* | every act recorded as the Assistant's with its reason and overturnable by the owner; secrets stay the owner's; one delegation function to narrow when hardening is asked for (ADR-0062) |
| The AI runtime on `/runtime` | another process on the machine dials `/runtime` as the runtime and is handed every session *(S/I)* | a key minted per server start, passed on the runtime's stdin, compared in constant time (ADR-0058) |
| Signature | a steered agent controls what is signed *(T/R)* | sign the content hash, not only a chat message |
| The owner's queue | flooding the queue so the owner approves by reflex *(D)* | a cap per chain, de-duplication, ordering by the effect ladder |
| The cell allocator | one cell touches another *(I/E)* | own user and directory; the allocator holds no data |
| Release chain and auto-update | a leaked signing key or captured release process steers every customer's agents *(T/E)*; forced downgrade; the update check leaks IP *(I)* | HSM, two-person signing, reproducible builds with published hashes, refuse a lower version, a transparency log; the static server identifies no cell. **Needs its own threat model before the first customer** |
| The landing queue | an unjudged or combined-broken change reaches the base; the push account is used outside the queue *(T/E)* | only a version whose patch id carries every gate lands, checked on the combined tree; agents cannot push; a breach is reverted by code; the session's own OS user (K2) and `land_policy` by origin and path are the hardening (ADR-0114) |
| A repository's release | an agent-written procedure publishes harm under the owner's accounts, or reads a credential *(T/E/I)* | the procedure is a rail (`security`); it runs only on a landed commit; the credential stays in the injecting proxy; `irreversible` grants wait out the overturn window; a breach is taken back by code; `land_policy` on `.remit/release/` and per-step connections are the hardening (ADR-0116) |
| The platform ledger | an agent edits the log *(R)* | `source_calls` is written by the server; a session can read and write the log files (ADR-0138); the hash chain is named debt |

## 7. What is still wrong on each operating system

- **Every OS:** Remit runs no isolation (ADR-0138). A session reads the environment of every process of the same user,
  including another session's MCP token; a model call with personal data bypasses a law policy; a program reaches the
  network around the egress gate, whose loopback port another process can borrow; there is no memory limit.
- **macOS:** the stop button is not final against a `setsid` child without a session user (K2).
- **Linux:** F2 stays false until the session user and a GitLab connection kind exist; the law depends on a legal pack that
  does not exist and `events` has no retention.

## 8. Units of work (dependencies, not phases; each is done whole)

| Unit | Content | Needs | Status |
|---|---|---|---|
| U0 | the web part of ADR-0024 | none | Built |
| U1 | K1 recovery | none | skipped by the owner |
| U2 | K2: a session user, a real stop, egress that records and counts, pidns, macOS `(deny default)`, a stop drill | none | Planned |
| U3 | K3: the cell allocator | U2 | Planned (no company defaults in code: Built; owner key for the owner, codes the owner makes for a person: Built) |
| U4 | K4: the gate | U2 | Built in part: the ladder (ADR-0027), `NoGrant`/`Rail`, `shell` and `web` capabilities (ADR-0028), `source_calls`; Planned: `Stopped`, `Law`, the seal set outside the database |
| U5 | K6: repository-declared checks | U4 | Built (ADR-0025); the GitOps executor is HOLD |
| U6 | K5: the extension host | U4 | Built: the plugin runner (`crates/plugin`), `crates/plugin-sdk`, `propose_extension`, `remove_extension`, `poll`, the door's signatures (ADR-0030); the `Forge` service lent to a sense (ADR-0057); Planned: the Module Federation loader, other services for plugins, a chain-written source path, forges as processes |
| U7 | K7: judges through an API, a backend per cell | U4 | Planned |
| U8 | K8: reason codes, scorecard rows | U4 | Built in part (the gate-round cap becomes a question to the owner); the rest Planned |
| U9 | K9: retention, delete, AI notice, signed hash | U3 | Built in part (`remit export`) |
| U10 | K10: the owner's screens | U0, U2, U8 | Planned |
| U11 | K11: distribution | U6 | HOLD |
| U12 | K13: items routed to the Assistant, `install_release`, Remit's own facts, `.remit/rails` and the `security` verdict (ADR-0087) | none | Built |
| U13 | K3: projects, several products in one cell (ADR-0108) | none | Planned |
| U14 | K6: the landing queue, `Landing`, the automatic revert, the forge's checks of a landed commit (ADR-0114) | none | Built |
| U16 | K13: releases by a repository's `.remit/release/`, repository grants, the code-driven way back, `install_release` as a step (ADR-0116; built with U14 in one plan) | U14 | Built; the dev instance's cut-over follows |
| U15 | K14: the measured limit, shares by key result, the key result tree, attempts, the progress rail (ADR-0115) | U14 for the back-pressure | Built |

No time estimate: there is no data, and every number would be a guess (LOW).

## 9. Universality: three sample companies through the design

- **A (EU SaaS on AWS with Postgres, Stripe, HubSpot, Zendesk, GitHub, Slack):** runs after U2-U6 plus routing models by region
  (K7), the auth kinds (K5) and receiving events from any source (K5). Postgres is not a REST manifest, so it needs a plugin.
  About five or six plugins. MEDIUM.
- **B (a regional bank):** **not a fit today.** Because the customer runs the cell inside their own network, a system behind a VPN
  needs no remote executor, and the model goes through an internal gateway or Bedrock/Vertex. What remains is the
  platform (no container image, and no isolation between agents and the host, ADR-0138), identity (no SSO;
  `-autonomous` is on by default so the Chair answers permission requests for the owner), and two-person approval. Works-council
  codetermination (BetrVG) and a regulator's approval requirements are outside the design. HIGH.
- **C (a Vietnamese shop with Shopify, VNPay, Zalo, Facebook):** work is files in a repo and the result is a landing (K6), so no new
  result kind; Shopify (one GraphQL endpoint for read and write), VNPay (HMAC-SHA512 per request) and Zalo (OAuth refresh) need
  the auth kinds and a GraphQL classifier in K5 (HMAC-query and OAuth2 refresh still open). Invisible cost (ADR-0010) is vital to a
  ten-person company: the adopter reads their own invoice. HIGH for the diagnosis.
- **One cell per company** fits A and C on one VPS, does not fit enterprise Kubernetes, and a laptop is for trying only.
- **Leaving a company** needs the list of every place data is kept (`work/`, transcripts, logs, key copies, branches pushed to the
  forge, up to 30 days at the model provider), a JSONL export, and delete by destroying the cell's key (crypto-shredding). Planned.

## 10. Decisions

**Made by the owner** (2026-11-02 unless dated): cells, not multi-tenancy (K3); one owner, no roles (ADR-0026), who comes in with the owner key or with Google (ADR-0105, 2026-11-17); every seat is an AI employee and people,
the owner among them, mentor them, positions being words (ADR-0062, 2026-11-04); a mentor works
from codes he has made for them (ADR-0060, 2026-11-04); the Assistant decides for the owner in everything but secrets (ADR-0062,
2026-11-04); Claude sessions run in an AI runtime outside the core (ADR-0058, 2026-11-04); one
OS user for the cell and one for the session, where the work is Planned (K2); Bash/WebFetch/WebSearch only by capability
(ADR-0028); extensions are go-plugin processes (ADR-0034); Git is the base of all work and Remit lands every change itself, with
no merge request (K6, ADR-0114, 2026-11-19); key results are the unit of scheduling on measured capacity (K14, ADR-0115,
2026-11-19); what lands is released by its repository's own procedure in code (K13,
ADR-0116, 2026-11-19); closed source and auto-update with an
owner's postpone button, no compatibility promise, a plugin broken by an update is disabled and reported, the update still
activates (K11); backup and restore skipped (K1); Remit evolves into Remit (named 2026-10-27); the market is SaaS
and e-commerce, banks are outside.

**Open:**
1. Whether "least grant" of ADR-0018 stays (the default is to keep it).
2. The order of extension work: event sources and tools (U6) before forges; the GitOps executor (U5) before any extension that
   writes.
3. For real-time work: run at once and log to Git afterwards (the proposal), or require a landing first.
4. When an outside forge becomes a priority (a third forge in use).
5. Licensing terms, warranty and limitation of liability; vendor liability under the EU product liability directive; the custody
   of the signing key; the security-report process; a no-IP-log update server; N for the postpone button and its security-patch
   exception (all HOLD until the first customer).

## 11. What is uncertain

- Accepting the loss of the SQLite file without a backup (K1) is bounded by Git holding pushed work, but orchestration context is
  lost. Not tested by simulation.
- Every **Proposal** is a hypothesis until a unit is built and passes its tests. Many statements about code (EXP) were not
  cross-checked; read the relevant code before each unit.
- The cost of a session user, of the egress proxy on macOS and of a second worker harness has not been measured. The count of call
  sites (about 100) came from a text `rg` and may include comments (MEDIUM).
- The deliberate deferrals (hash chain, human anchors, seeded faults, pass^k replay, the full legal pack, outside forges, SSO for the
  owner, containers) are **named debt** for after the first numbers exist; the blueprint asks for some of them, so this is where the
  design is weaker than the blueprint.
- Our liability as a closed-source vendor with auto-update (the directive, updates, the licence, warranty) has been read by no one
  but `[R §1.2]`; it needs our counsel.
- The release chain and signing key have no detailed design or threat model; the risk and the minimum controls are stated, and
  the SolarWinds and 3CX cases are cited from general knowledge, with no source in this document.
- The cost of support without telemetry and the quality of the `diagnose` bundle have not been measured.

## Record

Reviews of this design: `docs/archive/council/design-review-01-security.md`, `design-review-01b-security.md`,
`design-review-02-architecture.md`, `design-review-03-universality.md`. Code read again for it: `crates/grant/src/lib.rs`,
`crates/agent/src/agent.rs`,
`crates/model/src/actor.rs`, `crates/source/src/gate.rs`, `crates/orch/src/grants.rs`, `crates/orch/src/source_calls.rs`,
`crates/egress/src/egress.rs`, `crates/remit/src/flags.rs`, `crates/childenv/src/childenv.rs`.
