# 0096. `orch` splits into crates along an acyclic graph, the chain core at the bottom and the application on top

Status: accepted, 2026-11-13. Amends ADR-0072 (its crate table: "`orch` ... application layer and shell, one crate", and its
*Revisit when* "split it into crates along the agents' file groups"). Built 2026-11-13; *As built* below records where
the code departs from the plan and why.
Source: the owner, 2026-11-13 (translated): "the orch package is too huge"; his call to map the dependencies and write the
record before any code moves.

## Context

`crates/orch/src` is 214 files and about 73,200 lines (`find crates/orch/src -name '*.rs' | xargs wc -l`). Everything
hangs off one type: `Orchestrator` (`crates/orch/src/orch.rs`, 36 fields, its in-memory `Shared` 18 more) is reopened by
`impl Orchestrator` in 160 of the 214 files. Rust lets only the defining crate write `impl Orchestrator`, so no file can
leave the crate as it is.

The map was built from the source at commit `1a8beaf` plus the working tree of 2026-11-13 (scripts outside the repository:
for every file, the `Orchestrator` fields it names as `self.`/`o.`, the methods it calls that exactly one file defines in an
`impl Orchestrator` block, and its `crate::<module>` paths; code after a file's `#[cfg(test)]` left out). Files were
grouped first by the project agents that own them (`.claude/agents/*.md`); 25 files no agent lists were placed by their
`//!` header. It counts references, not semantics: a method name defined in two files is skipped, so the counts are a
floor.

### What the map shows

1. **One knot.** The file graph has one strongly connected component of **187 of 214 files, 69,879 of ~73,200 lines**
   (Tarjan over call and path edges). Every agent's file group sits in it: chain-runtime 45 files, signals 32,
   agent-tools 30, objectives-and-crew 30, review-and-ship 15, sources-and-plugins 13, rails 3, unassigned 19. Cutting
   crates along the agents' groups, as ADR-0072 foresaw, does not compile: every group reaches every other in both
   directions.
2. **The chain core reaches up into every domain.** It is not a lower layer today:
   - `step.rs`, the pure transition, imports domain rules: `gate::still_missing`, `goal::{GOAL_JUDGE_TASK,
     MAX_GOAL_CHECKS, finding_lines}`, `submit::submitting`, `watch::{SUBSCRIPTIONS_MAX, watch_*_text, watch_words}`
     (`step.rs:62-71`).
   - `apply.rs` and `commit.rs`, the one writer, write the signal pipeline's readings (`ingest::Write`, `ingest::Door`,
     `fact::finished_fact`, `live::mr_moved_in`, `signals::put_signals`: `apply.rs:13,130,235,239`,
     `commit.rs:14,17`) and other domains' records (threads, helps, employee names).
   - `lifecycle.rs` calls `employee_rules` 10 times and `chair_hire` 3 times; `turn_end.rs` dispatches to the Assistant,
     the goal judge, duels, the gate and helps; `views.rs` lays out every domain's view (43 references out); `schedule.rs`'s
     Tick runs every domain's periodic work.
3. **Every domain leans on a few shared mechanisms that sit in the wrong place.** `orch.rs` (`now`, `me`, `shared`,
   `add_event`, `decide`, `new_id`) is referenced 476 times from other files; `tools.rs` 70 (the `Tools` builder,
   `Effect`, `as_json`, `NoInput`: every domain's `*_tools.rs` registers through it); `schedule.rs` 57 (`kick`);
   `change_item.rs`, `changes.rs` and `owner_acts.rs` (a raised change item, the owner's secret acts) from crew, sources
   and signals alike.
4. **The chain core and the sessions are one unit.** With the core alone at the bottom, it still calls the session layer
   74 times (`session.rs` 21, `outbox.rs` 13, `health.rs` 10, `failover.rs` 4: telling a Chair is sending through its
   session). Two crates there would need a trait the size of the session API for no gain.
5. **"agent-tools" is not a domain.** Its group holds the tool mechanism (`tools.rs`), the per-card assembly
   (`server_for`, which adds every domain's tools), the Assistant and its tools (which act in every domain), and what an
   agent is given (agents, playbooks, capabilities, the skill library). The first is core, the second and third are the
   application, only the last is a domain.
6. **Callers outside.** `crates/rpc` and `crates/remit` name 40 distinct `orch::` items (types, the `Orchestrator`, a few
   free functions) and call about 90 distinct methods on it (`rg -o '\.(o|orch)\.(\w+)\(' crates/rpc crates/remit`).
7. **What changes together.** Of the 9 commits touching `crates/orch/src` since the cutover (2026-11-11), 8 touch a file
   this record places in the top crate, 7 one in the chain core, and 1 touches a single proposed crate only. The sample is
   small; it says the files every change goes through are the aggregators and the core.
8. **Tests.** 65 tests; `tests.rs` (2,194 lines, 28 tests) drives the whole orchestrator as a channel does, across
   domains (coach, Assistant, mentors, repeats); the rest sit beside their module.
9. **Build time: not measured.** The shared target directory (`$HOME/.cache/remit-target`) was held by other sessions'
   builds of their uncommitted work in `wire`, `agent` and `orch` ("Blocking waiting for file lock on build directory");
   the warm-up `cargo build -p orch` had not finished after 8.5 minutes and was stopped. A timing taken then would have
   measured their work. ADR-0072's trigger (an incremental build over a minute) is therefore neither confirmed nor
   refuted; the motive this record acts on is the size and the knot, not a number.

## Options

1. **Split now along the agents' file groups, each crate with its own trait for what it needs** (ADR-0072's *Revisit
   when*). Does not compile without first breaking the knot of (1); with the groups as drawn, the cross-group references
   that point the wrong way number in the hundreds and the core would declare a trait per domain it calls.
2. **Keep one crate; untangle inside it.** Per-domain service types over a slim core handle, the module graph made
   acyclic, no new crate. Cheap to move, but nothing enforces the edges: the knot of (1) grew under exactly this regime,
   and the owner removed fitness tests (no architecture test to hold it). It does nothing for the build.
3. **Hooks or an internal event bus.** The core publishes typed happenings (a turn ended, a chain finished, a commit
   recorded) and domains subscribe. Removes the core's upward calls, but the order of effects (a goal judge before the
   coach, a gate before a finish) becomes registration order nobody reads at the call site, against *Explicit over
   implicit*; a missed subscriber is silent.
4. **Layer the crates by what everything needs, then split the domains (chosen).** Move the shared mechanisms down into
   the chain core, move the aggregators up into an application crate, make the core's few remaining upward calls one
   trait it declares, and cut the domains between them. The compiler then enforces every edge.
5. **Extract only the pure rules.** 36 files (~6,550 lines) touch neither `Orchestrator`, the store nor tokio; moving them
   leaves the knot and the build as they are.

Patterns weighed (`aio-architect-advisor`): the **modular monolith** ("the critical word is enforced": a module boundary
holds only if crossing it is checked) argues against 2; **separation of concerns** and **hexagonal ports** (traits
declared by the consumer, as `ports.rs` already does for `Sources`, `PluginHost`, `Bundled`) shape 4; **strangler fig**
is rejected: AGENTS.md makes a change whole, never old and new side by side.
Mental models (`aio-mental-models`): **Gall's law** (the working system is the start: the split moves code, it does not
redesign behaviour), **bottlenecks** (the graph's constraint is the handful of mechanisms everything calls; fix those and
the rest follows), **second-order thinking** (what the split makes harder: a cross-domain feature now crosses a trait).

## Decision

### The crates

Bottom to top. A crate depends only on crates below it; same-rank crates do not depend on each other. Sizes are the files
as they stand (`tools.rs`, `orch.rs`, `views.rs` split across crates as marked).

| Crate | Provides | From `orch/src` | Files | Lines |
|---|---|---|---|---|
| `chains` | The chain core and its sessions: `Core` (store, notifier, clock, launcher, in-memory chains and runs, trackers, the stop tokens), `act`/`once`, `step`, `apply`, `commit`, admission, `kick`, events and decisions, the `Tools` builder and `Effect`, change items and owner acts, the readings door (`ingest`), facts, sessions, outbox, mailbox, stream, plans, prompts, permissions, accounts, health, failover, handover, subagents, workflows, repetition rail | chain-runtime's group minus the aggregators, plus `tools.rs` (builder half), `change_item.rs`, `changes.rs`, `owner_acts.rs`, `ingest.rs`, `fact.rs`, `event.rs`, `event_fields.rs`, `live.rs`, `quote.rs`, `tones.rs`, `blocks.rs`, `seen.rs`, `shown_decision.rs`, `permission_ask.rs`, `repeats.rs`, `turns.rs`, `telemetry.rs` | 59 | ~20,800 |
| `capability` | What an agent is given: agents, playbooks, capabilities and their requests, the skill library (import, render), rails on what an agent writes | `agents.rs`, `playbooks.rs`, `capabilities.rs`, `capability_request.rs`, `library*.rs`, `rail.rs`, `check_rail.rs`, `accept.rs`, `judge_store.rs`, `plugin_ships.rs`, and the tools that edit them | 21 | ~5,000 |
| `pipeline` | The signal pipeline: readings to firings to deliveries, subscriptions, sensors, routines, watches, event signals, the board | signals' group minus `ingest.rs`, `fact.rs`, `event.rs`, `live.rs`, `hire.rs` | 34 | ~9,300 |
| `reach` | Sources, credentials, grants and the gate, programs and scripts, plugins and extensions, outside MCP servers, accounts from plugins | sources-and-plugins' group, `mcp_servers.rs` | 17 | ~4,900 |
| `keys` | Client keys and their door, worker keys, entrance keys | rails-and-sandbox's group | 6 | ~1,800 |
| `crew` | Objectives and key results, employees (hire, standing, names, faces, the Chair's hiring), memory, mentors and helps, teaching, the coach, duels, outcomes, the analyst, feedback | objectives-and-crew's group, `chair_hire.rs`, `employee_standing.rs`, `team.rs`, `employee_face.rs` | 33 | ~9,600 |
| `shipping` | Clones and checks, submit, review and MRs, threads, the gate and the goal judge, verification, merging and canaries, releases of this instance | review-and-ship's group, `hire.rs`, `release.rs`, `remit_facts.rs` | 19 | ~8,800 |
| `orch` | The application: `Orchestrator` composing the services above, every action a channel calls, `server_for` (a card's tools from every crate), the Assistant and its tools, views and standings, lifecycle, waits, recovery, the Tick, intake, the inbox, digests | `orch.rs` (composition half), `views*.rs`, `lifecycle.rs`, `recover*.rs`, `wait.rs`, `inbox.rs`, `standing.rs`, `agentstanding.rs`, `atwork.rs`, `boards.rs`, `waiting.rs`, `turn_end.rs`, `assistant*.rs`, `assist*.rs`, `digest*.rs`, `intake.rs`, `activity.rs`, `runs.rs`, `worker.rs`, `tests.rs` | 25 | ~12,900 |

`crew` and `shipping` sit one rank above `pipeline`, `reach`, `keys` and `capability` and may use them; they do not use
each other (below).

### How each cycle is broken

1. **Shared mechanisms move down** (finding 3): the `Tools` builder, `Effect`, change items, owner acts, the readings
   door, facts and events go into `chains`. A tool handler stops receiving `Arc<Orchestrator>`: `tools.rs`'s
   `Call = Box<dyn Fn(Arc<Orchestrator>, …)>` becomes a handler that captures the service it acts through, so each
   crate registers its tools against its own service.
2. **Aggregators move up** (findings 2, 5): `views.rs`, `turn_end.rs`'s dispatch, the Tick's per-domain work, `lifecycle.rs`,
   `wait.rs`, `server_for` and the Assistant live in `orch`, which sees every crate.
3. **The core's remaining upward calls become one trait `chains` declares, `Domains`**, implemented once in `orch`: what a
   turn's end, a commit and a Tick hand to the domains, each a named method called at the place it is called today, in the
   order it is called today. One implementation, read top to bottom, replaces the scattered calls; this is option 3's
   decoupling without its hidden order.
4. **What the step reads moves into `chains`, never through `Domains`.** The step is pure and stays pure: a trait call
   in it would make it effectful. Every crate-crossing import of `step.rs` (`step.rs:62-71`) is a pure function, a
   constant or a value type, so each moves down as a chain rule:
   - `gate::still_missing`, `goal::{GOAL_JUDGE_TASK, MAX_GOAL_CHECKS, finding_lines}`, `submit::submitting` and
     `watch::{SUBSCRIPTIONS_MAX, watch_*_text, watch_words}`;
   - `hire::{goal_deadline, goal_deadline_id}` (`hire.rs:82,88`): the goal judge's deadline subscription the transition
     writes;
   - `lifecycle::start_from` (`lifecycle.rs:1029`): how a chain takes its playbook; `lifecycle.rs`'s actions stay in
     `orch`;
   - `wait::{WakeSubs, wake_message, clock, rfc3339}` (`wait.rs:37,170,199,210`): a wake's subscriptions and its text;
     `wait.rs`'s shell stays in `orch`. This also removes `pipeline`'s four `wait::rfc3339` references to `orch`.

   The one writer goes the same way. `commit.rs` writes, inside the chain's transaction, the records a change carries
   (`apply::Records`) through other domains' writers: `apply_subs::put_waits`, `employee_name::put_employees`,
   `help::put_helps`, `inbox::put_asked`, `signals::put_signals`, `threads::put_threads`, `workflow_run::{put_runs,
   stop_runs_of}` (`commit.rs:10-20,270`), and reads `wait::{Wake, chain_read}` and `deliver::Delivered` (`apply.rs:12,
   18`). Each is a write onto `store::Tx` or a value type, so each moves into `chains` beside `Records`: a chain's change
   is written by one writer, in one place, whatever domain the record belongs to.

   `shipping`, `pipeline` and `orch` import them from `chains`. The rule for the cut: anything `step`, `apply` or `commit`
   reads that does no I/O is in `chains`; anything that does I/O and that they call is a `Domains` method.
5. **Domain to domain, same rank:** `pipeline` calling `shipping` (`hire.rs`'s `fire_hire`, `merge_request_sense.rs`,
   `review.rs`'s `sync_mrs`) is a firing's action; `pipeline` declares the actions it fires as a trait (`Actions`), `orch`
   implements it with `shipping`. `crew` and `shipping` reach each other in 22 places (teaching from an MR's verdict,
   objectives opening checks; the goal judge and `hire.rs` from crew); each is moved to the side that owns the concept
   (a verdict taught is `crew` reading what `shipping` hands it through `Domains`), and what remains goes through `orch`.
   `capability` calling `crew`, `pipeline` and `shipping` is tool assembly (each domain's read tools): it moves to
   `orch`'s `server_for`.
6. **Channels** (`rpc`, `remit`) keep calling `orch::Orchestrator`; its methods delegate to the services. The 40 `orch::`
   items they import are re-exported from `orch`, so `rpc` and `remit` change only where a type moved name.

### Order of work, inside the one change

1. Commit or land every session's open work on `crates/orch` first: the split moves every file.
2. `chains`: cut `Core` out of `Orchestrator`, move the mechanisms down (cycle 1, 4), declare `Domains`.
3. The domain crates, leaves first: `keys`, `reach`, `capability`, `pipeline` (with `Actions`), then `crew` and `shipping`.
   Each owns its service type over `Arc<Core>` and the `Cargo.toml` edges the table allows.
4. `orch`: the composition, the aggregators, `Domains` and `Actions` implemented, `tests.rs` as its integration tests
   (`crates/orch/tests/`), since it drives the whole application.
5. Channels compile against `orch`'s re-exports; `docs/design.md`, `crates/orch/README.md`, each new crate's `README.md`,
   and every `.claude/agents/*.md` file list name the new crates.

The tests move with their modules; none is added or dropped. `make test` stays under a minute.

## Consequences

- Every edge between the parts is a `Cargo.toml` line the compiler holds; the knot cannot grow back silently.
- `chains` stays the largest crate (~20,800 lines): the chain core and the sessions are one unit (finding 4). A change
  there still recompiles every crate above it.
- A change inside one domain crate recompiles that crate, `orch` (~12,900 lines) and the channels; most commits so far
  touched `chains` or `orch` (finding 7), so the build gain for a typical change is small. The gains are boundaries and a
  smaller unit to read.
- Two traits are added (`Domains`, `Actions`) and the `Tools` handler shape changes for every tool.
- A cross-domain feature crosses a trait or lands in `orch`; deciding which is part of designing it.
- The project agents' areas become crates; `chain-runtime` and `agent-tools` are redrawn (`agent-tools` keeps
  `capability` and `orch`'s tool assembly and Assistant).

## Revisit when

- **Before cutting:** every `crates/orch` file other sessions hold uncommitted is landed (21 on 2026-11-13, `git status --short crates/orch`).
- **`chains` alone takes the build over a minute**: split the sessions from the chain core behind a session trait the core
  declares (finding 4 says this costs a trait the size of the session API).
- **`orch` grows past `chains`**: what it aggregates for one domain moves into that domain behind `Domains`.
- **`crew` needs `shipping`** (the edge runs `shipping` → `crew` only, *As built*): the concept between them (verdicts that
  teach) is a crate of its own.

## As built

The split landed in one change on 2026-11-13. Where it departs from the decision above, and why:

- **The order is linear, and `shipping` depends on `crew`.** Bottom to top: `chains`, `reach`, `pipeline`, `capability`,
  `keys`, `crew`, `shipping`, `orch`. `capability` reads the pipeline (automations, routines) and `keys` reads
  capabilities, so they are not one rank. `crew` and `shipping` could not be kept apart: the goal judge and checks read
  objectives and employees in more places than a trait would cover cleanly, so `shipping` uses `crew` and never the
  reverse. *Revisit when* names what would split them.
- **Sizes.** `chains` 90 files, ~25,500 lines; `reach` 17, ~4,300; `pipeline` 36, ~8,900; `capability` 25, ~4,800;
  `keys` 6, ~900; `crew` 31, ~8,900; `shipping` 20, ~7,400; `orch` 36, ~10,700, plus its test binary (~5,000).
  `chains` is larger than planned because rule 4 pulled more into it than the map counted (below).
- **More moved down into `chains`**, each because `step`, `apply`, `commit` or the session layer reads it and it does no
  I/O of another domain: sealing, entrance keys, git basics, signal sources, watches, the customer rule, duel marks, the
  hire input, the team rules, outside decisions, the owner mentor, takes, what was recalled, stages, the workflow tools,
  the judge roles, `check_words`, the named-Chair refusal, the capability names (`cap_names.rs`), the inbox, the
  lifecycle actions on one chain, the Tick's chain work (`schedule.rs`: admission, stalls) and the no-progress rail
  (`spin.rs`). `orch` keeps the lifecycle actions that cross domains (`lifecycle.rs`), the Tick that calls each domain
  (`tick.rs`), answering an item (`answer.rs`, `change_answer.rs`).
- **More moved up into `orch`**, each because it assembles from several crates: the Chair's hiring (`chair_hire.rs`), a
  client's tools (`client_tools.rs`), mentor standing, a card's capability tools (`capability_tools.rs`), the playbook
  author ask, propose, the archived list, the seen marks.
- **Elsewhere:** `check_rail.rs` is `shipping`'s (a rail checked against telemetry, which a change ships under); the
  built-in briefs and `gate_instructions.rs`, `okr.rs` are `capability`'s (what an agent is given); a mentor's device
  acting once per request is `crew`'s (`mentor_act.rs`).
- **The capability service is `Capabilities`**: `Capability` is the catalog entry the channels show.
- **`Notifier` splits per crate.** `chains::Notifier` holds the core's kinds of change; `pipeline`, `capability` and `crew`
  each declare the kinds only they make; `orch::Notifier` is their supertrait with the views' and the Assistant feed's.
  `chains::Notifier` gained `views_moved` and `mentors_moved` (no-op by default) so the core can mark views it cannot
  name. `Watched` (`views_watched.rs`) forwards every method but four it did not forward before the split either
  (`thread_updated`, `feedback_updated`, `insight_added`, `objective_updated`): unchanged behaviour, recorded here.
- **Services hold the services below them.** `Crew` holds `Pipeline`, `Capabilities` and `Keys`; `Shipping` holds
  `Reach`, `Pipeline`, `Capabilities` and `Crew` (`shipping::Below`). Each is `{ core: Arc<Core>, me: Weak<Self>, .. }`
  and derefs to `Core`; `chains::request::Service` gives each `act` and `once`.
- **`orch` re-exports the crates** (`orch::chains`, `orch::shipping`, ...) and the names `rpc`, `door` and `remit` used, so
  the channels changed only where a type moved name. `orch::Config` is `{ core: chains::Config, memory, jira, checks,
  updater, instance }`.
- **Tests:** the application tests are one integration binary, `crates/orch/tests/app/` (orch's `tests.rs`, and each
  lower crate's tests that drive the whole application, named `<crate>_<module>.rs`). `chains` has a `testing` feature
  that exposes the fake forge and seeded draws to it. Pure tests stay beside their module.

### Changes that landed during the split

Four features reached `crates/orch` while the split was in progress (ADR-0101 to ADR-0104) and were placed by concept:

- **The no-progress rail** (`spin.rs`, ADR-0104) is `chains`'s: `schedule.rs`'s `unstick` and the Core's stall state run
  it. The Assistant's exchange check stays in `orch`'s `assistant_tools.rs`, calling `Core::replies_in_a_row`.
- **The overturn window** (`overturn_window.rs`, ADR-0101) is `orch`'s: what calls it (answering an item, the
  Assistant's overturn, the Tick) is `orch`'s. `reach`'s `GrantAsk` is public so the window reads a grant's effect.
- **Seeded faults** (ADR-0102) split by concept: the trial's capability and playbook ids are `chains::cap_names`, its
  Chair's brief `capability::briefs`, and the trial (plant_fault, the submit refusal, `trial_ruled`) is `shipping`'s
  (`seeded.rs`), beside the judges it measures.
- **The second opinion** (ADR-0103) follows the duel judge: its role and `is_second_judge` are `chains`'s
  (`second_opinion.rs`, the marks the session layer reads), the rest is `orch`'s (`second_opinion.rs`, it re-decides the
  Assistant's answers), and the core hands a judge that ends without comparing up through
  `Domains::second_judge_ended`.
