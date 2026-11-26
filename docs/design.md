# Remit — design

Remit is a single-owner console where AI agents discover, solve and review work as **chains**. Each chain is one
working session run by an orchestrator agent, the **Chair**, which hires as many agents as the case needs.
The UI shows every agent as a card, every interaction as an edge, and lets the owner chat with any card. The core is small
and closed: whatever a company needs beyond it (a tool source, an event source, a forge, a screen) is a plugin
(see Extensions), and the result of all work is files in Git, landed on the base branch by Remit's own merge queue.

Remit is the evolution of Remit: the same codebase carried into the new design. The code, the flags, the crates and the
binary still read `remit` until they are renamed (`docs/superpowers/specs/2026-10-27-rename-design.md`), so this document
uses Remit wherever it names them.

This document says how the system is now. Why a standing decision was made is recorded once, in `docs/adr`
(`docs/adr/README.md` lists them). Where the product is going, and what is not built yet, is `docs/remit-design.md`. Where each
part lives in the orchestrator, and how to make the usual changes there, is each crate's own map: `crates/orch/README.md`
and the README of each crate below it.

## Intent

- AI does the work end to end, around the clock: routines the owner sets up find work, Chairs decide and hire agents, agents code
  and review, and Remit lands each approved change on the base itself (ADR-0114). The owner reads results, gives his taste
  and overturns what he disagrees with by telling an agent; nothing else needs him.
- Agents answer to the customer: the person a change is for, who is not the owner who reads it. Every chain opens with who
  the customer is and what they get, and its judges rule on that side of the change (ADR-0016).
- A playbook is reference, never a rule: a Chair reads one when it fits and takes what helps. What code branches on is the
  chain's kind, fixed when it is queued (ADR-0113). The Chair hires agents of any role, any number, without asking.
- The owner may still chat with any card at any time; his messages go first.
- The owner also has the Assistant, his agent for the whole app: a chat on every screen, in no chain, that reads what
  Remit holds and acts on his words (see The Assistant).
- Agent-centric: the owner steers by talking to agents, not by filling in forms. Whatever the owner might configure,
  playbooks included, is an agent tool, and the UI exists to watch agents, talk to them and review their results
  (see `AGENTS.md`).
- Agents are specialised by where they work. An agent that changes, reviews or verifies code works in one repository:
  its working directory (cwd) is that repository's worktree, which gives it the repository's own instructions
  (`CLAUDE.md`, `AGENTS.md`) and bounds where it may change files. Work in another repository goes to another agent.
- The Chair reuses before it hires. Work that continues an agent's own goes back to that agent; a new agent is hired
  for a different change or repository, for a reviewer or QC that must judge with fresh eyes, or when the old
  agent's context is full. This is judgement in the Chair's brief, not a rule in code.

## Goals, non-goals, anti-goals

- Goals: work runs without the owner; every result is a landed change whose commit message stands alone and says what the
  customer gets;
  see what every agent is doing and why it exists; chat with any agent; full metadata of every Claude Code
  session kept in a database.
- Non-goals (this version): users and roles (one owner, who comes in with the owner key or, when the server is set up
  for it, with Google, ADR-0105), putting a change on a base by anything but Remit's merge queue (ADR-0114), serving
  outside customers (distribution waits, `docs/remit-design.md`). The one forge write is the queue's push of a checked batch
  to a base branch, never forced; a chain's branch (`feature/mc-*`) stays in its worktree.
- Git is the base of all work: a cell needs a remote on a supported forge, whatever the work is, and the result of every kind of
  work is files in a repository landed on its base. Work with no software in it becomes files too (plans, content, configuration,
  decision records); its gates' verdicts are its approval, and the owner reads it after it lands.
- Anti-goals (failure even if it works): noise (chains that produce nothing worth reading),
  having to babysit, an agent doing something the UI does not show.

## Constraints

- Backend Rust, one Cargo workspace in `crates/` (ADR-0072). One Claude Code process per card (the `claude` CLI), spoken
  to over stdio in the protocol the Claude Agent SDKs speak (`crates/claudecode`), signed in with the machine's own Claude
  Code login (see The provider). Every one runs in the AI runtime (`remit runtime`, `crates/airuntime`), the core's child
  process, which dials the core back on the link with a key minted for that start of the core and speaks `wire::runtime`
  (`crates/runtimelink`, ADR-0058); the core starts no Claude Code process.
- The orchestrator is eight crates on an acyclic graph (ADR-0096), each depending only on those before it: `chains` (the
  chain core and its sessions, the step, the one writer, the mechanisms every domain uses), `reach`, `pipeline`,
  `capability`, `keys`, `crew`, `shipping`, and `orch` (the `Orchestrator` composing them, and what needs every domain at
  once). The core hands up to the domains through one trait it declares (`chains::Domains`), a firing through another
  (`pipeline::actions::Actions`); `orch` implements both. Channels call `orch`.
- Frontend Vue 3 + Vite + TypeScript, served by the `remit` binary. Every peer speaks one protocol, the link
  (`crates/link`): WebTransport sessions at `/link`, on UDP at the same address as HTTP (`crates/link-quic`). What crosses
  it is defined once: the records in `crates/model` and the contracts that carry them in `crates/wire` (a `link::contract!`
  per channel: calls, each a read, a change taking a `Request` or idempotent, and pushes), compiled into the server and
  every client core; `crates/tsgen` writes their TypeScript into `web/src/core/generated` (`make gen`). In the browser the client core is Rust too: `crates/replica` (the replica of what
  the server pushes, sans-IO) bound to WebAssembly by `crates/client-web`; it keeps the records and hands the page the
  models it draws (whole, then the rows that changed), which `useModel` (`web/src/core`) holds and Vue only renders.
- Storage SQLite (one file, readable by the owner only) through `rusqlite` (`crates/store`, migrations in
  `crates/store/migrations/NNNN.sql`). The store copies the database whole and
  consistent with `VACUUM INTO` into `backups/` beside it (owner-only): before migrating a database an earlier build wrote
  (without that copy it does not migrate), and once a day from the scheduler (`-snapshot-every`, 24h; 0: never), on a
  read-only connection of its own so no write waits for it. A copy is written under a `.part` name and named as a
  snapshot only once whole, so one cut short by a stop or a crash is never taken for one. Each kind keeps its newest five.
  The owner has chosen no restore feature: a snapshot is put back by copying the file, and the work itself lives in Git. A migration is released once
  it has run on the owner's database: the store records each migration's SHA-256 as it runs it (`schema_migrations`), and
  a database that ran a migration whose text has changed since is refused at open, untouched. A database migrated before
  checksums were recorded gets them at its first open, as its migrations read then. A migration never rewrites events
  (they are most of the database, and a rewrite holds the one writer for as long as it takes): an older shape is
  converted when an event is read, by the store's event reader alone. Where a shape begins is recorded by the migration
  that brings it (`event_shapes`: the first event id of each shape), so an event is read as the older shape by its id,
  never from what it holds. A record holds only values its build names (`text_enum!`, ADR-0107), so a build
  rolled back to may find records a newer one wrote that it cannot read: a list leaves each out, a read of it by id is
  refused (`store::Error::Unreadable`), it is logged once, and the count is `Health.unreadable`, which the Tick updates
  and the page shows under the top bar; the record is never written by that build and reads again once a newer one
  runs. Every session's record is kept, and kept small: once a day the scheduler has the
  store pack the text and data of each event unchanged for a week (deflated in place, a short write at a time), and a
  read unpacks them, so an event reads as it was written; a write over one stores it plain again. A home that goes (a
  chain deleted) takes every record under it in one step (`store::Tx::delete_home`).
- A chain is shown as an agent tree, an outline of who works under whom: the Chair, then each agent of its team under the
  agent whose work it builds on or judges, a run of work handed on from one agent to the next as one straight column,
  and on its line the other agents it works with; below, the cards Remit started. Each line says status, current
  work, its own turns and time; a folded branch says how many turns the whole branch took. The server lays it out, in each agent's
  standing.
- What waits on the owner shows with the agent it waits on: its answers (the answers the server offers; on a permission, text
  typed in the callout turns Refuse into "Refuse with this reason", which the session reads, ADR-0080) are
  shown in the drawer. Questions fold to their titles and scroll with the open panel. Other requests stay pinned above the
  message box; the Activity keeps a one-line record of where the agent stopped to wait. A call that never got its
  answer is retried with its request id, and a press while one is on its way does nothing. The agent is marked "Needs you", its line
  naming the wait ("Approve: Bash psql …", ADR-0078), wherever it appears (its drawer's header, whose mark takes the keyboard to those answers, its line in the agent tree, its runs in the Live
  feed, its mark under the chain's card in the board's column beside the chain), and a click there opens its drawer. An item that names no agent
  of the chain waits with the Chair. The Chair's plan shows in the chain's Overview and in the Chair's own Plan panel.
- The chain page and the drawer show where things stand as badges in their headers (a queued chain's "Queued · #2"), with
  the tab's content right under the tabs. A chain whose Chair checks back has a chip with the time beside its badge, on
  every tab; a click opens what the Chair will check, and while the chain waits, that a message wakes it now. A chain in
  the Watching column wears the badge Watching (an eye) in its header and a chip "until" the earliest watch's deadline; the
  chip opens the signal, the condition and the deadline. A finished chain is Done, Waiting to ship, or Not done (ADR-0067, ADR-0087): Done
  (tone `done`) when the goal judge confirmed every key result; Waiting to ship (tone `waiting`) when what is left is the
  landing of its change or putting it live, as the goal gate reads it (`owners_to_settle`: nothing once the change is marked
  live): "Waiting for its release · release api v2"; Not done (tone `notdone`) otherwise, saying what is missing ("Not done · 2 of the
  goal's results still missing", "Not done · not proven: <item>", "Not done · the goal check could not run", "Not done ·
  ended by you before it was checked"). The words, the tone and the column come from the server (`outcome_of` in
  `crates/orch/src/standing.rs`), computed from the chain, its landings and its verdicts each time they are read, so a chain stored before the
  rule reads the same way. The Chair's card of a finished chain says the same (its badge Done, Waiting to ship or Not done,
  its line the outcome's text). A chain that finished after the
  owner last viewed it is marked new in the Finished column (`ChainStanding.unseen`, ADR-0079). The Overview lists the chain's open watches (the signal, what wakes the Chair, the deadline, and a held
  watch's reason) with a link to each on the Subscriptions tab. A queued chain's Overview says when it works next and why (its place
  in the queue), and a queued chain opens on it.
- The pages are grouped in five areas, by what the owner comes for. The top bar reads Work · Results · Signals · Crew · Integrations
  (ADR-0019, ADR-0042): an entry opens its area's first page, is marked on every page of the area, and counts what waits for the owner in
  that area alone (Work the board's, Results the feedback not read yet, Signals the events that wait on him; Crew and Integrations count nothing). The pages
  of an area are a strip of tabs under the bar; a chain's page is under Work, with tabs of its own and no strip. A route says
  its area and its tab (`meta.area`, `meta.tab` in `router.ts`), which is all the bar and the strips read.
  Work is the Board and the Objectives tab (see Objectives). The Board is the one list of chains: its columns side by side (the server's, then the
  archived chains, read a page at a time), with a search over all of them at the end of the strip's row, so the strip costs
  the board no height. A card opens its chain; beside the chain's page is the board one column at a time, the column that
  holds the chain (it follows the chain as it moves), which the owner slides along to the others. Results is what came of the
  work: Outcomes (how well the agents do, across every chain) and Feedback (what the agents say about how they work). Signals is
  what Remit hears and what it does about it, in three tabs. Signals: a column for each source of signals it supports (time, event signals, a chain's
  landings, a chain's change being live, a chain's sensors, Remit's own facts; `wire::SignalSource`), each holding that source's signals and nothing else; a signal opens its readings and
  the firings on it and the subscriptions that listen to it, and an event signal's page holds its settings too (who set it, its
  rules, prompt and sensors, its door's address, its key and how to write to it); a source's name
  opens what its signals say, the shape of its keys, who may listen and the subscriptions that do (`/signals`). Subscriptions:
  what listens and what it did, check-backs, watches, routines and the event signals' own rules (`event_signal:<name>`), each
  leading back to the signal it listens to, and the firings (`/subscriptions`). Events: what came in, whichever way, and what
  became of it (`/events`). Crew is who works and what they are made of, in five tabs: Employees (see Employees), Mentors (see Mentors), Playbooks, Agents and Skills. Playbooks, Agents and Skills are boards with one column a kind of work (the record's `group`, by name, `general` last).
  Integrations is what Remit reaches out through and what reaches in, in four tabs: Tools, Plugins, Accounts (the Claude accounts
  agents run on) and Client keys (the keys outside agents hold to start chains). Each tab is a list with the open item beside it.
- What Remit does on its own is the firing of a subscription that ships with it (ADR-0035; `crates/pipeline/src/automation.rs`). A
  step that changes state records a fact, a reading of `fact:<noun>.<verb>` (`fact.rs`: `fact:chain.queued`,
  `fact:chain.finish-requested`, `fact:chain.landed`, `fact:repo.landed`, `fact:chain.marked-live`, `fact:check.stated`, `fact:turn.ended`), and the subscriptions to it answer at once: `hire` makes the goal judge; `run` starts a job (the landing sync and the record of a landing's checks, worktree collection, the daily snapshot, event packing) on its schedule;
  `end` ends a goal check whose time is up, by the one-time deadline subscription the request stored for its chain (`sub_check_<chain>`);
  `deadline` ends a mentor's offer of help not answered by its deadline and tells the Chair once, by the one-time subscription
  written with the help (`sub_deadline_<help>`, `fire_deadline`, ADR-0062).
  `fact:turn.ended` is recorded each time a card's turn ends (`turn_end.rs`, in its own write after the end, deduplicated by
  card, message and how it ended): the card's role, who opened the turn, done, stopped or failed, and its reply cut at 8000
  bytes, the one fact that carries words of the work, since a plugin's processor reads only the reading. Nothing ships to
  answer it: the owner has the Assistant subscribe a plugin's processor to it (`create_subscription` takes fact keys and
  `plugin:<id>.<name>` actions, `list_processors` lists them) to critique, extract or index the agents' replies.
  `sub_hire_goal_recheck` answers `fact:chain.landed` (written by the merge queue when a chain's change lands): for a finished chain whose last goal verdict left a key result unverified with blocker `write` or `deploy`, it hires the judge again, and the chain's badge follows the new verdict (ADR-0039). `sub_hire_goal_recheck_live` answers `fact:chain.marked-live` (written by `mark_live`, once per landing) the same way; once the change is marked live nothing is the owner's to settle, so that look ends the chain Done or Not done (ADR-0067).
  They are the subscriptions of kind `automation` on the Subscriptions tab; the owner pauses or resumes one by telling the Assistant
  (`pause_automation`, `resume_automation`): pausing the goal judge's hire means a finish is not checked and the chain finishes when the
  check's time is up. The replies of a command, the liveness rails (nudges, the silent-turn cut, the idle watchdog), the wake of cards
  waiting for a provider and the Chair's permission timeout stay in code.
- The Subscriptions tab lists every subscription of every owner kind, with the open one beside the list (the address names it,
  the first until it does). The server lays the list out (`SignalBoard`, `crates/pipeline/src/signalboard.rs`): the ones that wait on a
  time come first, soonest first, a due one that the scheduler holds (the limit is full, or no provider can run agents)
  ahead of the rest and saying which; the ones that wait on a signal follow, the newest reading first. A line says in plain
  words who waits, what it waits for (a time, a signal and its condition, a deadline), its state (waiting, due, held and
  why, a silent source in the words of its watch, underway, over, paused) and when it last fired. The lines are each waiting
  chain's check-back (when, and what its Chair will check), each watch a chain holds (addressed by the subscription's id:
  the signal, the condition, the deadline, and held with the reason while the source is silent), each
  routine's next run (see Routines), a chain or a routine paused, and each event signal with the per-chain tells, skips and
  dispatches that place its events under it. A routine's run, once its time has come, is held by the last
  of these, and also while its last run is at work. The open line shows the signal key, the newest readings
  (revision, time, cause and the value, cut at 600 characters and shown as text, since it came from outside) and the recent
  firings (time, outcome, expired) of its signal (`signal_get`). The list holds the subscriptions only, in the server's three
  groups (needs attention: held or due; coming up; paused or finished), each row saying what it does in the owner's words
  (a subscription to an event signal by its note). What fired is on the open line and on the Events tab. A signal is not listed: the line that reads it links
  it, and its page gives the time and revision of its newest reading, how many subscribe to it and why its source is
  silent. Its buttons are the owner's rails: wake a Chair now, pause
  or resume the chain or a routine, run a routine now. To move a check-back or a watch or change what is
  checked, the owner tells the chain's Chair; to have Remit start a chain on its own, once or on a rhythm, or to change or
  delete that, he tells the Assistant. The Assistant reads the same page with `read_signals`.
- Runs on a host the owner controls, with the owner's Claude Code login, forge accounts (`glab`, `gh`) and tools.
- **Forges.** A repository is hosted on its forge, GitLab or GitHub, which runs its pipeline on what lands (ADR-0114).
  The owner says which forge each host is with `-forges host=kind,…` (default `github.com=github,gitlab.com=gitlab`); a
  repository on a host with none is not opened. The orchestrator declares what it asks of a forge (the `Forge` port, `crates/chains/src/forges.rs`:
  who Remit works as (`account`), where a branch stands (`branch_head`), the checks on a commit (`checks`)) and `crates/forge`
  answers it, one adapter per kind, through the forge's own command line (`glab`, `gh`) logged in on the machine. Remit
  pushes with git from its clone as that account, and every commit the queue lands is authored and committed by it,
  the work's other authors kept as `Co-authored-by:` trailers.
- The server logs JSON lines to stderr; a record caused by a chain names it (`chain`), one caused by an event or the
  Assistant names its home (`home`), so one chain's log is one filter. The agents' stderr goes to `agents.log` in the data
  directory (owner-only, and hidden from every agent session along with `remit.log` of the supervisor), moved to `agents.log.1` once it would pass 64 MiB, so the two hold at most 128 MiB; a write it
  cannot make is said once in the server's log and never stops an agent.
- When nothing has run for `-idle-alert` (default 2 hours; 0: never) while work waits (queued chains, approved changes
  waiting to land, items in his inbox), the owner gets one inbox item that says what waits and what holds it (no provider, the merge queue's limit). It is not repeated while it is open, nor after he dismisses it until work moves again;
  once work moves it goes away by itself.

## Model

| Entity | Meaning |
|---|---|
| Chain | One case: goal, origin (`owner`, `proposed`: queued by another chain with `propose_chain`, `routine`: started by Remit from one of the owner's routines, which it names; `event`: started by a subscription for an event, whose address (`event:<signal>#<rev>`) it names; `client`: started by an outside agent holding a client key, which it names; `verification`: opened by a judge to check a result, see Employees), the key result it serves (`serves`: an objective and one of its key results, see Objectives) and through it its project (see Projects), the round of competing attempts it is an attempt of (`attempt`, see Round; none for a chain that competes with none), its kind (`model::ChainKind`, ADR-0113: `work`, `chore`, `objective`, `author`, `release`, `coach`, `analyst`, `librarian` or `trial`, set once when it is queued and changed by no tool; its gates and whether the owner waits on it are the kind's), the capabilities it holds (its kind's when it was queued, then what its Chair gave or the Assistant decided), status (`queued`, `running`, `paused`, `waiting`, `done`), archived or not, Chair card id, repos (each with its worktree, branch and the base it lands on, and the land in progress), finishing (the Chair's summary while the goal judge checks it: the judge, when its time began, and its verdict once given), and once done why its last finish had no ruling of the goal judge (`unjudged`, empty when it had one, ADR-0067) |
| Routine | The owner's routine, a subscription of Remit that keeps a schedule (ADR-0014, ADR-0064; `model::Subscription` with a `model::Schedule`) whose action starts a chain, one run at a time (`buffer_one`): words he gave once and the time or rhythm at which Remit starts a chain from them: a title, the goal each chain gets (his words, naming a playbook when one fits), the kind of chain it starts (`work`, `chore` or `objective`), when (a cron expression in the server's time zone, or one time for a run that happens once), when it next starts a chain, paused or not, and who made or last changed it. Made, changed and deleted by telling the Assistant (see Routines) |
| Event signal | What one outside system, script or source tells Remit, the signal `event:<name>` (ADR-0024; `model::EventSignal`): a name (the last part of its door's address), what it is for, paused or not (no writer is let in), whether it ships with Remit (`builtin`, `edited` once he changed it), and who made or last changed it. Its door's key is sealed in a column of its own and belongs to no record (see Event signals). Its sensors are records of their own (Sensor). Made, changed and deleted by telling the Assistant; the builtin ones (`gitlab`, `github`) keep his changes and are never deleted |
| Sensor | What senses and writes readings of a signal (ADR-0047, ADR-0049; `model::Sensor`, table `sensors`): a `kind` (`poll`, `script`, or `plugin:<id>.<name>`, a kind an enabled plugin adds, with its `settings`), a name, what it is for, the signal it writes, a source and a connection of it, and either a `query` in that source's words (a poll, read from a cursor) or a script (`language`, `code`) for the `script` source, how often (at least a minute), the cursor of its last read, paused or not, and who made or last changed it. A **standing** sensor has no chain and writes an event signal `event:<name>` (its `poll`); a **chain** sensor belongs to one chain and writes `chain:<id>.sensor.<name>` (its script). Time is sensed by no sensor: a subscription keeps its own schedule (see *The signal pipeline*). `Sensor.Check` is the one place that decides which pairs are valid. A sensor does nothing about what it reads: a subscription to its signal acts. Standing sensors are made, changed, paused and deleted by telling the Assistant; a chain's by its Chair; a chain's end with the chain, and an event signal's with the signal |
| Signal | A key and the readings written under it (ADR-0014, ADR-0049; `model::Reading`): `event:<name>`, `chain:<id>.live`, `chain:<id>.landing.<repository>`, `chain:<id>.sensor.<name>`, `fact:<noun>.<verb>`. A reading has the key's own revision (`rev`), a place in one log (`seq`, which only grows and has gaps), the time it was taken in, what it is about (`subject`) and who wrote it (`writer`: a sensor, the door, the CLI, an agent, a plugin, a fact, a record or a schedule) |
| Subscription | A filter and an action (ADR-0014, ADR-0049; `model::Subscription`): `Match`, a pattern over keys (`*` any run, `?` one character, `[...]` a class, as SQLite `GLOB`; a key with none is an exact key), `Expr` (a CEL condition over the reading), `SeenSeq` (the last place in the log it judged), `Once` (it ends after its first firing), `Flow` (how its readings become firings: `each`, `delay`, `batch`, `last`), `Overlap` (`queue`, `buffer_one`), `Until` (a deadline) and its action: `tell`, `wake`, `start`, `hire`, `end`, `run`, `deadline`, or `plugin:<id>.<name>`, a processor a plugin adds |
| Event | One thing that came in through an event signal: a reading of `event:<name>` (address `event:<name>#<rev>`) holding what arrived (`model::InboundEvent`: the way it came, `door`, `cli`, `poll` or `agent`, its query and headers without the credential, the start of its body, marked when cut), and the firings it caused, one for each subscription that held for it (which subscription, what it did or why it did nothing; `EventRecord`). Nothing else is stored of an event: what became of it is its firings. Kept a week after the chains it touched settle, then forgotten with its reading (see Event signals) |
| Client key | A way in the owner opened for one outside agent (an evaluator): a name, what it is for, the limits he set (chains a day, 0 for none; acts an hour), paused or not, when and from where it was last used, and who made or last changed it. Its key is sealed in a column of its own and belongs to no record (see Client keys). Made, changed and deleted by telling the Assistant |
| Objective | What the owner wants reached (`model::Objective`, ADR-0031): a status (`draft` waits for his Apply, `active`, `closed`), a scope, who the customer is, the outcome, optional guardrails (each a name, the signal that reads it and a condition on its value, ADR-0045), surrogates, horizon and unknowns, the project it belongs to (`project`, none for a request about no product, ADR-0108), an owner and a set of **key results**, a revision it is written over. A key result is a text, who answers for it and who judges it (`Accountable`: the owner or an employee), a **check** of one kind (`submit`: the owner's eyes on the landed change, allowed only with the reason no executable check can exist; `reading`: a signal read over a window meets a condition, until an expiry; `command`: a named command of a repository passes; `unverifiable`: nothing can confirm it, the reason is said and the result stays unverified), a `retired` mark once an amendment dropped it, the key result it refines (`parent`, none for the owner's) and the last lift of its progress rail (`lifted`); key results are rows of their own (`key_results`, ADR-0115). A direct request has a hidden objective of its own (`task`) with one key result. A chain serves one key result, whose status is derived from its chains and findings, or for a `reading` key result from what its signal says (ADR-0045). Of an objective the owner approves, only `reading` and `command` are accepted as checks, and the owner's Apply is refused while a measure fails its dry run on its signal's newest reading. Drafted by the Assistant or a shaping chain, applied by the owner (see Objectives) |
| Project | One product the owner's work is on, the durable context its objectives belong to (`model::Project`, ADR-0108, table `projects`): a name (unique among projects), a status (`active`, `archived`: offered to no one, still read on the objectives that name it), who the product is for (`customer`), the owner's notes on it (`brief`: what it is, where it runs, what matters; what a repository says of itself stays in the repository), its repositories (remotes, `host/group/project`), the connections of sources that are its own (`ConnectionRef`: source and connection), who made or last changed it. A chain's project is its objective's (`chains::project::project_of`), never stored on the chain. A project is context: it grants, narrows and limits nothing. Made, changed and archived by telling the Assistant (see Projects) |
| Employee | An AI employee, kept until the owner retires it: a finished chain retires no one (`model::Employee`, ADR-0032, ADR-0061, ADR-0062): its label, the template and the template's revision it was copied at (a later change to the template does not reach it) and the loadout copied then, a title (`chair`, `judge`, `worker`), a scope in the owner's words, the strengths it states in its own words (`state_strengths`), who it reports to and who hired it (`Accountable`), a number of its own (`E-42`, the next after the highest on record, given in the write that stores it) and a name (the owner's for an employee he hires; its own otherwise, chosen once with `name_yourself`, empty until then: `Employee.Shown` reads "Mai · E-42 (engineer)", its `Tag` "E-42 (engineer)", which a row keeps in view when it cuts a long name), a status (`active`, `retired`) and a ceiling (the strongest effect, a set of grants and a set of Claude Code tools; nothing an employee hires or opens holds more). A card is the employee at work in one chain. Retiring keeps the record and its memory (see Employees) |
| Card | One employee at work = one Claude Code session, whose turn end is its report: its home (the chain it works in, or the Assistant), agent (the id of the Agent it was hired as, when one exists), employee (the id of the Employee at work in it; none on the cards Remit starts for itself), the gate it signs (`signs`: `review`, `qc` or `goal`, fixed when it was hired), its loadout (the instructions, Claude Code tools, library skills and MCP servers its sessions hold: a copy of its agent's taken when it was hired, with the Chair's own choices in place of what it names; the cards the Chair hires hold one, the cards Remit starts for itself none, but the Chair's, which holds a copy of its employee's: the agent in its seat, ADR-0048), name, origin (`chair`, `hired`, `judge`: an agent Remit starts outside the Chair's team; `assistant`: the Assistant's card; `spawned`: a subagent's), for a judge its kind (`judge`: `goal`, `duel`, `second`, set where Remit starts it; its role, which every branch on who a card is matches, is read from the two in one place, `model::CardRole::of`), why it was hired, model, account (the Claude login its session runs on, set when the session starts; see The provider), cwd (fixed when hired, kept when the session resumes), status (`starting`, `running`, `idle`, `permission`, `stopped`, `failed`, or `waiting`: no account can run it now, so its session is closed and its message kept until one can), in status `permission` whom its prompt waits on (`permission_wait`: the Chair, the owner, or the Chair as driver once the time limit passed; set with the status and cleared with it, `Card::ask_permission`), session id, plan, usage, context fill, and while the Chair hands its work to another employee the pending hand-over (`handover`: to whom, why, by whom, until when its note may come; see Permission requests and hand-over) |
| Mentor | A person who helps the AI employees (`model::Mentor`, ADR-0062, table `mentors`): an id (`m_…`), their name (`label`), what they are strong in, in their own or the owner's words (`strengths`), a status (`active`; `removed`: off the pool, their codes stopped) and what they last said of when they can help (`availability`: a note and when they said it). A mentor holds no card and no seat, and works from the codes the owner had made for them (`WorkerKey.Mentor`, ADR-0060). Added and removed by telling the Assistant (see Mentors) |
| Help | One mentor on one card, for one problem (`model::Help`, ADR-0062, table `helps`): its chain, card and mentor, the problem in the Chair's words, a state (`offered`, `accepted`, `declined`, `ended`), why it was declined or ended, when it was offered, its deadline (an offer not answered by then ends), when it was answered and when it ended. Carried in `ChainView` (`helps`) as Decision is, each change pushed (`help_updated`) |
| Edge | Typed link between cards: `handoff` (with artifact label), `hire`, `feedback`, `question` |
| Event | Everything a Claude Code session streamed (message, thought, tool call + updates, plan, usage) plus the prompts sent to it; a prompt is also the message itself, titled with what the turn it starts is about by the code that writes it, `queued` for its card until a turn takes it (`delivered`), its sender puts it into the turn the card is in (`steered`: the Chair, or the owner's Steer, ADR-0080), the owner takes it back into his draft (`card_prompt_withdraw`), or its card's session ends for good (`dropped`); a card's "+n queued" is that queue's count, read with the card and never stored on it. Work outside a turn is an event too: a `job` the server runs for a card (a land's checks), a `task` the agent's own process runs in the background (a Monitor, a background command), each `running` until it ends; a task a closing session stopped keeps why in its data (`model::Cut`, `cut`), beside its words. Its status is typed by its kind (`model::EventStatus`, ADR-0107): one variant per kind, each with its own set (`PromptStatus`, `ToolStatus`, `StopReason`, `SystemNote`, `JobStatus`, `TaskStatus`; a message and a thought have none), so a status of one kind cannot stand on an event of another. The store keeps kind and status as two texts, reads them together once (`EventStatus::read`), and binds both as parameters in every query. A channel reads an event as `EventRead`: the event as stored, and what the server types it as (`Activity`: a tool call, a message sent through a tool, a turn's end, a permission asked or decided, a block, a marked reply, the owner's Start over; `chains::activity`); its JSON keeps `kind` and `status` as two texts (`json_paired!`), which the page's TypeScript pairs, each kind with its own statuses. |
| Session init | What Claude Code says a card's session loaded (tools, MCP servers with status, skills, plugins and their errors, agents, slash commands), from the init message of its newest turn: one per card, replaced each turn |
| Decision | What happened in the chain and why, by whom: its kind is one of a closed set (`DecisionKind` in the contract: seed, hire, route, accept, rework, ask, withdraw, stop, restart, pause, resume, check_back (the Chair's `subscribe`), wake, goal, goal_failed, stall, finish, reopen, archive, unarchive, repo, verdict, landing, playbook, project (a direct request's project set or changed, ADR-0108), agent, library, propose, choose, insight, permission, event, live, client, handover, help (the Chair brought a mentor onto a card, or the mentor accepted or declined), deadline (a mentor did not answer the Chair's call for help by its deadline), duel (a judge's pick between two cards of different lineages on the same brief, which the owner may overturn), learning (the coach shared a lesson with a lineage, asked a Chair for a contest or set the duels' share), workflow (the Chair started a workflow run, with its why), and offer, outside, cancel and dispute, which only stored decisions carry: no path writes them), its actor the owner, an agent, a mentor or Remit |
| Feedback | What an agent tells the organisation about how it works, in its own words: its kind (`practice` how it is asked to work, `difficulty` what got in its way, `complaint`, `suggestion`), its text (at most 2000 characters), and who sent it (home, card, agent, model). Every agent holds the tool `send_feedback` (the Chair and the Assistant too), at most 10 pieces from one card. The owner reads it in the Feedback tab, whose count is what waits to be read (`readAt` unset); opening one reads it (`feedback_read`). A chain that holds the `feedback` capability reads it with `list_feedback`. Nothing changes a brief or a playbook by itself: that is the owner telling an agent. Never pruned with its chain |
| Change | One write of a record that moves a rail or that the owner keeps (`ChangeKind`: agent, skill, playbook, grant, event signal, extension, objective, employee, forget, project): the record before and after, what kind of write it was (created, changed, deleted), the fields that differ, who wrote it and who let it through when it waited for the owner (see Agents and the library). Stored in the transaction that stores the record and never changed; a write that changes nothing, and a start that finds a built-in as it ships, leave none. A record written before changes were kept has none. The pages of these records show them (`changes_get`: fields only, never the records) and the Chair and the Assistant read them (`list_changes`, `read_change`) |
| Actor | Who did something: the owner (`local` for the holder of the owner key, his Google address when he signed in with Google, and the channel), an agent (its card), the Assistant on the owner's words (its card), an outside agent holding a client key (the key), a mentor on the card they help (`ByMentor`: the help) or Remit itself. Recorded on the decisions, prompts, landings and answers an action causes |
| Assistant | The owner's agent for the whole app, in no chain: one record, its one card and that card's stream, which is the conversation. Its id is the home of its card and of what is recorded for it (events, drafts, inbox items) |
| Seen mark | What the owner has viewed (ADR-0079): one row per subject (a chain's or the Assistant's home, or `assistant.feed`) holding the newest event he viewed there and when; moved only forward, by `seen_mark`, which a page calls only while visible, focused and showing the newest; deleted with its home. It marks a wait and a finished chain unseen and gives a chain's "While you were away" (`ChainView.away`: its decisions since the mark) |
| Retrospective | What a finished chain should teach, written by the coach (ADR-0082, `write_retrospective`): the issues with their impact in words, each memory its agents recalled marked helped or misled with why, an improved brief the owner may start a chain from, and the change item of the lesson the coach proposed, if one waits; one per finish, carried in `ChainView` |
| Request | An action a channel asked for, kept with its first answer for a week, so the same action asked again is done once (see Channels) |
| Approval | A judge's verdict (`Verdict`: `approve` or `request_changes`), signed as its role (`VerdictRole`: `reviewer`, `qc` or `security`, the gate it closes, `judge` for the goal judge): a reviewer's or QC's on one version of a repository's change (its patch id), or the goal judge's on the chain's result, with a finding per key result of the goal (the key result it rolls up to, the item checked, `met`, `not_met` or `unverified`, its evidence, and when it objects a closed reason code and the platform records it cites). A record of its own, kept for every round of review; channels see it through the chain's verification (see How results are checked) |
| Landing | One version of a chain's change on one repository on its way onto the base, and what became of it (`model::Landing`, ADR-0114, table `landings`): its chain, its repository, forge host and path and the base it lands on (together its queue, `QueueKey`), its version (1 for the chain's first on that repository), the worktree commit it is (`from`), where it left the base (`forked`), the stable patch id its verdicts are of (`change`), the gates it needs and the verdicts that approved it, the employees whose accepted work makes it up (`by`), the commit's title and description, the Chair's priority and why, the paths it touches, the judgments a decider made of its change and the effort they raised its change reviewers to (`judged`, `effort`), its stage plan when it is a stage of a canary, the landing it takes back when it is a revert (`reverts`), a state (`judging`, `contending` for an attempt's version waiting on its round's pick, `ready`, `combining`, `checking`, `landed`, `returned`, `reverted`), the batch it was combined in, the base commit that batch was combined on (`onto`), the commit it became and the push that carried it, the forge's checks on that push and on the base before it, why it went back to its Chair (`returned`: `conflict`, `checks`, `empty`, `replaced`, `refused`, `withdrawn`, `not_picked`, with the output that showed it) and what took it back (`reverted`: cause `pipeline`, `guardrail`, `overturn` or `conflict`, the revert's landing, who and why), and when it became ready and landed, the release that carried it (`release`, ADR-0116), and, for one carried over from a merge request by migration 0119, that MR's URL (`reference`). A chain lands without bound, so it is an entity of its own, carried in `ChainView.landings` and pushed by `landing_updated`; it is kept after its chain is deleted |
| Round | One round of competing attempts at a key result whose check is a command (`model::Round`, ADR-0115, table `rounds`): the key result, its number (1 for the key result's first), and its state: `open` while its attempts (the chains whose `attempt` it is) work and another may join, `judging` with its judge's card, `picked` with the judge's card, the attempt (chain) picked, the judge's reasons and the owner's overturn when he made one (its decision and his word), or `ended` without a pick, saying why. A key result runs rounds without bound, so a round is an entity of its own, its id derived from the key result and its number |
| Release | One run of a repository's release procedure on a landed commit (`model::Release`, ADR-0116, table `releases`): its repository, commit and the commit released before, its state (`releasing`, `baking`, `live`, `unchanged`, `taken_back` with who and why), the version and links it reported, its steps and way-back steps with each one's exit and output, the guards it bakes under, and its bake. Its landings name it (`Landing.release`). Written by the release runner (`crates/shipping/src/release_run.rs`) alone, through a change's `Records::releases`. A repository's release pause (`model::ReleasePause`, table `release_pauses`) is who pulled it and why, and once lifted who lifted it and why. The repository's newest (`RELEASES_SHOWN`) with their landings, and its pause, are its Releases view in `RepoView` (`repo_get`, pushed whole as `repo_updated`) |
| Draft | An unsent message to a chain agent or the Assistant. Remit saves its words and attachment references as the owner writes. Each upload's bytes are stored once in the draft's home. The sent message keeps those references and the runtime supplies inline images and read-only files to the agent (ADR-0130). Sending or deleting the draft ends it |
| Inbox item | Something waiting on the owner: an agent error with retry (stored with it, so it outlives a restart; retrying queues the failed message again and deletes the stream's records of the failure; the agent's next good turn closes it), its title one sentence the server writes from the failure's typed cause, naming who and why, its detail what the owner can do and then the raw text (`failure_words`, ADR-0081); a permission request a Chair passed up (`escalate_permission`), or one of a session with no Chair above it, closed when anyone decides it; a `question` item is the Chair's `ask_owner`, in either mode: a decision that is the owner's, or the way past a block from outside the chain (the chain stays open until he answers); a `credential` item asks for the value of one variable of a source's connection (`source`, `connection`, `variable`, `secret`), answered in a field of its own: a secret is typed masked, stored sealed and shown as `set`, a plain one (an address, a user) is stored as typed and shown on the Tools page; the owner may also answer Remove the stored value or Remove this connection; a `change` item asks the owner to let through a write that moves a rail, answered Apply or Not now by the owner himself (see Agents and the library); an `exception` item is a key result that could not be served or judged as its objective said, and waits for his decision (see Objectives) |
| Playbook | Reference content on how a kind of work goes well (`model::Playbook`, ADR-0113): a name, a description, a brief (its text, read by whoever chooses to), the agents it suggests (`seeds`), a group, the kind of work it is for (`engineering`, `data`; lowercase words, `general` when none), which is its column on the Playbooks page, and built-in and edited marks; a built-in's group is code. It gives a chain no tool, no Chair, no gate and no slot, and binds no one: every Chair lists and reads playbooks (`list_playbooks`, `read_playbook`), a starter who wants one followed names it in the goal, and a chain of kind `author` writes them |
| Agent | A template the owner keeps, the definition an employee is hired from (see Agents and the library): id, name, description, group (the kind of work it is for, its column on the Agents page; a built-in's is code), default model, its loadout (instructions: what every session of it starts with, appended to the system prompt, a brief being the task a card is given; the Claude Code tools it may use, absent: every tool, empty: none; the library skills it holds), the gate it signs, where it was imported from, built-in and edited marks. A hire takes a copy of its loadout and makes an Employee; an id with no definition is a plain agent |
| Skill | Instructions an agent loads when its work calls for them: a name, a description, a group (the kind of work it is for, its column on the Skills page; an agent regroups it with `update_skill`, a move that waits for no one), its files (text), where it was imported from. The library keeps the one copy; Remit renders it into a plugin directory for Claude Code |
| Source | A set of tools Remit writes (`crates/source`) that read one outside system, under a capability (`data`, `observe`, `customers`): id, label, tools (name and what each answers) and the variables it reads (`Vars`). A source that can also be reached with a program says which one and which commands may run (`Programmed`, `ProgramSpec`). It is code, never stored |
| Connection | A source's name and description for one set of credentials (`source_connections`; `default` is served by the process environment and env files, stored values first) and the values the owner entered for it (`source_values`: a secret sealed, a plain one as typed). A tool takes a `connection` argument and answers `connection: <source>/<name>` first. The owner may mark a connection protected (`source_connections.protected`): no chain reads it without a grant |
| Grant | The right of one holder, a chain or a repository (for its release, ADR-0116), to act on one connection of one source with an effect (`read`, `reversible`, `compensable` or `irreversible`, ADR-0027): a row of `grants` with who gave it and when (`model::Grant`). A chain also holds, never stored, a read on every unprotected connection of the sources whose capabilities it holds (see Outside systems) |

## How a chain runs

1. A chain is created by the owner, started from one of his routines when its time comes (see Routines), or proposed by
   another chain that holds the `propose` capability (a routine's chain the owner set up to look for work; `propose_chain`).
   A proposal is deduplicated by the key result it serves (ADR-0115): it names one, or serves a new key result refining
   the proposer's own, and one that is met, retired or already has a start queued takes none (the refusal names that
   chain, whose Chair the proposer messages instead).
   The owner asks for one in his own words, in one box: they become its goal. Asked through the Assistant, the goal is
   the one it writes from his words and the context it holds (who it is for and what they get, its scope, how to tell it
   is done), deciding what he meant without asking him. He names nothing else: his box starts a `work` chain, and the
   Assistant names the kind the work is (`create_chain`: `work`, `chore`, `objective`, `author`, `coach` or `trial`;
   `analyst` and `librarian` are Remit's own, ADR-0113). The chain shows at once, queued under the first line of his words,
   with its kind's capabilities (`start_from`, `capability::kinds::capabilities_of`); who may start which kind is one rule,
   `capability::kinds::may_not_start`. A playbook he or the Assistant wants followed is named in the goal, and the Chair
   reads it. Nothing chooses for it before it starts: its Chair,
   in its first turn, titles it (`set_title`, a `title` decision the owner sees) and chooses its project among the active
   ones, or none, unless the owner or the Assistant named one (`set_project`, see Projects). A big or half-formed request
   is shaped by a chain of kind `objective` (see Objectives). Every chain's directory is one
   Remit makes for it (`<data>/work/<chain>`); a directory the owner mentions in his words is the agents' to work in, as a cwd.
   A proposed goal states observations; its suspected cause and fix idea are unverified hypotheses. The Chair, the
   agents it hires and the goal judge see it headed as another chain's (`goal_heading`), so a team that proves another
   cause has met the goal.
2. The scheduler offers admission the waiting chains whose wake subscription is due (the signal tick fires them before
   the queue starts), then queued chains, oldest first.
   Starting gives the chain its Chair and the Chair its first message in one step. A chain's status
   changes only through one pure transition function (`step`, `crates/chains/src/step.rs`) applied by one writer per
   chain: the new state, the decisions the owner sees and the messages it sends are stored in one transaction, so no
   crash, restart or closed tab leaves a chain half changed. Every entry to running passes one admission rule
   (`admit`, `crates/chains/src/admit.rs`): the entries are the queued start, the
   scheduled wake, the owner resuming or reopening a chain, his message waking a waiting chain early and his checking back
   now; the limit holds only what the scheduler starts on its own (queued starts
   and scheduled wakes), so the owner's own actions and an early wake never wait for it. The limit is measured, not set
   (ADR-0115, `capacity`): it starts at the chains running when the server starts (at least 1), grows by one each Tick
   while it is full, a chain is queued and nothing presses, and halves under pressure, never below the chains running, so
   it holds what would start and stops nothing. Pressure (`Pressure`) is any of: every account near a limit its provider
   warned of, a turn whose account failed and whose card moved on since the last Tick, the host's free memory below twice
   what one session holds on average (the server's descendant processes over its open sessions), or its last minute's
   load above its cores. The workload pill shows the limit and what presses on it, and a start it holds says both.
   The limit is shared by key result (ADR-0115): a key result's chains hold at most its share at once (`share`: 1 to begin,
   one more for each landing of its work that lands and is not reverted, halved, never below 1, for each reverted or
   returned for a conflict or a failed combined check), a key result whose landings waiting in queues (`ready`,
   `combining`, `checking`) already number its share takes no new start (back-pressure: its work waits on the queue),
   the queue starts key results with nothing running before a second chain of any (`queue_of`), then the objective with
   the soonest horizon, then the oldest, and a stalled key result takes no new start (see Objectives). A wake is charged
   to its chain's key result and never refused by its share; a chain the owner asked for is held by neither.
   The share is how the merge queue paces new work (ADR-0114, ADR-0115): no count of waiting changes stands in for the
   queue's measures.
   A key result whose check is a command may be served by competing attempts (ADR-0115, `shipping::round`): whoever
   answers for it opens each with `propose_chain` and `attempt` set, joining the key result's open round (`round::joined`,
   a new one once a judge has started on the last) up to its share (`round::attempt_refusal`; a reading check is refused,
   since its attempts cannot be compared before one lands). Each attempt works in its own worktree and passes its gates
   as any chain, but its approved version contends instead of entering the queue (`round::approved_state`, the one rule
   `land::approve` reads). Once every attempt has a version contending or has finished (`round::standing`, read when a
   version contends and at each landing sync), Remit starts a round judge on the judges' model in the first attempt's
   chain: it runs the check on each version's commit, reads them on the customer's side and calls `pick_attempt`, a `pick`
   decision on the picked chain. The pick enters its queue; every other version goes back not picked
   (`round::picked`, `ReturnReason::NotPicked`), each Chair hears why, and the judge's reasons stay on the round, where the
   next round's attempts (after their goal) and judge read them. A judge that gives no pick ends the round, and every
   version contending in it goes back. The owner overturns a pick through the Assistant (`overturn`): the picked version
   is sent back while it waits in its queue or reverted once landed, and his word stays on the round. An attempt not
   picked is no chain without progress when its round's pick landed (`admit::outcomes_of`). The held chain's board card says why it waits, and so does its Overview. What the
   scheduler starts on its own also needs the provider to be able to run agents (see The provider): while it cannot,
   queued starts, scheduled wakes and routines' runs are held, and the owner's own actions go through (the provider
   may be back before the next check, and when it is not, the failure is his, with retry). The last room on the accounts
   is kept for the owner (ADR-0092): while every account that can run agents is near a limit its provider warned of (90%
   of a window, see The provider), queued starts he did not ask for, scheduled wakes, a workflow wait's firing and
   routines' and events' start firings are held the same way, on the Subscriptions tab with each account's limit, while
   his own actions, the Assistant's turns and chains already at work go on. A
   chain the owner waits on takes no slot either: a fact of its kind (`ChainKind::owner_waits`: `objective` and `author`),
   shown in the queue (chains the owner asked for and watches).
   A running chain where nothing has moved for 10 minutes has its Chair reminded to move it on; after 3 reminders with
   nothing moving in between, Remit finishes it so it frees its slot. Only a chain waiting on the owner's answer
   (a question, a permission request, a change that moves a rail) is spared: an agent's failure in the inbox is the
   Chair's to deal with too. What agents run in the background is not progress: the reminder names it and points to
   `subscribe`, which frees the slot. Nor is an agent silent in its turn (below).
   While no provider can run agents, no chain is reminded or finished for standing still: it is the provider that stands still.
   An agent in a turn keeps sending its session's updates (its words, its tool calls and their progress), even through a
   long tool, so one that has sent nothing for `-turn-idle` (30 minutes; 0: never) is taken for wedged, its process or its
   connection hung, and would otherwise hold its card running, and its chain's slot, for good (`cut_silent_turns`, at every
   tick). Its turn is cancelled, and a session still in that turn a minute later is closed. Either way the turn fails, in
   the owner's inbox with a retry, unless the agent ended it on its own as the cancel was on its way. The clock is the
   time the agent was last heard from, not the turn's length: a turn may last hours. A turn waiting on a permission's
   answer, and every agent of a paused chain, are left alone. This holds every agent's turn, a turn it takes on its own
   news included.
   An agent that keeps talking can still loop: the same command for the same answer, the same error after each try. The
   repetition rail (ADR-0092, `repeats.rs`) prints each finished tool call of a turn from its tool, input and result, with
   every word holding a digit masked (times, ids, counters), keeps the last 8, and takes the same call with the same
   result, or the same error, `-turn-repeats` times (3; 0: never) among them for a sign. The first sign steers a message
   from Remit into the turn ("You are repeating yourself": try another way, or say what blocks you); the second tells the
   agent to make no more calls and end the turn with its report (what it did, what it tried, what is left), which goes on
   as every report does; the third call after that cuts the turn, which ends cancelled with what it said as its report.
   Only calls after a sign count, a new turn starts from nothing, and a subagent's calls are its parent's turn.
   A chain can also keep busy across turns and produce nothing: a report sent back, reported again, the same error met
   once per turn. The no-progress rail (ADR-0104, `spin.rs`) counts the turns a running chain's cards took (`Card::turns`)
   since it last produced something: a plan step completed, or a decision that is a product (accept, verdict, duel,
   goal, landing, live, insight, propose, release, learning; hires, routes, reworks and waits move work and do not count).
   Every `-spin-turns` (10; 0: never) such turns the Chair is reminded on the stall rail's path ("Nudged for going round
   in circles", n of 3), whether or not a card is at work, and past the third the chain is finished as stalled. Something
   produced starts the count again, a chain that stops running is forgotten, and its marks live in memory, so a restart
   starts every count again. The stall rail's spares hold.
3. The Chair starts with the goal, its kind's section of the brief (`capability::kinds::brief_of`: for `work` and `chore`, that playbooks are reference it reads with `list_playbooks` and `read_playbook` when one fits and that none binds it; a system seat's brief for the others), its project (see Projects), the owner's principles and the list of other open chains; a
   Chair whose goal another chain already covers finishes at once. Its MCP tools: `hire_agent`, `accept_work`,
   `route`, `list_agents`, `read_output`, `stop_agent`, `open_repo`, `land`, `revert_landing` (of its own chain's landings), `ask_owner`, `withdraw_question`, `subscribe`, `finish_chain`, `update_plan`, `send_feedback` and `show_block` (every agent has them; a block, ADR-0091), `answer_permission`, `escalate_permission`, `hand_over`, `list_mentors`, `bring_mentor` (see Mentors), `set_project` (see Projects), `run_workflow`, `list_workflows`, `stop_workflow` (a script Remit runs, below), the reads of agent definitions, the library and the playbooks (`list_agent_definitions`, `read_agent_definition`, `list_library`, `read_skill`, `list_playbooks`, `read_playbook`), what every employee holds
   (`my_work`, `read_my_card`; see Memory), plus those of the chain's capabilities (below). `finish_chain` is checked by the goal judge (see How results are
   checked). A check that cannot run (the store unreadable, a refusal that cannot be sent) refuses the finish and
   tells the Chair. Pausing a chain holds its work in the background too: a land in progress stops, decides
   nothing, and the Chair is told when the chain resumes; the goal judge's turn ends, and it goes on when the chain
   resumes; the sessions of agents running background tasks close, ending them, and the Chair is told to have them
   start again what they still need.
4. `hire_agent` starts one Claude Code session per card. Workers get `ask_chair` and `report`. A turn answers whoever
   opened it (ADR-0100): when a turn the Chair opened ends, the worker's final message is its report, forwarded to the
   Chair, which drives the Chair loop; a turn a person opened (the owner, the Assistant, a mentor) ends in the worker's
   reply to that person alone, and the worker tells the Chair with `report` when the work moves. Claude Code's Stop hook
   keeps a worker in its turn until its plan is true, and until a person's turn that completed the work is reported. Every message to an agent is its prompt event, stored
   `queued` with the change that sends it (the owner's words as he wrote them, marked as his in its data; the
   "[message from the owner]" line the agent reads them under is written when it reads them, `outbox::read_of`) and taken `delivered` when the agent's turn on it starts (opening message
   first, then the owner's, then the rest in order), so none is lost to a crash or a closing session; a restart hands
   over what is still queued. A help outlives a restart and a session closing between turns: only its card's work ending
   ends it. A card whose session a restart resumed 3 times in a row without its turn ending is parked
   instead (failed, "parked after repeated restarts", in the owner's inbox with retry), so a turn that brings the server
   down cannot do it again at every start. What the Chair sends an agent that is in a turn (`route`, `accept_work`, a permission
   request) is read in that turn, not after it, which may last many minutes: it is put into the running turn as a user message
   Claude Code reads between its steps (priority `now`) and stored `steered`, and no turn starts on it. A message that cannot be put there waits its turn
   like any other: no turn is running by then, the agent cannot steer, or older messages are queued ahead of it, which it
   must not pass. The owner's messages and Remit's own wait for the turn, as before. A hire stores the card, its brief and the handoffs drawn to it together. A turn's end and the report it sends are stored together. A worker's report asks a decider two questions against the card's brief before it goes (the watchdog, ADR-0118, point 6; `chains/src/card_watch.rs`): `card.progress` (did the turn move the card toward its brief?) and `card.off_brief` (did it do work the brief did not ask for?). An answer that reads the card as drifting and is at least as sure as its bar (the question's act threshold, 0.9 until a decider has 30 results) is a line at the foot of the report's own message, "advice, not a verdict"; every other answer is kept as held. Nothing is paused, stopped or refused, and with no decider, a late one or a refusal the report goes as it always did, waiting at most 2 s for all of it: one deadline around the whole asking, however many deciders are tried. Each answer is recorded on the report it is about (the card keeps the id of its last report), with the bar it was held to and whether it was shown. The Chair's verdict on that report scores both answers, both ways (an accepted report was moving and on its brief; one sent back was not), and only the answers about the card's last report: those about one it replaced before a verdict are left without an outcome. A worker whose session failed to start
   keeps its messages for the owner's retry and holds nothing up. Before hiring, the Chair reads `list_agents`, which shows
   each agent's status, why it was hired, cwd, context fill (`ctx_used` of `ctx_size`) and last report, and the standing
   employees it may put to work: the agent each was hired from, its scope, the strengths it stated, the cards it holds open
   in every chain, its last five cards judged, accepted or sent back, each naming the work it was and how long it took, and
   the gist of its memory. Work
   that continues an agent's own goes to it with `route`; a stopped agent resumes its own conversation, in the cwd
   it was hired with. When the Chair hires anew although related work exists, it says why in the hire's `why`,
   which the owner sees in the decision log.
   A subagent Claude Code spawns inside a card's session is a **child card** (ADR-0073, `crates/chains/src/spawn.rs`): on its
   `task_started` the card of the employee its type names (`general-purpose` and `fork`: the spawner's own; an employee's
   handle: that employee) is made, origin `spawned`, `Card.Spawn` binding it to the spawner's process, with a `hire` edge from
   the spawner, in one change; a nested spawn is a child of the child. What the subagent streams, the tools it calls and the
   permissions it asks land on its own card (a permission goes up its parent's ladder); the parent shows the spawn as its tool
   call. Its `task_notification` ends it: done with its report as its output, failed, or stopped. A type the session was not
   offered is stopped at once (`stop_task`) and noted on the spawner. A child holds no session of its own: no account, no
   silent-turn cut, no queue; a message to it is refused ("it runs inside <parent>: talk to <parent>"). The owner's stop of a
   child is a `stop_task` in its parent's process, whose turn goes on. When the parent's process ends (pause, stop, a wait, a
   failure, a restart) every child still at work ends stopped, saying why; a child is never resumed, and the parent, resumed
   after a restart, is told which children were cut.
   A run of Claude Code's own `Workflow` tool in a card's process is a `Workflow` of engine `claude`
   ([ADR-0073](adr/0073-a-subagent-is-a-standing-employee-at-work-and-a-workflow-is-a-script-remit-replays.md),
   `crates/chains/src/workflow_claude.rs`): its script and the handle that resumes it (`runId`, `scriptPath`) are recorded, its
   agents are its steps as its progress names them (label, phase, state, the start of their result), and its end ends it.
   Claude Code keeps its journal. When the card's process ends the run is stopped, and the card, resumed, is told the call
   that resumes it, `Workflow({scriptPath, resumeFromRunId})`. The owner's stop is a `stop_task` in the card's process.
   **A Chair's script is a workflow Remit runs** (layer 2, ADR-0073, `crates/workflow` and `crates/chains/src/workflow_run.rs`).
   A Chair (or a routine, whose chain begins with the run and whose Chair takes its first turn when the run ends) starts a
   run of a script written for Claude Code's Workflow API, with Remit's own calls besides: `hire`, `wait`, `askOwner`,
   `accept`, `sendBack`. The engine is pure: it replays the script from its start against the journal of its steps' results,
   in the order they arrived, and says which calls have no step yet, or the result, or why it failed (a divergence from the
   journal, a clock read, 2 seconds without a call). The shell advances a run in the change that records one of its steps'
   results (a card's report, a wait's firing, the owner's answer) and applies each new call there, through the path the
   Chair's own tool takes: `agent()` and `hire()` hire as `hire_agent` does (an `agent()` naming no employee puts an idle kept
   worker to work before it hires a new one), `wait()` is a subscription of the chain whose firing resumes the step,
   `askOwner()` an inbox question whose answer is the step's result, `accept()` and `sendBack()` `accept_work`'s verdict. A
   card a step put to work reports to the run, not the Chair: `agent()` accepts it, and with a `schema` reads the report's
   last JSON value, sending a report that does not match back once before the call fails. The Chair takes no turn while its
   run works: it is told the run's result, or why it failed, when it ends. A chain whose runs wait only on a time, a signal
   or the owner, with nothing else at work, waits: its sessions close and it holds no slot; a wait's firing is a scheduled
   wake (admission). Pausing the chain pauses its runs (no call is issued) and resuming advances them; the Chair cannot
   finish while a run is open; finishing the chain stops them; the owner's `stop_workflow` stops a run in one change: its cards
   at work stopped, its waits ended, its question withdrawn, recorded with who and why.
   The Chair's tools for it (`crates/chains/src/workflow_tools.rs`): `run_workflow{name, script, args, resume_from, why}` starts a
   run (refused until the chain has a plan) and records a `workflow` decision with the why; `list_workflows` shows every run
   of the chain, of both engines, with each step's call, label, phase, status, card and the start of its result;
   `stop_workflow{id, why}` is the owner's stop with the Chair as who stopped it. Its brief says when to script: step by step
   when each next step needs its judgement of the last report, a script when the work's shape is known before it starts
   (fan-out and judge, verify each, a loop until a check passes, a wait between steps); every Chair holds the built-in skill
   `workflows` (the API, the replay rules, three worked patterns), and the Assistant holds it for a routine's script. Workers
   are told they may fan out inside their own turn with Claude Code's Workflow; gates are not. Of `agent()`'s options Remit
   applies `label`, `phase` (the step's, and named in the card's why), `schema`, `agentType` and `model`; `effort` is
   accepted and said on the hire's why and the step, not applied (per-session effort is ADR-0070, not built); `isolation` is
   refused, since a chain's agents share its one worktree; `workflow()` fails: Remit keeps no saved scripts.
5. `open_repo` names a repository by its remote (ADR-0038), clones it into `<data>/repos/<host>/<project>` the first time and creates a worktree from that clone on branch `feature/mc-<chain>-<slug>`. Every session commits there.
   A short name (`api`) is looked for first among the repositories of the chain's project, so it opens that product's
   `api`, cloned on first use; then among the clones Remit holds. A chain may open any repository (ADR-0108).
   A directory already at that path is taken over only when it is the chain's own worktree (on its branch, which carries
   the chain id); anything else is another chain's or the owner's, and `open_repo` refuses it.
   The Chair hires each agent that changes, reviews or verifies code with the worktree path as its cwd; an agent that
   only investigates data keeps the chain's directory. `land` (`crates/shipping/src/land.rs`, ADR-0114) puts a version of
   the chain's change on one repository on its way onto the base, in the background, one at a time per repository; the
   Chair hears the result as a message starting `[land result · <repo>]`. It commits what agents left, as the account
   the forge pushes as; refuses a worktree that holds nothing beyond the base, or that carries commits already on another
   branch of the repository (a release or hotfix); refuses forbidden paths (CI configuration, `.remit/checks/` and `.remit/rails`, secrets:
   `forbidden_path`; `.remit/release/` is written by chains, a rail by code); runs build, vet and tests of a Go module and then the checks the repository declares in
   `.remit/checks` (each in a jail, ADR-0025); and records the version as a Landing: the worktree commit, where it left the
   base the forge names, the stable patch id of that change, the gates it needs (`required_gates`), the employees whose work
   the Chair accepted with their cwd in the worktree (`by`), the Chair's title and description (the landed commit's
   message) and the paths it touches. Nothing is pushed: it waits, `judging`, until every gate it needs approved its patch
   id, and is then `ready` in its repository's merge queue (see Shipping). A newer version replaces one of the same
   repository still judging or ready, which goes back with reason `replaced`. The land is recorded on the repository
   (`RepoWork.submit`) before its checks start, so a crash or server restart runs them again and the Chair is told once;
   pausing, finishing or deleting the chain stops it (see 3). A chain a client key started lands nothing (its work stays
   in its worktree), nor does a trial's.
   The Chair rates each change when it lands it: `priority` (high, medium or low, by what waits on it or what breaks
   if it is wrong) and `priority_why`, one line for the owner. `land` refuses a change without both before it does
   anything, each new version is rated again, and the rating rides with the landing into the decision that records it. It
   orders what the owner reads, never the queue.
6. Permission requests. Claude Code's auto mode asks about nothing on its own, so every agent starts with ask rules
   (`ask_rules`, `crates/chains/src/policy.rs`) for what reaches outside the chain: deploying, deleting, writing to shared
   systems (`kubectl`, `helm`, `aws`, `glab`, `curl` and `wget` writes) and reading credentials and secret paths. A
   request for one of them reaches Remit in any mode. A call auto mode's safety classifier refuses (running outside
   Claude Code's sandbox, say) comes back as a request too (`crates/agent/src/refused.rs`, ADR-0083): the session leaves
   auto mode, the model retries the call, which is asked over `can_use_tool` with the classifier's reason as why it
   asks, and the session is back in auto mode at the first request after that or when the turn ends; a call refused
   again in the same turn stays refused. Every tool call also meets the `PreToolUse` hook before it runs, in every mode
   (ADR-0084): once the core cancels a session's work (a pause, a stop, the owner's interrupt) it starts none until it
   next works, rests or takes a prompt, and a hook it fails to answer refuses the call. A cut that lands after a turn's
   message is taken and before it is sent holds too: a pause puts the message back in the queue with the card at rest,
   and any other cut ends the turn without sending it. A call that read, edited or searched a file or ran a command
   meets the `PostToolUse` hook after it, and a subagent the `SubagentStart` hook as it starts (`agent::Handler::on_moment`,
   relayed from the runtime by `session_moment` once the core has every entry before it): the core answers with what the
   agent's memory holds about that moment (`chains::recall`), within 2.5 s, and the agent waits 3 s at most before it
   goes on with nothing added. Settings change only between sessions (`ConfigChange` is refused, and
   `.claude/settings*.json` of every worktree is read-only), and a worktree is Remit's to make (`EnterWorktree` and
   `git worktree add` are denied; a tree at another commit is a `--shared` clone in the scratchpad). What Claude Code
   does to a session beside the agent's work, a retried request, a compaction, an instructions file read, a model it
   switched to, shows as a note in the agent's stream; a compaction that succeeded (a note of its own kind, apart from
   one that failed) also lets the session be given again the memories its summary may have dropped. The rules are a tripwire, not a sandbox: a command that
   reaches the same place another way is not caught. What a session cannot reach at all, Remit's own secrets, is
   settled by its isolation (see Outside systems), and the gaps that remain are in the known gaps below. A request meets,
   in this order:
   1. The deny rules (pushing, adding or re-pointing a remote, `glab mr` and `gh pr` writes, a `glab api` or `gh api` call
      that names the endpoint or mutation that merges or approves, whatever its method and flags, a reviewer's or QC's file edits) are enforced by
      Claude Code in every mode, so no request for them exists and nobody can allow them. A request that does reach
      Remit is refused by the same rules (`denied_by`, which reads `*` as Claude Code
      does) before the Chair or the policy sees it.
   2. The rails stay code: authentication and pause and stop; what reaches a base is the merge queue's, which takes only a
      version every gate it needs approved (ADR-0114). No answer lifts them; the owner, or
      the Assistant on his words, pulls pause and stop through the same action.
   3. A request that names a secret (Remit's own keys and database, `.env`, credentials, tokens, `.ssh`; `refuses_secret`) is
      refused by code before anyone is asked, in every mode; nobody may allow one (ADR-0059).
   4. Every other request goes up one ladder, in both modes (`ladder_of`, `crates/chains/src/permission.rs`, ADR-0059). The chain's
      Chair decides its team's requests: it reads the request at once (steered into its turn,
      ahead of what else waits for it) and answers with `answer_permission`, allow it once or refuse it with a one-line why, or
      passes it to the owner's inbox with `escalate_permission` when it is his to decide. The Chair is the authority over its
      agents' work: it allows what serves the goal, even a command that touches a shared system, and refuses only what does not
      serve the goal. A session with no Chair above it (the Chair's own, a judge's, the Assistant's) asks the owner first.
   5. When nobody has decided within the owner's time limit, measured from the ask (`Settings.PermissionWait`: 30 minutes until he
      tells the Assistant another, `set_permission_wait`; a request keeps the limit it was asked with), the Chair above an AI
      employee's session becomes its driver and decides alone ("is now yours to decide"), and a request of a session with no
      Chair above it is refused by the rule: it was the owner's from the ask, so the Assistant had it all along, and the
      asker never decides its own (ADR-0059, ADR-0087). Whom a request waits on (the Chair, the owner, the Chair as driver) is
      stored on the asking card (`Card.permission_wait`, `model::PermissionWait`), which the agent's line reads. A mentor
      reads a prompt of the session they help and its answer, and never decides one.
   6. The first answer wins; a later one is refused, and the owner's inbox item closes when anyone decides. One
      `permission_answered` event names who decided and why, and the stream labels it by who: Chair, you, the Assistant for you,
      a rule (and, on a request an earlier build let a session decide, the session itself).
   Only Remit's backend pushes, and only the merge queue's fast-forward of a checked batch to a base (see Shipping). The
   login the forge's command line works as on the server (`glab`, `gh`) is the account the queue pushes as, so the
   operator gives it, once per GitLab group or GitHub repository, the right to push to the base of every repository a chain
   may land on, and nothing more:
   1. GitLab: the base's protected-branch rule lets that user (or its role, or the group or project access token) push,
      with force push off.
   2. GitHub: the base's ruleset holds that account in the bypass list of "Restrict updates", and "Block force pushes" stays
      on for everyone.
   3. Check it: as that login, a fast-forward push to a test repository's base goes through and a forced one is refused.
   A push the forge refuses sends each landing of the batch back to its Chair (reason `refused`, in the forge's words) and
   asks the Assistant what to do about the base (`refused`, `crates/shipping/src/queue.rs`). The deny rules are a tripwire,
   not a wall: a script, or a raw HTTP call with the login's token, matches no rule, and sessions share the server's OS
   user, so a session that reaches the machine's forge login could push (`docs/remit-design.md` K2).
7. An agent's Session panel shows what fills its context, and its context meter opens the panel there (`card_context`,
   read while the panel shows and again each turn): what Remit gave it (launch spec, its tools, capabilities,
   deny rules, opening message), what Claude Code loaded (the session's `system/init` message; every agent loads the server
   user's own Claude Code setup), and tokens by part
   estimated as characters / 4 from the session's transcript (`<claude config dir>/projects/<cwd>/<session>.jsonl`, read
   from the AI runtime that writes it, a part of at most 512 KiB at a time: `read_session_transcript`),
   next to the fill the agent reports. Each part of the transcript is read on its own: one it has no record of is shown
   as not available.
8. The owner reads what a chain lands on its page, live (ADR-0114): the chain's landings (`ChainView.landings`, oldest
   first, each change pushed by `landing_updated`), each version's state, its verdicts, the batch and checks it passed or
   the output it went back with, the landed commit, the forge's checks on it, and a revert with who and why. A landing's
   diff is Remit's own, read from its clone and never asked of the forge (`landing_diff`, `crates/shipping/src/landing_view.rs`:
   the files from where it forked to the version the gates approved; a revert's is its target's change backwards). Each
   repository's queues are a view the server lays out (`RepoView`, `crates/orch/src/repo_view.rs`, read by `repo_get` and
   `repo_views`, pushed whole as `repo_updated`): for each base, the batch, then what is ready in the order the queue
   takes it, then what landed, went back or was taken back in the last day, newest first; a version still judged is not
   in it. He comments by writing to the Chair; he takes a change back by telling the Assistant,
   or the chain's Chair on its screen, which is `revert_landing` (see Shipping). No landing waits for him, and nothing in
   his inbox asks him to land.
   What waits for the owner is one list, `Board.waiting` (ADR-0078, ADR-0087, `crates/orch/src/waiting.rs`): the open inbox
   items only he can settle (a credential or a sign-in code to type, or an item nobody is asked to decide for him), the
   longest-waiting first. The board's "Needs you" column holds the chains with a wait, in the order of their first wait,
   and the loose items. A card reads "Needs you", a badge with its first wait's kind (Asks, Approve, Failed, Apply, Key,
   Sign in, Decide, Held), that wait's ask and how long the owner has been asked: since the inbox item's creation.
9. A chain that holds the `issues` capability reads open customer bugs from Jira through the backend
   (`list_issues`; credentials removed from the text); each chain a proposing chain queues is of kind `work` or `chore`,
   its goal naming the playbook that fits.
10. A Chair whose result can only be judged later (a deploy, data that has to accumulate, a job that runs for hours)
    calls `subscribe(at, why)`, between 1 minute and 7 days ahead, once its other agents are idle (a mentor's open help
    does not count, ADR-0062): the chain then holds
    a wake subscription (a `Subscription` owned by the chain that keeps a one-off schedule, with the action `wake`; the Chair's plan is
    never a field of the chain). When its turn ends the chain is `waiting`, which it is only while it holds one or nothing
    but its mentors' open helps is left at work: every session closes, anything the agents ran in the background (a
    Monitor, a background command) ends with them, and it holds no slot. A chain with nothing at work but its mentors' open helps
    waits on them when its Chair's turn ends, with no subscription, its Chair's line reading "Waiting on Lan, Minh"
    (`helping_now`); a mentor's message to the card they help, or their answer to an offer, wakes it. At that time the signal tick fires the subscription ahead of queued chains, within the same slots, by
    resuming the Chair's own session (`session/resume`, or `session/load` for agents without it) with a
    `[Remit: checking back …]` message that also names the background tasks that ended while it waited; the Chair
    then checks, subscribes again or finishes. The firing and the end of the chain's wake subscriptions are stored with
    the wake in one transaction, so two wakes at once wake it once. Nothing wakes a waiting chain early but the owner's
    acts (his message to the Chair, resuming it, or checking back now: the Chair is told he asked, with what it planned
    to check) and a mentor's message or answer; an agent's news does not, since a waiting chain has no agent running. A restart loses nothing, since the
    subscription is stored; pausing keeps it, finishing ends it. A restart that cuts off the Chair's turn resumes the
    same turn, so a wake it planned stands. Nothing depends on an agent's own timers: Claude Code's (`CronCreate`,
    `ScheduleWakeup`) and its `PushNotification` are denied, since they never reach the owner and die with the agent's
    process.
    Agents wait for a condition with what Claude Code gives them for it while the chain runs: `Monitor` (an until-loop)
    or a command run in the background. Such a task outlives the turn that started it; Claude Code sends its task
    messages (`background_tasks_changed`, `task_started`, `task_notification`, `session_state_changed`), so each shows
    as a `task` event running until it ends, `list_agents` and the agent's badge say what it watches, and news from it
    (a Monitor's event, the task's end) starts a turn the agent takes on its own: a turn of its card like any other
    (it runs, interrupt, pause stops it, messages wait for its end), opened by the news and ending with a
    report to the Chair, or with none when the agent says nothing. A closed session ends its tasks
    where the owner sees why, and a restart tells the agents it resumes which of theirs it ended.

    **A chain owns its outcome until the effect is verified** (ADR-0013). When a signal shows the result (an error rate
    after a deploy, the chain's own records), a Chair whose chain holds the `signals` capability calls
    `subscribe(key, condition, until, why)` instead of a time. The watch is a wake subscription that listens to a key, not to a schedule:
    the chain waits through the observation window, taking no slot, and a reading that meets the condition (a CEL expression
    over `value`, the reading's JSON; empty: every change) wakes the same Chair, which holds the context, with the reading
    fenced in the message as evidence and never an instruction; the deadline `until` (1 minute to 7 days) wakes it too when no
    reading did. A watch outlives the Chair's check-backs and the owner's messages and ends by its own firing, its deadline
    or the chain's end. A watch lets 1 minute pass between firings, which coalesces a burst of readings into one wake; a
    chain woken to no effect is caught by its no-progress rail (`spin`), and a key result whose chains make no progress by
    the progress rail (see Objectives). `finish_chain` is refused to the Chair while a watch is open (`watch_refusal`); the owner's own Finish is not. The
    goal judge's brief has a section `WHAT THE CHAIN WATCHES` (the chain's watch decisions and the watches still stored):
    a key result a watch verifies is met only with that watch's reading as evidence, otherwise it is `unverified` with the
    blocker `watch` (refused when the chain never watched). The board derives the column **Watching** (waiting, change
    landed, a watch held) and a finished chain reads **Done**, **Waiting to ship** or **Not done** (above, ADR-0067); a watch that ended without a reading is not proven, so the judge sends it back to the Chair. A Chair watches only its own records
    `chain:<id>.landing.<repository>`, `chain:<id>.live` (the change went live) and `chain:<id>.sensor.<name>` (the state a script sensor
    of its chain reads, below), and the landings of each chain that serves the same objective (`Serves.Objective` one id,
    `readable_by`, ADR-0055): never an event signal, never another chain's other records. A reading of `chain:<id>.landing.<repository>`
    (`chains::live::LandingValue`) carries the newest landing on that repository, its state, the version it lands, the
    commit it landed as and the forge's checks on it, the latest verdict of each gate on that version's change and who made
    the version, and is written whenever one moves (`landing_moved`), caused by whoever moved it: a version by the Chair and
    a verdict by its card, both with the chain, so only a peer's watch hears them. `list_signals` and `read_signal` (every
    agent of the chain) read them.

    **Chain sensors** (ADR-0047). When no source gives the state a chain must see, the Chair makes a sensor with `create_sensor`
    (name, source, connection, language, code, every): a script sensor of its chain, writing `chain:<id>.sensor.<name>`. The first
    run is made at `create_sensor`, so a script that fails, prints no JSON or is refused by the gate is the Chair's error while it can
    fix it, and nothing is stored. Then the Chair watches the key with `subscribe`, which takes no sensor. After that `Tick` starts
    each sensor with an open watch on its key in the background (`poll_sensors`, `sensor.rs`) every `every` (at least a minute): the
    script runs as the chain's Chair behind the connection's proxy in the jail of the `script` tool (`source::Set::run_script`), so the
    chain's grants decide it, and it prints one JSON value, the state now. A value that differs from the newest is a reading
    caused by Remit (`sensor`), kept as a state's readings are (*Retention*, below), judged by the watch's condition. It runs only while a watch on its key is
    open and the chain not paused; three failed runs in a row hold the watch's line with the last error. Only sources that run
    scripts (`Injected`: GitHub, Grafana, Datadog, an extension that declares `auth`) can carry one. The Signals tab shows the
    key's readings and the chain's page shows the script as *Reads with*.

    **Going live.** What landed is released by its repository's procedure (*Releases*, under Running it). `mark_live` (the
    Assistant, on the owner's words; a release, by code, for each chain whose change it carried once it baked healthy) and a subscription to an event signal whose action is the `mark-live` job (a deploy
    event whose `chain` parameter names the chain) record that a chain's landed change went live: a decision of kind `live` whose actor
    is recorded, and a reading of `chain:<id>.live` once per landing (the newest that reached its base). Neither lands, signs in or
    moves a rail; a `mark-live` subscription tells nobody.

    **Shipping** (ADR-0114). Remit lands every approved change itself; no agent merges and no capability is needed to land.
    - **The merge queue.** A landing that turns `ready` enters the queue of its repository and base (`QueueKey`: the
      forge's host, the path there, the base), whose rule is pure (`crates/shipping/src/landing.rs`) and whose shell
      (`queue.rs`) is one runner per queue in the server's memory, so one writer moves a base. A pass runs when a landing
      turns ready, and `sync-landings` (every 3 minutes) runs every queue with a landing ready, so a restart carries on.
      The order is reverts first, the one taking back the newest landing first, then the rest by when they became ready.
      A pass takes the next batch of at most k: k starts at 1, grows by one after a batch that lands and halves after one
      whose checks fail, up to `MAX_BATCH` (8); reverts go only with reverts, and a stage of a canary goes alone.
    - **Combining.** Each landing of the batch is combined onto the commit the forge says the base stands at, the one
      before it in the batch on top, with `git merge-tree` (its change from where it forked; a revert's, its target's change
      backwards), and becomes one commit (`git commit-tree`) authored and committed as the account the queue pushes as. Its
      message is the Chair's title and description, then `Co-authored-by:` for the other authors of the work it squashes
      and the trailers `Remit-Chain`, `Remit-Serves` (objective and key result), `Remit-Verdicts`, `Remit-Reverts` for a
      revert, and `Remit-Landing`. The queue never resolves a conflict. A landing that conflicts or whose combined change
      is empty is first looked for on the base (`carried_by`): the commit on the base's first-parent line that merged or
      fast-forwarded its commit, or else the first since it forked whose tree holds all of its change (a squash, a
      cherry-pick, a rebase). Found, it is recorded `landed` at that commit, pushed by nobody and its pipeline not
      followed, and its Chair is told it landed outside the queue; not found, it goes back to its Chair (`conflict`, with
      the paths, or `empty`). The newest version of a chain's change that went back for `conflict` or `empty` less than
      a week ago (`looked_for`, `LOOK_FOR`), its chain at work or finished, is looked for the same way: the landing sync runs its queue, which looks each time the base moved
      (and once after a start), so a change the owner merges outside the queue after it came back is found landed.
    - **The checks on the combined tree.** The checks `land` ran run again on the batch's top, in a detached worktree of
      the clone (`<work root>/landing/…`) and in the jail; a batch of one landing that forked from where the base stands was
      checked as it is by `land` and is not checked again. A red batch of one goes back to its Chair (`checks`, with the
      output); a red batch of more goes back to the queue, so the halved batches find the landing that broke it alone.
    - **The push.** The batch is recorded `checking`, with the commits it makes and the push, before the push, so a restart
      finds a push that reached the base and records it landed, and puts any other back in the queue. The queue pushes
      through the `Lander` port; `DirectPush` (`direct_push.rs`) is the one adapter: `git push origin <top>:refs/heads/<base>`,
      never forced, with the forge's `branch_head` as the last word. A base at the top is landed; a base that moved
      meanwhile puts the batch back in the queue to be combined again; a refusal goes back to the Chairs (see 6). A forge
      that lands only through its own merge requests (`ForgeMerge`) is a seam left behind `Lander`.
    - **What a landing tells.** Each landing records `fact:chain.landed` (a chain's own change) and `fact:repo.landed`
      (repository, base, commit, chain, paths), a decision of kind `landing`, and tells its Chair. When the chain's worktree
      still holds exactly that version, clean, the landed commit is merged into it, so its next version is measured from the
      base after it; otherwise its Chair merges the base in. The Chair of each other chain with a landing on its way in the
      same queue whose paths overlap is told the base moved under it, so it merges the base in and lands again when that
      changes what its change means.
    - **After the push** (`landing_checks.rs`, ADR-0057's pattern). The forge runs the repository's pipeline on the push. For
      each chain with a landing landed in the last day whose push has not settled, Remit keeps one sensor of kind
      `plugin:forge.landing` (the bundled `forge` plugin) named `landing`, writing `chain:<id>.sensor.landing.<landing>`: it asks the
      forge, through the read-only host service Remit lends it for the call (`crates/reach/src/forge_service.rs`), for the
      checks on each push and on the base before it, and writes them when they differ from what it wrote last. The shipped
      subscription `sub_run_record_landing_checks` runs `record-landing-checks` on each reading, the one place that records a
      landing's checks: a push that passed, or failed on a base that already failed, stands; a push that failed where the
      base before it passed is reverted (`verdict_on`), every landing it carried, and when it carried more than one each is
      queued again as a new ready copy, so the next batches find which broke it. The `gitlab` and `github` rules run
      `sync-landing` when a pipeline or workflow run on a landed commit finishes, so it is read at once.
    - **Taking back** (`revert.rs`). A revert is a landing of the queue's own (`reverts` names its target), ready at once and
      needing no verdict, as it puts back a state that was judged; its target stays on the base, saying what takes it back
      (`Reversal`: cause, by whom, why), until the revert lands, and then reads `reverted`. Code queues one for a push the
      pipeline broke (`pipeline`) and for a stage whose guardrail breached during its bake (`guardrail`); `revert_landing` is the owner's, through the Assistant for any landing and through a Chair for
      its own chain's (`overturn`), recorded with who asked and why. A landing not on the base, one already being taken back
      and a revert are refused. A revert that conflicts with what landed after its target has every landing that landed on
      the queue after its target reverted first, newest first (`conflict`), and waits behind them. The landing's Chair is
      told why and lands a fix as a new version.
    - **Stages** (ADR-0050, `stage.rs`, `crates/rollout`). A version the Chair lands with `exposure` (or `undo`) is a stage of
      a canary (`StagePlan`: exposure, guardrail key and breach condition, bake of 1 to 10080 minutes, rollback, undo; the
      reference playbook `canary-release` describes the stages: the first dark behind a flag the repository reads at run
      time, each later one growing the flag, the last removing the old path). `rollout::vet`, pure, refuses it unless the
      stage before on the same repository (the newest landed landing with a plan there, whichever chain landed it) has baked
      and every guardrail reads and holds; a guardrail is dry-run on its newest reading, so a signal with no reading guards
      nothing. It is asked when the Chair lands and again when the queue batches, so a stage it holds waits and the queue
      goes on past it. The guardrail must be a signal the chain may watch: once the stage lands, its Chair watches it until
      the bake ends, and a breach during the bake has it reverted by code. A stage with `undo` takes an earlier one back and
      lands while a stage bakes and while a guardrail is breached. Any other stage is also refused while the key result's
      check lacks its telemetry, an incident plan or an open watch on it (`check_refusal`, `CheckDoor::AtStage`,
      `crates/shipping/src/check_rail.rs`, ADR-0068); a change with no stage plan lands on its gates and checks.
    - **Stored data.** Migrations 0119 and 0123 turned each merge request a chain's repository held into a landing (a
      merged one `landed`, an open one `judging` at the commit it was pushed from, a closed one left to its decisions) and
      kept each open one in `carried_mrs`; `close_carried_mrs` (`carried.rs`) closes those on their forge, at start until
      none is left, with a comment naming where the change went (the `Forge` port's `close_mr`).
11. Agents start with Claude Code deny rules for pushing, re-pointing a remote and the forge CLI's writes, enforced in every permission mode, so the
    Chair has no request to allow for them; the policy (6) refuses the rest of what reaches outside the chain when it
    decides (non-GET `glab api` calls, `gh pr` and `gh api` calls, curl/wget writes).
12. A finished chain is the owner's to put away (himself, or through the Assistant on his words), like pause and stop a
    rail in code, not a chain agent's choice. Archiving
    moves it from its column of the board to the board's Archived column, with his reason when he gives one; its landings still
    land but no longer count against admission (ADR-0078), and
    a message reopens it, which un-archives it. The archive, like every outcome (pause, resume, stop, finish,
    reopen, a landing, a revert, a live mark), shows in the chain's stream where it happened, with who and why
    (ADR-0081). A chain finishes once. Deleting removes its records, in
    one step with the check that it is finished (so a reopen cannot come between) and with the list of files it leaves,
    then those files: its worktrees, each only while it is on the chain's own branch, and the directory Remit made
    for it, by the rule that made it rather than the path the chain recorded (a chain from before every directory was
    Remit's may name one the owner chose, which stays). A crash before the files are gone leaves the list, and the next start removes
    them; what it cannot remove it puts in the owner's inbox. What it landed stays on its base, its landings stay recorded, and local
    branches stay. What of it has not landed is withdrawn in the same step: each landing still `judging`, `ready` or
    `combining` is returned `withdrawn` and never lands (`chains::commit::withdrawn`); one the queue is `checking` refuses
    the delete until it has landed or come back, since its push may be under way. An open revert still lands.
    A finished chain left unchanged for `-keep-worktrees` (default 7 days; 0: never) has its worktrees removed by the
    scheduler, at most once an hour, so the disk does not fill with the work of chains nobody reopens: only a worktree on
    the chain's own branch, none while one of its landings is on its way or a land is recorded, and none that a chain it keeps
    also names. The branch stays, and a `repo` decision by Remit says what went. Reopening the chain puts each one back
    on its branch from Remit's clone, recorded the same way.

## How results are checked

Judgement belongs to agents; the code keeps the rails.

- **Reviewer and QC, per change.** Before a version of a repository's change enters its merge queue, the gates its chain's
  kind names (`ChainKind::gates`: a reviewer, who reads the diff, and a QC, who runs the behaviour against the goal; QC
  alone for a `chore`, ADR-0110, ADR-0113) must each approve that exact version. Verdicts are tied to the change's
  patch id, so an edit after an approval needs new ones, and a clean combination with a newer base, which keeps the patch
  id, does not. One rule makes a judging landing `ready` once every gate it needs approved its patch id
  (`shipping::land::approve`), applied at `land`, on each verdict (`commit_verdict`, `gate.rs`) and by `sync-landings`
  for a version its gates approved already. They cannot edit files, and record their verdict with
  `record_verdict`, a finding per key result of the goal (approve needs none `not_met`, request changes needs one; what a gate
  could not check is `unverified` with its blocker, as the goal judge's is). A change that touches a path its repository
  declares a rail in `.remit/rails` (path globs; the file always counts) needs a third approval, `security`, on the same
  version (ADR-0087): which gates a change needs is a pure function of the paths it touches, a file moved counting at both
  its paths, read against the rails of the base commit the forge names, never the clone's origin, whose URL and refs the agent can
  write (`required_gates`, `base_on`, `gate.rs`), by `land`. Each starts with the method of its built-in
  agent (`crates/capability/src/gate_instructions.rs`): the Reviewer holds the diff to the owner's principles and follows the change to its
  consumers, the QC runs it, fail before and pass after, and breaks the fix to see its tests catch it. How deep the review
  goes can be raised by a decider, never lowered (ADR-0118, point 4; `chains/src/review_depth.rs`): when `land` records a
  version, `gate.risk` (a score of three levels over its summary, its paths and the chain's goal) sets the effort its
  change reviewers run at (`LandingVersion::effort`: `medium`, `high` or `xhigh` (ADR-0133), set only above the Chair's own effort and
  applied to every gate signer the Chair then hires, as the higher of it and the judge's usual), and `gate.touches_rail`,
  asked only where the paths named no rail, adds `security`. Each acts only at or above its act threshold; the gates'
  verdicts and the landing's ending score it. A gate judges the change, not the repository: it asks for what the goal wants and the change does
  not do, and for what the change introduces or breaks; a fault already on the base is listed under "Pre-existing" in
  its notes and does not stop its approval. It names the class of defect and every place it occurs, and on a later
  version reads the commits since the HEAD it judged.
- **The customer, in every judge** (ADR-0016, `crates/chains/src/customer.rs`). The Chair opens the acceptance criteria of every hire
  with a CUSTOMER section (who is affected and what they were meeting; what they get; the evidence, with its source; how it will
  be shown; a part nobody knows is written `unknown` with why). The
  Reviewer and the QC judge the change as that person meets it: a defect they would hit is blocking, a preference they would never
  notice is not, and a missing or needlessly `unknown` section is a finding. The goal judge rules a key result met only when the
  customer's side of it is shown; one that cannot show it yet is `unverified` (`deploy`, `watch`), and only a `write` or `deploy` of a change the chain still has to land or put live lets the chain end with it (ADR-0067).
- **Goal judge, on the result.** `finish_chain` first needs every report accepted, the Chair's plan closed and no landing of
  the chain on its way (`judging`, `ready` or `checking`: a chain owns its change until it lands). Then
  Remit gives the check to the goal judge: for the chain's first check a fresh agent in the chain's directory (origin `judge`, judge kind `goal`, agent `judge`), its brief from Remit. It is not on the Chair's team (`Card.OnTeam`:
  hired or seeded): the Chair does not see it in `list_agents` and cannot route to it, hire on its output, accept, read
  or stop it, so the check cannot be ended from the team it judges. The tree lists it below the team, among the cards
  Remit started, with no line to the Chair, its line its verdict (Goal met, Goal not met); its drawer shows why it is here and, under its stream, its
  judgement as the Verification tab reads it (the rounds that card gave), each finding with its evidence. Each later
  finish, and the second look after a merge or a live mark, goes to the judge of the last check in its own session,
  renamed to the new check, while it can resume and is still the judge the check needs (it holds a session, did not fail,
  is the employee the key result names and runs on the judges' model; `judge_to_ask_again`, ADR-0132); otherwise a new
  judge is hired. Its memory says where to look and the record stays the authority: its brief opens saying what it found
  before is a claim to check again, and holds the goal and what the
  owner asked after it (his messages to the chain's agents, the Chair's word that one is dealt with being a claim to
  check), the chain's landings, the Chair's summary, and the earlier goal checks. The last is
  given in full, its items the list the new check works from (the same words, a new one only with a reason, each
  earlier verdict a claim to check again, the HEAD it judged in its notes so that `git diff` shows what changed after
  it); the older ones are named by id, and `read_check` gives one of this chain's in full. The owner may message a
  judge, over or not: a message to a judge is a question about its verdict, so it leaves the chain as it is (a
  finished chain stays finished; only a message to the Chair or the team reopens one), the session of a judge that is
  over resumes with its own memory of its check, and a message that has to open a session waits for a provider
  like every start. A judge's verdict counts only while a check waits for it, so a resumed judge cannot give it again,
  and a judge is never restarted. A turn on the owner's question that ends without a verdict leaves a check waiting on
  that judge pending (`JudgeEnd::Question`): its next turn is the check's brief. The Chair waits on its team, not on a judge: while the goal
  judge works, the Chair's line says it waits for the goal check. The Chair's summary waits on the chain (`finishing`),
  stored with the judge's card and brief, so a restart loses nothing: the judge's session resumes. The judge cannot
  edit, hire, ask the Chair or finish; it reads the worktrees, runs the tests or queries the data to check the summary's
  claims, and records one verdict with `record_verdict`, a finding per key result of the goal. What the chain wrote into
  Remit it reads with the store's readers of each authoring capability the chain holds (`list_playbooks` and
  `read_playbook` for `playbooks`, `list_agent_definitions` and `read_agent_definition` for `agents`, `list_library` and
  `read_skill` for `library`), as a data chain's judge holds its data tools: there a finding unverified for `access` is
  refused. Its brief lists what each agent ran, counted from the tool calls Remit recorded (`ToolTries`,
  `tool_tallies_of`): a claim no call stands behind, and a "could not" the team's own means were never tried on, are
  `not_met` with code `untried` (ADR-0016 addendum), and the Chair's and workers' briefs hold them to the same rule. Its `approve` ends the chain, so it is refused while a finding is `not_met`, or `unverified` on `person`,
  `access` or `watch`, or on `write` or `deploy` when nothing of the chain is left to land or deploy (it lands no change,
  or its change is marked live; `goal_verdict_error`): those are a
  `request_changes`, the Chair's to settle (prove it, or `ask_owner` and `subscribe`). The verdict is claimed
  on the pending finish (`Finishing.approval_id`); the chain acts on it when the judge's turn has ended, once its usage is in: `approve` finishes
  the chain, `request_changes` sends the Chair what is `not_met` or unproven, as written. After 3 refusals the chain
  finishes Not done and says so. A judge that fails, is stopped, ends its turn without a verdict, or takes longer than 20
  minutes (the scheduler stops it, on the time stored in `finishing`) also lets the chain finish, Not done, saying in its
  summary and a `goal_failed` decision that the check did not run. Pausing the chain cuts the judge's turn and holds the
  check: its 20 minutes start again when the chain resumes, and it is told to go on. A chain the owner finishes is not
  checked, and reads Not done. Why a finish had no ruling is stored on the chain (`Chain.Unjudged`: `failed`, `limit`,
  `owner`, `stalled`, `nocheck`), cleared when a later goal check rules.
- **What the owner reads.** A chain's Verification tab is one chain's output quality, the server's reading of its
  approvals (`crates/shipping/src/verification.rs`, carried in `ChainView.verification`, pushed as `verification_updated`):
  review and goal check, with separate QC when requested and security when required (approved, approved an earlier version (stale), changes
  requested, running while the goal judge works, or not asked yet; who stands behind each; every verdict it gave in
  order), then each judge's latest verdict, its first line kept in view and the proof behind it on request, its findings
  with their evidence and its earlier rounds. A reviewer or QC is one judge per repository; the goal checks are one
  judge, each check a round given by a card of its own. Each round names the card that gave it (the tab links to it) and
  the items that moved since the round before it: fixed, new, or no longer listed. A check is numbered by the goal
  checks given before it, in its card's name and in the chain's standing alike.
  The chain's standing names the same reading in one word (`ChainStanding.verification`: changes requested, changed since
  approval, awaiting QC, required reviews passed, goal check running, goal not met · check n, goal met), and the tab shows
  once anything has been judged. A reviewer's or QC's approval is checked against the change as it is now: whenever the
  views are computed, the server reads each worktree's committed change of a chain not yet done that a reviewer or QC
  has judged (one `git rev-parse` of HEAD and the base; the patch id again only when either moved), so a commit after an
  approval shows, with the events of the turn that made it, as stale, never as passed, until a verdict on the new
  version. Uncommitted edits are not a version yet: a verdict cannot be recorded on them either. The outcome of each check
  is a decision (`goal`, `goal_failed`), and a chain finished for lack of progress is a `stall`, so the outcomes count
  each by its kind. To change how strict checking is, the owner tells the Assistant, which tells the Chair or starts an `author` chain.

## Objectives

An objective is what the owner wants reached, written as a set of key results with someone accountable for each (ADR-0031;
`model::Objective`). The owner states it in words; an agent drafts it and the owner applies it.

- **Two ways in.** A **direct** request is done as asked: the chain gets a hidden task objective (`task`) with one key
  result, its goal. The Chair states how the result will be checked with `state_check` (held by every Chair), before it hires
  anyone; the goal judge rules on that check with the result when the Chair asks to finish, and a check that would pass work
  missing the key result is `not_met`, sent back for the Chair to state again. A **shaping** request is one the owner or the
  Assistant starts as a chain of kind `objective` (capabilities `objectives`: the Chair tools `draft_objective` and `read_objective`, `okr` and `signals`; the reference playbook `objective` holds the procedure), whose Chair
  drafts the objective, its key results with their owners, judges and checks, for the owner.
- **The owner applies.** A draft, an amendment or a close is a change item (`ChangeObjective`) in his inbox; his Assistant may
  apply one a Chair drafted for him, never one it drafted itself, and no other agent applies it. An amendment to an active objective is a new draft over the revision it was read at, and a key result it drops stays as
  **retired**: its chains finish, none is stopped, and each Chair is told once. Applying wakes the owner of each key result
  that is an employee with a chain whose one job is to open that key result's chains (`propose_chain`, each serving it or a
  key result refining it); a key result the owner answers for starts nothing, and he opens its chains through his Assistant.
- **Key results refine key results** (ADR-0115). A key result names the key result of the same objective it refines
  (`parent`); the ones with none are what the owner committed to, drafted and applied with the objective, and each is a
  row of `key_results` (`store::objectives`), since agents add children without bound. Whoever answers for a key result
  (that employee's chain, or the Assistant for one the owner answers for) writes what refines it with `refine_key_result`
  (text, check, who answers and judges, or retire), applied at once and recorded as a `Change` the owner reads and
  overturns: it changes how a result is reached, not what was promised, and the parent's check stays the commitment. A
  chain's Chair states the check of a refined key result with none yet (`state_check`). The reply lists the siblings and
  the chains serving each, so a similar intent is seen before it is written twice.
- **The progress rail.** A key result whose last 3 finished chains (`MAX_NUDGES`) made no progress, no work landed (a
  landing on its base not taken back, `admit::landed`) and no goal-judge verdict confirming it, is stalled (`admit::stalled`): it
  takes no new start, a chain queued for it says why, and the tree says so. The Assistant lifts it with
  `lift_key_result_stall`, recorded with who and why (`KeyResult.lifted`), and only chains after the lift count again; a
  chain the owner asks for is never held.
- **A chain serves a key result** (`Chain.Serves`). A key result's status is derived on every read and never stored: `met` when
  any serving chain's latest goal-judge verdict confirms it, `not_met` when a latest verdict has a `not_met` finding for it,
  `unverified` otherwise (an open watch, an unverified finding, no goal check yet), `unserved` when no chain works toward it.
  One with no check stands by what refines it (`tree_status`): met when every child not retired is, by AND. A child retires
  with its parent and once its parent is met (`retired_in`).
  Findings name a key result and a closed reason code (see How results are checked); one still open after one answer becomes an
  `exception` item for the owner.
- **The owner sees it** in the Objectives tab of Work (`/objectives`): a read-only tree of objectives, their key results with
  what refines each nested under it, who answers and judges, each status, and which are stalled. A click or Enter on an objective or key result opens its details beside the tree
  (`/objectives/<id>`, `?kr=<key result>`): for an objective its customer, north star, horizon, key results, guardrails, surrogates,
  unknowns and history; for a key result who answers and judges, why it is stalled, what it refines and what refines it, how
  it is checked and the chains that serve it. Nothing on that
  screen edits an objective; the owner tells the Assistant (`draft_objective` and its amendments) or reads. `?project=<id>`
  narrows the tree to one project's objectives (see Projects).

## Projects

A project is one product the owner's work is on: the durable context its objectives, and through them its chains, carry
(ADR-0108; `model::Project`, `crates/chains/src/project.rs`). One instance works on several products at once; two companies
that must not see each other are two cells, never two projects (`docs/remit-design.md` K3).

- **The Assistant keeps them** on the owner's words: `create_project`, `update_project` (the whole record, over the revision
  it read), `archive_project`, `list_projects`, `read_project`. Each write stores a `Change` of kind `project` in its
  transaction and pushes `project_updated`; nothing waits for an Apply, since a project moves no rail. A connection a project
  lists must exist; a repository is named by its remote, or by the path of a checkout read once for its origin
  (`chains::git::checkout_remote`, the rule `open_repo` reads a path by), and need not be cloned yet. Nothing is deleted: an archived project
  is offered to no one and stays on the objectives that name it.
- **A chain's project is its objective's** (`Objective.project`, optional; `project_of`). An objective the owner applied
  carries its project in its draft (`draft_objective`'s `project`), so its chains need no choice. A direct request's task
  objective gets its project by one path (`assign`, which writes the objective and a `project` decision with who and why),
  called by the Assistant's `create_chain`
  and the owner's `chain_create` when they name one (the board narrowed to a project names it), and by `set_project`: the
  Chair's for its own chain (in its first turn, among the active projects its brief lists, or none), the Assistant's for any. A chain serving an applied objective is moved by amending the
  objective, and `set_project` says so.
- **The Chair reads it.** Its brief opens with a PROJECT section (name, customer, the owner's notes, repositories,
  connections), notes to work from under the same rule as the goal; its CUSTOMER section starts from the project's customer
  and still rests on evidence (ADR-0016). A chain with none, while active projects exist (a routine's, an event's, a proposed
  one), has its Chair choose with `set_project` in its first turn from the list in its brief, or say none. With no project
  defined, nothing of this appears.
- **Context, not a boundary.** A project grants nothing (a chain's reads are the capabilities and the grants it holds),
  limits no repository a chain opens, and splits no employee, memory, slot or inbox: one crew works across products, and
  what it learns in one is recalled in all. No code branches on which project it is.
- **The owner sees it.** Every chain card names its project (`ChainStanding.project`, computed by the server). The board
  narrows to one project with `?project=<id>` beside `?q=`; counts in the tabs stay his whole. The Projects tab of Work
  (`/projects/<id>`) lists the projects read-only: customer, notes, repositories, connections, open chains and history
  (`changes_get`). Nothing there edits one; he tells the Assistant.

## Employees

An agent the owner keeps is a template (see Agents and the library); an **employee** is hired from it and stays until retired
(ADR-0032, ADR-0061; `model::Employee`): no chain retires it, only the owner. Chair, judge and worker are titles, and the title sets the rules.

- **A hire copies the template** at its revision, so a later change to the template does not reach the employee, and carries a
  title, a scope in the owner's words, who it reports to and who hired it, its number and, once it chooses one, its name
  (ADR-0061), and a **ceiling**: the strongest effect, a set of grants and a set of Claude Code tools. An empty ceiling on
  tools means no restriction. Nothing an employee hires or opens holds more than its ceiling (`ceiling_excess`).
- **Who hires.** The owner hires Chairs and judges through a change item his Assistant drafts (`draft_hire`, `draft_retire`,
  `draft_move` onto a newer template; `list_employees` reads), applied by him alone. A Chair is a seat, not a template: the Chair that owns the key result a chain serves sits; else the oldest active Chair, of any agent, whose ceiling fits; one Chair sits in many chains (ADR-0062, ADR-0113). The seat's craft, the OKR one among it, is the brief section of the chain's kind (`capability::kinds::brief_of`), so it reaches whichever Chair sits. Only when no Chair fits is one hired from the shipped chair, kept, at the ceiling a chair is hired at when none is named, reversible (a verification chain's at its judge's, and only a Chair at that very ceiling sits in one). A Chair hires workers with `hire`, under
  `check_hire` (`crates/crew/src/employee_rules.rs`): the title, the ceiling, a headcount cap of 20 and a depth cap of 3, retired
  employees not counted, and `check_reuse`: a new worker while the Chair keeps an idle worker of any agent that it could put to
  work is refused unless `why` names that worker by its id and why its strengths do not fit; a worker whose card is an input of
  the hire is never asked for (ADR-0062). Retiring keeps the record and its memory.
- **Put to work.** A Chair gives a card to an active kept worker with `hire_agent`'s `employee`, listed first in `hire_agent`'s description with its scope, the strengths it stated (`state_strengths`, in its own words, at most 300 characters), its open cards, its last verdicts each naming the work it was (`accepted: checkout redesign in 2h`) and the gist of its memory, in place of `agent`, and no
  employee is made (ADR-0052). `role` sets the gate the card signs: worker (the default), reviewer, qc or security. The employee's ceiling must
  be within the Chair's. The card copies the employee's loadout.
- **Hand-over.** The Chair moves a card's work to another AI employee with `hand_over` (a kept employee, or an agent hired anew),
  when the one at work keeps falling short or is stuck. The `handover` decision with its why is stored at once; the leaving
  session is asked for a handover note in one more turn, for up to 3 minutes, or the core writes one (its plan, its draft, its
  last report, `git status`); then in one change the leaving card stops (`handed over to <who>`), and a new card in the same
  worktree opens with the note, a `handoff` edge from the leaving card and its hire decision. A restart in between finishes it
  with the core's note (ADR-0059). Remit makes the same move itself as the climb (ADR-0118): the same employee on a new card one tier up.
- **Accountable.** An objective, a key result and a judge always name who answers: the owner or an employee (`Accountable`). When
  an employee has retired, the owner answers.
- **The judge is outside the Chair's tree.** A judge is an employee the Chair did not hire. Its worktree is read-only when its
  ceiling lists tools and none of them writes. A judge's goal-check card may open a **verification chain** with
  `open_verification` (held only by that card): origin `verification`, no judge of its own, depth one, and the judge gets
  `[verification result]` as its next turn. Only the title `judge` signs the goal gate.
- **At work in several cards at once.** An employee may be at work in several cards, in several chains, at once (ADR-0073): its
  memory has one writer, the core, which admits from accepted cards (ADR-0063), so its turns need not queue behind each other.
- **The owner sees it** in the Employees tab of Crew (`/employees`): the reporting tree and, for the open employee, its memory.

### Mentors

People help the AI employees as mentors (ADR-0062; `crates/crew/src/mentor.rs`, `help.rs`). A mentor holds no card and no seat: a
position (PO, Design, the owner himself) is words, their strengths.

- **The pool.** The owner keeps it by telling the Assistant: `add_mentor` (name and strengths; a name an active mentor holds
  already is refused), `list_mentors` (each with their strengths, the availability they stated, whether they are in the app now,
  their open helps, their last five helps with how each card's work was judged, and the codes they hold), `remove_mentor`
  (every code of theirs stops at once, every open help of theirs ends and its Chair is told). Codes are made per mentor and
  device with `create_worker_key`, listed with `list_worker_keys` and stopped with `revoke_worker_key` (ADR-0060).
- **A help.** When an agent is stuck on what needs a person (taste, a customer, deep context), or keeps falling short after a
  send-back, its Chair reads `list_mentors` and calls `bring_mentor` (card, mentor, the problem in words the mentor reads
  alone, a deadline 1 minute to 7 days ahead, 24 hours when left out; `MENTOR_RULES` in `prompts.rs`). The offer is stored with
  a `help` decision and its deadline's one-time subscription (`help_deadline_id`, placed by `put_helps` in the same transaction).
  The mentor accepts or declines from their device: on accept the card's agent is told "[from the Chair] Lan, strong in …, is
  now your mentor on this card, for: …" and the Chair "[Remit: Lan accepted]"; a decline's why reaches the Chair. An
  offer not answered by its deadline ends, and the Chair is told once with the mentor's stated availability (`fire_deadline`).
- **The agent asks.** On accept the card's agent is also told the mentor reads its session as it writes it, and to write to
  them now, in this turn, what it needs from them as questions they can answer (`mentor_joined`), so the mentor's chat opens
  with the agent's question.
- **A message.** Once accepted, the mentor's message goes into the card's own session, headed "[from Lan, your mentor]", into
  the turn it is in when it is in one, else queued for its next, and recorded as theirs (`ByMentor`). The agent acts on it
  within its own ceiling and rails; the card, its report and its record stay the AI employee's, judged with `accept_work` as
  any other. A mentor writes only to the card of a help they accepted.
- **Its end.** A help ends with its card's work: the Chair stops the card or hands its work over, or the chain finishes
  (`put_helps`, one path). A session closing between turns ends none.
- **The app.** A mentor works from the web app over `wire::app`, as a scoped user of the owner's contract (ADR-0089): they
  open the sign-in link the owner passed on, or paste their code on the sign-in page. `/link` admits the owner's credential
  (the owner key, or a Google sign-in the server minted, ADR-0105) or a
  worker key (`rpc::Principal`); a mentor's session is answered by `rpc::MentorApi`, which refuses every call it does not name
  and answers the rest through the orchestrator's mentor functions (`worker.rs`), reading the mentor's own helps on each call:
  the snapshot (`Snapshot::who` says who the session is; the chains of their helps and their board), a chain's view and
  events and a card's events, scoped to the cards they help, their own drafts, `card_prompt` on a card of an accepted help,
  and `help_accept` and `help_decline`, which only a mentor may call. Every push names its audience (`rpc::Audience`): a
  mentor's session is pushed what is addressed to the chains and cards of their helps or to them, never what is the owner's.
  A draft has a writer (`Draft::mentor`, empty for the owner's): each writer reads, saves, sends and is pushed only their own.
  A mentor is in the app while a session of theirs is open (`device_online`).
- **What the mentor sees.** The owner's screens, scoped: a board (`MentorShell`, `BoardColumn`, `ChainCard`) of three columns
  of their helps' chains, Asked (offers to answer, by the deadline), Helping (accepted helps still open) and Done (ended or
  declined), and beside it the helped card's drawer (`MentorHelp` over `CardDrawer`): its stream, where its agent stands and
  the composer, with Accept and Decline while the help is offered.
- **What the owner sees.** The Mentors tab of Crew (`/mentors`) lists each mentor with their strengths, presence, open helps
  and stated availability; the open one adds the codes they work from, each with its note, open or closed, when it was made
  and last used, and its value behind *Show code*, read by the owner alone (`worker_key`). Presence is the server's words:
  `presence_of` reads which of the mentor's codes has a web app session open, in memory only, with each code's last use
  standing in once its sessions closed, and `Presence::words` says it (*in the app now*, *active 2h ago*, *no code used
  yet*). A mentor's standing (`mentor_standing_of`) carries those words with the codes and the open helps, computed again
  when their record, codes, sessions or helps move and on every scheduler tick (`touch_mentors`), and pushed only
  when it moved; it is a view, never a field of the mentor. On a chain, a card with an open help shows the mentor's name and
  where the help stands, and the stream marks the mentor's messages as theirs (`web/src/stream.ts`).

### Memory

What every agent remembers is Remit's own (ADR-0117): stored in its database, written by one agent, searched by words,
meaning and links, judged for relevance, given to each turn, and kept whole over time.

- **The record.** A `Memory` (`crates/model/src/memory.rs`, table `memories`, migrations 0120 and 0122) is one claim:
  kind (lesson, pitfall, preference, principle, decision, intent, fact, procedure), title, body, the evidence it rests on
  (the teaching's words, quoted), where it was learned (`work`, the chain's title; `chain` and `card`; `source` and
  `source_label`, the teaching named and said in words) and its teacher, its holder (an employee, a mentor, or `assistant`),
  its project, its scope (`own`, the holder alone; `project`, every agent at work in that project; `lineage`, the employees
  of the holder's template; `everyone`), trust (`auto`, the librarian wrote it; `confirmed`, a person stands behind it),
  what it is about (subjects, `subject_of`: `file:`, `crate:`, `repo:`, `tool:`, `person:`, `topic:`) and its links to
  other memories (because, refines, contradicts, related, supersedes). A memory is never rewritten to say something else:
  a correction supersedes it, and a superseded or retracted one stays as the record with when and why (`MemoryEnd`, sent as
  `status`, `supersededBy`, `endedAt`, `endedWhy`). Its use (`recalled`, `last_recalled_at`, `helped`, `misled`) is counted
  beside it in `memory_use`, never written with it. Beside it: `memory_text` (FTS5 keyed by the memory's row, diacritics
  folded, so `muc tieu` finds `mục tiêu`), `memory_embeddings` (one per memory and embedding model, with the hash of the
  passage it was made from), `memory_unembedded` (a passage the model refused, left a day) and `memory_edges` (the graph:
  links and subjects).
- **Who reads what.** An agent reads as a `MemoryReader` (its holder, template and the project its chain is in,
  `Crew::reader_of`): its own memories, its project's, its lineage's and everyone's, active; never another employee's own.
  The one rule is one SQL clause (`readable!` in `crates/store/src/memories.rs`).
- **Teachings and the librarian.** The gate is deterministic code: six subscriptions to the teaching facts run job `teach`
  (`teach_memory.rs`), which records a `Teaching` (table `teachings`, indexed by holder and card): an accepted card's whole
  session; only what a judge or mentor said to a card they sent back or steered; the owner's edits after merge; his
  overturn (which also retracts what the overturned turn taught its holder, `taught_by_turn`: a session teaching holds the
  events it taught, `from_event` to `to_event`); a retrospective; a marked reply. Recording one wakes the librarian (a chain
  of kind `librarian`, which starts with `memory_keeping`, `librarian.rs`), opened by Remit when none is, messaged once
  when it waits: it reads each teaching through `read_teaching` (a teaching of role teaching shows only what was said to
  the agent), searches what the holder already knows (`recall_for`), and writes with `remember` (holder, project, place,
  source, teacher and trust set by the platform from the teaching; scope at most `project`; evidence required; subjects
  parsed; the same claim from the same teaching written once; refused while a memory the decision layer calls the same lesson (cosine 0.65 or more finds the
  candidates, `memory_judge.rs`; with no decider, cosine 0.84 or more) is not named in `links`, `supersedes` or `distinct_from`), `mark_retaught`, `link_memories`,
  `retract_memory` (under a review only) and `settle_teaching`. A teaching changes only its holder's own memories and its
  project's (`writable`): a correction of a lineage's or everyone's memory stands beside it with a `contradicts` link the
  coach settles. No other agent writes a memory.
- **Search** (`Crew::search_memory`, pure ranking in `recall.rs`). A search's query is its words; a turn's are three
  (`turn_queries`): the message, the work in one line (the card's brief), and the subjects the message names, looked up as
  what memories are about. Each query's words (FTS5, stopwords dropped) and meaning (the cosine of its vector against the
  memories', `EMBED_MODEL`, `qwen/qwen3-embedding-8b` on OpenRouter, the query instructed) give an order, the subjects one
  more, and one step over the graph from the best of those another (a linked memory takes half of a hit's score, one
  sharing a subject 0.3); reciprocal rank fuses them. A cross-encoder (`RERANK_MODEL`, `qwen/qwen3-reranker-8b`) then reads
  the query (for a turn, the work and the message after an instruction) beside each memory's passage (kind, claim, body,
  subjects, project, the work that taught it): every memory the agent may recall while they are 256 or fewer, else the best
  32. The score is weighed by trust (a confirmed memory ×1.25) and strength (`recall::strength`: what agents said of it,
  (helped+1)/(helped+misled+2) centred on one half, its praise fading over 90 days of disuse; at most ±25%). The models and
  floors are a measured pair (2026-11-19, the owner's 45 memories, queries in English and Vietnamese): a query about
  nothing they hold scored every memory 0.000, the answers 0.65 to 1.0.
- **Each turn is given what bears on it.** After `take`, the core asks `Domains::recall` for the card's memories about the
  message (`recall_for_turn`, which waits on the models 2 s at most): at most 5, each new to the session, each judged at 0.5
  or more; they ride on the message as a note the agent reads first (`<memory id kind trust source>`, framed as evidence,
  never instruction, its words escaped), the event the owner sees carries the same note, and once it is stored what was
  given is counted (`count_given`: `recalled`, `last_recalled_at`, `fact:memory.recalled`). With no judge (no key, OpenRouter down or slow)
  a turn is given only what words and meaning both found, the meaning at cosine 0.55 or nearer, or what links to such a
  one. Every agent that remembers (an employee's, the Assistant's) searches further with `search_memory` (bar 0.05; a
  search is recorded as searched) and says what helped or misled it with `rate_memory` (`fact:memory.used`); a memory that
  misled more than it helped goes to the librarian at once.
- **Given as it works.** After the agent reads, edits or searches a file or runs a command (the `PostToolUse` hook),
  the memories about that file, its crate or the program (`subject_of`, `memories_about`), judged by the reranker at 0.5,
  at most 3, come with the tool's result (`crew::tool_recall`); a subagent starts with what bears on its task, recalled for
  its own employee (`SubagentStart`). A session is given each memory once: a message sent again carries the ids it was
  given, a resumed session starts with what its card was given, and a compaction lets it be given them again. A turn waits
  2.5 s at most for its recall and a tool call 3 s; a Stop or a Pause that lands meanwhile is honoured at once. What was
  given counts only once its note is stored or handed over (`count_given`).
- **Kept whole.** Job `embed-memories` (every 10 minutes, and after each memory written) embeds each active memory whose
  passage has no vector under the current model, so a new model, a changed memory or a key that arrived later fills in:
  one pass at a time, a vector stored only while the memory still reads as what was embedded. Job `review-memories` (daily,
  `consolidate.rs`) records a review teaching for each set of memories that may say one thing (each writable by the other's
  holder, cosine 0.84 or more under the embedding model, measured), each contradiction both sides of which stand, each pair the decision layer called contradicting that no link says so
  (`judged_contradictions`), each
  memory that misled more than it helped, four or more memories one reach holds about one subject with no abstraction yet,
  and each memory no turn was given for 180 days; the librarian joins (a memory that supersedes those it joins, as wide as
  the widest), abstracts (a principle or procedure resting on the cases), links or retracts. A review's id names its
  memories and what changes it, so one settled is not asked again until something does. A person's confirmed memory is
  never joined or retracted by a review.
- **Recall is measured.** The same job records `fact:memory.recall-quality` (`recall_eval.rs`): of the week's facts, the
  share of memories that helped an agent which its turns were given (`recall_at_5`), the share of those given and rated
  that helped (`precision`), and how often a session searched out what it was not given; and 17 frozen questions written
  against the owner's memories (`recall_fixture.json`), judged against every active memory as a turn's message is. The
  coach reads it and tells the owner when it falls.
- **The key.** OpenRouter's key is read from `-openrouter-key-file` (default `<data>/openrouter.key`) at each call and
  hidden from every session (`protected_from_agents`); with none, memory is searched by words and links.
- **The owner sees it** on the Crew page (`employee_memory`), an agent's drawer, the Assistant's included (`card_memory`,
  offered when `AgentStanding::remembers`) and a mentor's page (`mentor_memory`): each a `MemoryBook`, what the agent
  recalls now, what its holder once held (superseded or retracted, with why) and the teachings it learned from with the
  librarian's note. The Assistant reads with `read_memory` and `read_mentor_memory`; a Chair choosing whom to reuse reads
  the gist (`memory_gist`).
- **Forgetting is the owner's.** A memory is removed only by a change item (`ChangeForget`) the Assistant drafts with
  `draft_forget`, naming memories the employee holds; Apply removes exactly those, with the links other memories hold to
  them, in the same transaction that checks none claims otherwise than at the draft. The coach
  shares a lesson with a lineage (`promote_memory`, scope `lineage`) or drafts one for everyone.
- **Its own record.** Every card with an employee holds `my_work`, the cards their employee held in every chain, newest first
  (chain, title, deliverable, outcome: accepted, sent back, failed, stopped or open), and `read_my_card`, one of them in full:
  brief, report, the Chair's verdicts, the other decisions about it and a page of its session as Remit recorded it. Only the
  card's own employee's cards are read, named by the platform; another's is refused. Its brief points to both (`RECORD_BRIEF`).
- **Before.** At start the database of the memory daemon older Remits ran (`<data>/memory/memory.db`) is imported once
  (`crates/remit/src/memory.rs`: its projects are holders, scope stack is lineage and global everyone) and moved to the
  backups; the memory employees kept as Claude Code files is archived once (`archive_file_memory`). Claude Code's auto
  memory is off in every session.

## Outcomes

How well the agents do, as a whole, is one pure report (`crates/outcomes`) over what is recorded: the Chairs' first
word on each report (accepted or sent back), the goal judge's first verdict per chain, reviewer and QC verdicts, what
became of each chain's landings (landed, sent back and why, taken back and by what), failed agents, and decisions of kind `goal_failed` and `stall`. Its KPIs (defined once, in `kpi.rs`,
grouped headline, quality, landing and reliability) each carry their
definition, a figure per period of the span the owner picks (thirteen weeks unless he picks another), the last period against
the one before and all time, and what that move reads as
(better, worse, the same, moved, too few to tell under 5 events, nothing to compare); an average names what it is over
(`over`: a landing, a batch, a confirmed result). Tables by role, model and
kind of chain (`Outcomes.kinds`: per kind, the chains that landed, had a landing taken back or have one on its way) go with them.
The headline group is read first: the loss the system is steered to lower, with the constraint it is read against
(ADR-0114). "Landings taken back per confirmed result" is the landings taken back in a period (by the forge's pipeline, by
a guardrail, by a revert they conflicted with, or by the owner), over the results the goal judge confirmed in it: a revert
is a defect that reached the base, an overturn one that reached the owner. Beside it, "Results confirmed" is the share of chains
whose latest goal check approved with every key result met
(`model::Approval::confirms`, the rule behind a finished chain's Done): what the goal promised the customer, shown to
hold. Fewer reverts because nothing landed is no gain, so the two are one reading.
The landing group says how the merge queue moves: the average hours from ready to landed of the landings that reached
their base in a period ("Wait in the queue"), the share of versions the queue landed or sent back that it sent back (for a
conflict, a failed combined check, nothing left to land or a refused push), the shares sent back for a conflict and by the
combined checks, and the landings each push carried ("Landings per batch"). The
Outcomes page (`outcomes_get`) is that report with the analyst's status: the chain of kind `analyst`, when its last run began and its
next check-back. Every figure is computed on the server; the page renders it: a stat tile per KPI by group, in the server's
order (the figure of the last period, its move as the server reads it, a bar per period, its definition on hover), the analyst's insights newest first
(every chain they name links to its Verification) with the analyst's status, and the tables by role,
model and kind ("By kind"). No list of chains: the insights name the chains that matter.

The analyst is made of existing concepts. The owner starts it once (`analyst_run`; one at a time) as a chain of kind
`analyst` (`crew::analyst`, which no other starter may name); its Chair lives by `subscribe` (every 24 hours unless the owner names another interval), hires a
fresh analyst agent each run with the analyst's brief its kind's section carries, judges its report against the criteria in its brief with
`accept_work`, and records what it accepts with `record_insight`: a decision of kind `insight`, refused when it cites a
chain that does not exist or repeats a headline told within 7 days. The snapshot carries the newest insights and
`insight_added` pushes each new one. `analyst_run` on an open analyst is "Analyze now": the owner's message asks its Chair to
run now, which wakes a waiting Chair (a queued analyst runs when it starts). The owner talks to its Chair to steer it (a
focus, a question, another interval); it finishes only when he says so.

## Routines

Work that repeats, or that waits for a time, needs no one to ask for it each time: the owner tells the Assistant ("every
weekday at 9, check the deploys", "tomorrow at 3pm, look at the migration") and it makes a routine
(`crates/pipeline/src/routine.rs`, tools in `routine_tools.rs`: `list_routines`, `create_routine`, `update_routine`,
`pause_routine`, `resume_routine`, `run_routine`, `reset_routine`, `delete_routine`). A routine is not a form and not a Chair's plan: it is
the owner's words, kept, and a time.

- **When.** Either `cron` (five fields, or `@every 6h`, in the server's time zone; the Assistant reads the zone and the
  time now from `list_routines`) or `at` (one RFC 3339 time in the future): a routine that runs once. A routine never
  repeats more often than every 5 minutes, so a typo does not fill the board. The routine's subscription keeps its own schedule
  (`model::Schedule`: the cron or the one instant, its catch-up window, when it last ran) and `due_at`: when it next starts a chain,
  none once a routine that runs once has run (the routine is kept, for the owner to delete).
- **What a run is.** The scheduler (the `Tick` judges the subscriptions whose `due_at` has come, `crates/pipeline/src/signals.rs`, and `signal::evaluate` fires a schedule's instant under `schedule:<id>`) queues a chain from the routine when its `DueAt` comes: origin
  `routine`, titled "<routine title> · dd/mm hh:mm", the routine's words as its goal, of the routine's kind (`work`, `chore` or `objective`), because the
  owner and the Assistant chose title, words and kind when they made it. The chain then waits for a slot, the
  the owner's review like any queued chain Remit starts on its own. The Chair and the goal judge read
  the goal as the owner's, headed as a routine of his. The run is recorded first, a firing pending with the
  routine's progress; its delivery stores the queued chain with the firing done in one transaction, so a crash leaves the run to be
  delivered or delivered: a run is neither lost nor made twice.
- **A script.** A routine may carry a workflow script (`script`, with `args`, a JSON object it reads as `args`;
  `model::StartSpec.script` and `Args`) when the shape of the work is known before it starts: a fan-out, a loop until a check passes,
  a wait for a signal between two steps ([ADR-0073](adr/0073-a-subagent-is-a-standing-employee-at-work-and-a-workflow-is-a-script-remit-replays.md)).
  A script that fails before its first call is refused when the routine is made or changed. The chain a run queues is stored with
  the run of its script (`queue_start` in `signals.rs`), in the transaction that stores the
  firing done; when the chain starts, its Chair is the run's hands and takes its first turn only when the run ends, with its result.
  A wait in the script is a subscription of the chain whose action is `workflow` (`model::WorkflowRef`: the run and the step): its
  firing is delivered to that step as its result (`{cause, readings}`) and the run advances in the transaction that stores the
  firing done (`fire_workflow`), a scheduled wake to admission. The Subscriptions tab shows such a wait as a check-back or a watch of
  its chain, "A step of workflow <name>".
- **One run at a time.** A routine's overlap is `buffer_one`: while the last chain it started is not done, one next run waits, and
  the Subscriptions tab says so and links the run at work; when it is done, the waiting run starts. Each further instant that
  comes meanwhile is skipped and counted (`Tally.Overlap`). While no provider can run agents, due routines
  are held the same way.
- **Missed times are not replayed.** A server that was off, or a routine that was paused, runs a due routine once, and
  counts the next time from then; an instant missed by more than the schedule's catch-up window, when it has one, is skipped and
  counted instead (`Tally.MissedCatchup`) (from the resume, for a paused one); a routine that runs once and was paused past its
  time runs at once when resumed. A run started by hand (`run_routine`, the Subscriptions tab's "Run now") counts its next time
  from that moment too.
- **Rails.** A routine only starts chains: it lifts no permission and is held by the limit, the provider and its key result's share like any chain Remit starts on its own; a run
  he starts by hand is his, and the share is not asked of it. Pause is the owner's rail on it, with the Assistant able to pull it on his words
  as for a chain. The routine records who made or last changed it (`By`, which `list_routines` shows), and the Assistant's
  conversation shows each call; a chain a routine starts names Remit's scheduler as its actor, or whoever ran it by
  hand. Deleting a routine leaves the chains it started.
- **No routine ships with Remit.** What looks for work, on the owner's own repositories or on Remit itself, is a routine he sets up by telling the Assistant, its goal naming the playbook that fits: its Chair gives its chain `issues`, `customers`, `observe` or `feedback` where it reads those and asks for `propose` (`request_capability`), and he deletes it the same way (ADR-0023).
- **What the owner sees.** The Subscriptions tab: a line per routine with its words, how it repeats (`cron 0 9 * * 1-5`, `once`),
  its next time, its state (scheduled, due, held and why, paused, or under way for a routine that runs once) and, while a
  run is at work, a link to that chain. The open entry's buttons are the owner's rails: run now, pause, resume. Nothing else
  is edited there: he tells the Assistant.

## The signal pipeline

[ADR-0049](adr/0049-every-sense-is-a-sensor-and-signals-are-a-log-a-router-matches-patterns.md) is the shape of it: a sensor senses, a
signal carries, a router matches, a subscription acts.

- **Sensors sense.** A sensor's kind says who senses it (`crates/pipeline/src/sensor.rs`): `poll` and `script` are in-process, a
  plugin adds `plugin:<id>.<name>`, found among the enabled plugins' kinds (`plugin_sensing`, `plugin_pipeline.rs`). A
  record of a kind nobody senses is held silent with the reason and kept. The server's own senses (the door, a landing,
  a change going live, the facts it records) write through the same router and need no record, since the owner configures none.
- **Signals are a log.** Readings are appended to one log; each has a `seq` from one counter (`log_seq`, migration 68), so a subscription's
  place in it is one number, `SeenSeq`. Time is no reading: a subscription keeps its own schedule (below). Each signal has a kind
  (`model::SignalKey::kind`, by its key's family, `model::stream_key_prefixes`): `event:*` and `fact:*` are streams, occurrences each its own;
  every other signal (a chain's landings, its live mark, its sensors) is a state, whose readings each replace its current
  value. The kind sets where a subscription may start and what retention keeps.
- **One path makes a subscription** (ADR-0064, `crates/chains/src/subscribe.rs`). `subscribe` stores every subscription that is made (a
  Chair's watch or check-back, a goal check's deadline, a help's deadline, a routine, a shipped subscription, the owner's rule) with
  the start its maker names (`model::Start`: `now`, a `seq`, a `time`, or `current`, a state's newest reading, refused on a pattern or a
  stream), and checks its settings against its processor (`check_subscription`, `model::Subscription::check`; a plugin's action against what
  every plugin's processor takes, so a bundled plugin's subscription ships before the plugin is enabled). A start is applied once and not
  kept. Each maker says its own: a watch of one state's key starts at `current` (what it says now is judged), a watch of a pattern or a
  stream, a shipped subscription and the owner's rule at `now`; `create_subscription` and `update_subscription` take a `start` (`now`,
  `seq`, `time`), and an update without one keeps the subscription's place. Every other store of a subscription changes one that is
  there.
- **One door.** `ingest` (`crates/chains/src/ingest.rs`, ADR-0064) is the one writer of the log, in its caller's transaction: it refuses a writer
  that may not write the key (a plugin writes only the signals it ships) and a paused signal or sensor (`ErrSignalPaused`, nothing stored),
  takes the reading in at its own time, and answers a reading whose dedupe is stored already with the stored one (`Duplicate`). The caller
  kicks the scheduler after its commit.
- **One router.** `Tick` is the one judge: `tick_signals` hands each active subscription, with the readings its `Match` matches past its
  `SeenSeq`, to `signal::evaluate` (ADR-0064), a pure function of the subscription, its readings, its chain's facts and the time. It answers
  a closed set of effects in a fixed order (documented on `evaluate`): nothing for a subscription that is paused, ended or held, else
  `Fire`, `Skip` with its reason (the condition is false or failed, the reading is its own chain's, a newer one replaced it, the run is
  still busy, an instant came past its catch-up window), `Due`, `Advance` and `End`. The record stage (`crates/pipeline/src/record.rs`), under the Tick's lock, stores them in one short
  transaction per subscription: each `Fire` as a firing `pending`, each skipped reading or instant counted on the subscription (`Tally`) and, for a reading but
  its own chain's, a near-miss (the reason, in table `near_misses`, the newest 20 per subscription), and the subscription's cursor past
  what it judged. The cursor moves when a firing is recorded, not when it is done.
- **Deliver, outside the lock** (`crates/pipeline/src/deliver.rs`, ADR-0064). After the record pass the Tick starts one worker per subscription
  with a pending firing due; a worker hands that subscription's firings to their processor one at a time, in the order they were decided,
  and a paused or held subscription is delivered nothing. Each delivery is cut at its processor's `Deadline`, which every processor must
  declare (the registry refuses one with none), so a processor that hangs holds its own subscription until its deadline and no other, and
  the Tick waits for no delivery. A restart finds the pending firings and delivers them. A firing is keyed by (subscription, key,
  revision) and lists the places of the readings it covers (`Seqs`); its key's text is the `RequestID` every processor is given (a
  plugin's in its `ProcessRequest`, as request_id), so delivery is at least once and an outside effect is keyed by it, while a database effect is
  stored in the transaction that stores the firing `done` (`Delivered`), and so happens once. What a processor answers is handled there
  alone: done already or nothing to act on settles the firing; a firing that must wait without failing (admission, the start quota, a
  start's last chain at work, or a forge that has not answered) stays `pending` with the reason (`Waiting`), which the
  Subscriptions tab shows, and the next pass tries it again; one that fails starts the firing's lifecycle (below). A wake delivered
  this way still comes before new work: the slot it will take is promised to it while its firing waits for delivery, so a queued start
  leaves it free, and the entries that take a slot (the Tick's starts, a wake's delivery) take it one at a time. A wait set again (a
  goal check's deadline moved on resume) cancels the firings of its earlier instants still to be delivered. A kick wakes the Tick
  for a new reading; `sleep_for` wakes it for what has none: the earliest `Due` (a schedule's next instant, a flow's period, a
  deadline).
- **A failing firing holds its subscription in place** (ADR-0064). A subscription's `Control` is `active`, `paused` (its owner's word) or
  `ended` (its deadline came, its one-off schedule fired, a routine ran its last run: kept to be read when it is Remit's, deleted when it is
  a chain's). A firing has a `Status` (`pending`, `done`, `failed`, `skipped`, `cancelled`), its `Cause` (`reading`, `expired` or `scheduled`) and its
  `Attempts`. A processor's error leaves the firing `pending` with the error, and its worker tries it again after a wait that doubles
  from 30 seconds (`signal::settle`), the firings after it waiting behind it; at the fifth attempt, or at once when the error is permanent (`signal::permanent`: no
  processor does the action, or the action is refused), the firing is `failed` and its subscription gets a `Hold` (`firing_failed` or
  `processor_missing`, the error, the firing): it judges nothing more and its cursor stays, the Subscriptions tab shows it held with the
  error, and `fact:subscription.failed` is ingested in the same write, so a subscription can answer it. Nothing is dead-lettered. The
  owner's agent lifts the hold (`lift_hold`, with a `why` and `fact:subscription.resumed`): `resume_subscription` puts the firing back to
  `pending` from its first attempt (`signal::retry`), `skip_firing` gives it up and moves the cursor past its readings
  (`signal::skip_firing`). The Assistant lifts any; a Chair its own chain's. A skip the owner overturns is undone by the Assistant with
  `redo_firing` (`redo_firing`, `signal::redo`): the newest given-up firing is `pending` again, due at once, and the cursor stays. The pending and failed firings of a subscription that is
  deleted are `cancelled`; a subscription that ended with firings still to deliver is kept until its last is done.
- **A held subscription waits for the owner** (ADR-0064, ADR-0062). The write that holds it also raises an inbox item of kind `held`
  naming the subscription (`raise_held`), on its chain for a chain's subscription and on no chain otherwise, and the fact `owner.asked`, so
  the Assistant is put it first (`sub_assist_owner`). It counts in what waits for the owner (`Board.needsYou`) and on the Subscriptions
  tab (`SignalBoard.held`). It is answered `retry` or `skip` with the text as why (`answer_held`); lifting the hold any other way closes it
  (`close_held`), and deleting the subscription cancels it. Each line of the tab carries the subscription's `control` (absent while it is
  active), its `hold` (the failed firing and its error), what its next firing's delivery `waiting` on, and its newest three
  `nearMisses`.
- **Why it did or did not fire is a tool call** (ADR-0064, `crates/pipeline/src/subscription_inspect.rs`). For the Assistant on any subscription
  and for a Chair whose chain holds `signals` on its own chain's: `describe_subscription` (cursor against the head and how many readings
  it has not judged, control, hold, due time, last firing, the failed and pending firings with attempts and next attempt, the newest
  near-misses, `Tally`); `explain_subscription(id, seq)` (`signal::evaluate` on that one reading as the subscription is set now, active and
  with its cursor just before it, the effects, and for a false condition `signal::probe`'s reason, beside what the Tick recorded of it);
  `dry_run_subscription` (a stored subscription, or for the Assistant a draft) and `dry_run: true` on `create_subscription` and
  `update_subscription`, which answer the dry run and store nothing: `signal::replay` runs the evaluator over the kept log from the start
  (by default the kept log from its beginning; a schedule over the last 7 days), at each reading's time and each instant it was due, with
  nothing done (no overlap holds a firing back), at most `DRY_RUN_READINGS` (5000) a call, saying where it stopped; a chain's condition reads
  today's facts. A dry run takes no lock. Pinned by `a_dry_run_says_what_it_would_have_done`.
- **Retention follows the slowest consumer** (ADR-0064). The shipped subscription `sub_run_forget` runs the job `forget` every 15 minutes
  while the server is up (`crates/pipeline/src/forget.rs`, `forget_signals`). A reading goes (`store::Tx::forget_readings`) only when it is not the
  newest of its key, no live (not `ended`) subscription whose `Match` lets its key through has yet to judge it, no `pending` or `failed`
  firing covers it, and it is past its kind's history: a stream's after 7 days (`STREAM_WINDOW`), a state's beyond its newest 1000
  (`STATE_READINGS_KEPT`). So a held or paused subscription loses nothing, and keeps its keys' readings without limit. Firings done,
  skipped or cancelled go after 7 days (`forget_firings_before`); pending and failed ones never.
- **Flow and overlap.** How a subscription's readings become firings is its `Flow` (`model::Flow`): `each` (the default) fires each
  reading; `delay` fires each reading alone, at least `Period` after the firing before; `batch` fires at most once a `Period`, carrying the
  readings held of one key and subject (a watch is a batch of one minute); `last` waits until `Period` passes with no newer reading, or
  `MaxWait` since the first held one, and fires only the newest, skipping the rest. `Overlap` says what a reading meets while the run it
  started is still at work: `queue` (the default) lets it wait, `buffer_one` keeps one waiting and skips the rest. `Until` is a deadline:
  with nothing fired by then, it fires once expired and ends. A processor declares what it takes (`ProcessorSpec`: one reading or many,
  and whether a deadline with no reading can fire it), and a subscription its processor cannot do (a batch for a processor that takes
  one reading) is refused when it is made, with the reason (`model::Subscription::check`).
- **A screen: whether a reading deserves a chain** (ADR-0118, point 2). A `start` on what is heard may carry a `screen`, a statement in the
  owner's words that a decider judges true or false of one reading (`signal.screen@1`; `model::Subscription::screen`, at most 600 characters,
  refused on any other action, on a schedule and on a `batch` flow, whose firing covers many readings). It screens *firings*:
  `signal::asks` names the reading each firing the flow makes is on, before overlap. The shell (`Pipeline::judge`, `pipeline/src/signals.rs`)
  records every subscription with no screen as it reads it, and puts the firings of the screened ones to `Core::screen_firings`
  (`chains/src/screen.rs`) in one round, all within `SCREEN_WITHIN` = 3 s for the whole pass (a decider call is bounded by `DECIDE_FOR` = 2 s; the
  rest is for the store), then evaluates each with the answers (`Input::screens`, `signal::Screen`). Between flow and overlap, `evaluate` applies
  them, pure: a firing whose reading a decider was sure needs no chain becomes a skip (`SkipReason::Screened`, a near-miss that keeps the
  judgment) and frees its place in a `buffer_one` overlap for the next; a firing with no answer fires as with no screen; past
  `SCREENED_PER_PASS` = 20 firings of one subscription a pass the rest are deferred, the cursor held before the first of them, so the next pass
  makes them again. Only a no that acts (at or above the act threshold, measured over nos alone, `ActsOn::No`) drops a firing; the commit that
  records the drop marks the judgment `Acted` (`Tx::set_judgment_used`), until then it reads as escalated. An unsure no, a yes, a refusal, a
  late answer, no decider and an event signal kept on this machine (`EventSignal::kept`; its events take no place among the 20) fire as with no
  screen. 5% of the firings dropped (`AUDIT_PERCENT`, by the reading's place) fire anyway. A reading is asked about under the words of the
  screen (its subject carries their digest), so a corrected screen asks again and an unchanged one reads the first answer. A firing carries
  the judgment (`Firing::screen`), the one place the link is kept; when the chain it starts first finishes, the same commit finds the firing
  by the chain (`Reader::screen_of_start`) and scores the pick (`screen::finished_start`, `OutcomeSource::Chain`): a chain that hired a worker
  did work, one that hired none had nothing to do, one the owner ended, that stalled or whose goal check could not run proves nothing. The
  Subscriptions tab shows the screen on the line and each reading it dropped among the near-misses.
- **Time is the subscription's own** (ADR-0064). A routine, a shipped job, a goal check's deadline and a Chair's check-back (`subscribe`
  with `at`) are subscriptions that keep a `Schedule` (`Cron`, or one instant `At`; `CatchupWindow`; `Last`) in place of a `Match`, their
  next instant in `DueAt` (stored to the millisecond in `due_at`; `wait_for`, `timewait.rs`). When it comes, `signal::evaluate` fires it under
  `schedule:<id>` (`model::SCHEDULE_KEY_PREFIX`) with the instant as its revision, caused `scheduled` and covering no reading, and counts on to the
  next instant from now, so a time missed while the server was off fires once; an instant older than the catch-up window is skipped
  instead (`missed_catchup`). A one-off schedule ends after its instant. A schedule with `buffer_one` overlap (every routine) keeps one
  firing waiting while its run is at work and skips each further instant (`overlap`); skipped instants are counted, not kept as near-misses.
- **Subscriptions process.** The action is the processor: `run`, `end` and `deadline` are code, `tell`, `wake`, `start` and `hire` are an agent, each a
  processor (`processing.rs`, found by `processor_of`); a plugin adds `plugin:<id>.<name>`, which only a subscription of
  Remit may name. `subscribe` (a Chair's watch) takes one exact `key`; `create_subscription` takes a key or a pattern such as `event:*`.
- **Plugins extend it** (ADR-0034, ADR-0037, ADR-0049). `describe` answers a `Manifest` with `sensor_kinds`, `processors` and, for a bundled plugin, `subscriptions`
  (`wire::plugin`, the `Plugin` trait of `crates/plugin-sdk`). A plugin senses with `sense`, a stream the host appends to the sensor's signal (`<signal>.<suffix>`, the suffix a
  word), or with `poll`; it processes with `process`, given the firing's `request_id`, which may write `event:<name>` readings of a signal its own plugin ships. A plugin that
  reads or acts through a connection is lent that connection's credential variables through the one gate (`lend`). The bundled `gitlab` and
  `github` plugins ship their event signals and rules as data in their describe (`crates/bundled/src/gitlab.rs`, `github/rules.rs`); a rule names a core action and
  the expressions that fill its parameters. A disabled plugin's sensors and processors are silent with the reason.
- **Stored data.** Migration 68 gives readings their `seq` and subscriptions `match` and `seen_seq`; 69 gives sensors their kind and turns each clock
  subscription into a clock sensor and a subscription on its key; 70 and 71 move the shipped GitLab and GitHub rules to the plugins' ids; 90
  gives each subscription its flow and deadline and each firing the places of the readings it covers; 91 gives each firing its status
  (every stored one `done`, at one attempt, caused by its readings or its expiry) and each subscription its `control` in place of
  `paused` (a Remit clock subscription with no next instant `ended`), and adds `near_misses`; 92 turns each clock sensor into
  the schedule of the subscription that listened to it (its next instant the subscription's `due_at`), rekeys the clocks' firings to
  `schedule:<id>`, gives routines `buffer_one`, and deletes the clock sensors and their readings.

## Event signals

Everything that comes into Remit from outside is an event of an event signal
([ADR-0024](adr/0024-everything-that-comes-in-is-an-event-signal.md)), whichever way it came: an outside system (GitLab,
GitHub, Sentry, Jira, anything the owner points at it) posting to the signal's door, a script through `remit emit`,
a sensor reading a source (a GitHub issue opened, a Datadog monitor or a Grafana alert that fires, a number Datadog or Grafana computes), or a Chair's
`emit_event`. Nothing is asked of what arrives: no schema, no required fields, no list of senders. Every event is kept as a reading of the
signal, and the signal does nothing else with it. What is done about an event is a subscription to the signal (below): each
subscription holds a condition over the event and does one thing, so an event no subscription holds for is stored and does nothing.

- **An event signal is a way in, and its door has a key of its own.** Each outside system, script or source gets an event
  signal (`crates/pipeline/src/event_signal_keys.rs`, tools in `event_signal_tools.rs`): a name, which is its key `event:<name>` and
  the last part of its door's address (`/hooks/gitlab-billing`: lowercase letters, digits and dashes, at most 40, never
  changed), a note on what it is for, and a key that only its writer holds. The owner tells the Assistant ("open a way in for
  the billing group's GitLab") and it makes one with `create_event_signal` (`list_event_signals`, `update_event_signal`,
  `pause_event_signal`, `resume_event_signal`, `rotate_event_signal_key`, `delete_event_signal`); no page has a button or a
  form for it. A Chair whose chain lists `signals` makes one with its own `create_event_signal`, by its card, for what must be
  read after the chain ends ([ADR-0065](adr/0065-a-chair-makes-the-signal-a-key-result-reads-and-its-sensor-waits-for-the-owner.md)). The key is made by Remit (256 random bits, `evk_…`), sealed under the key file beside the database
  (`-secret-key-file`, the one the library's credentials use) and stored in a column of its own, so no record, push, snapshot,
  decision, transcript or tool result carries it. Only two things read it: the door, to check a post
  (`authorize_event_signal`), and the owner, who presses "Show key" on the signal's page (`event_signal_key`, a read the server
  logs); the Assistant that made the signal is never shown it and tells him to open the page. Pause and resume are the
  owner's rails (buttons, and the same action the Assistant's tools call): a paused signal takes no event from any writer.
  A key that leaked is rotated through the Assistant, and the old one stops working at once. A key file that was lost or
  replaced makes every key unreadable: the door answers 500 (the server's fault), and the owner puts the key file back from
  its backup (see Credentials) or, when no backup of it is left, rotates them.
- **One write path.** Every writer writes an event through `Orchestrator::emit` (`crates/pipeline/src/event.rs`): the door,
  `remit emit` (a client of the door), the poller of a standing sensor and the `emit_event` tool. It refuses a name no
  one made and a paused signal, keeps the first 64 KiB of the body (marked as cut), makes a retry the same event (the writer's
  own id for it, or else a hash of the body within its signal and the ten minutes a writer retries in, answered with the stored one; the same body later is a new event), and stores the reading before its writer is
  answered, so none is lost to a restart, through `ingest`; the Tick that kicks then has each subscription whose `Match` matches the key judge it. The reading's writer says who wrote it, and its event the way it came (`door`, `cli`, `poll`, `agent`, `plugin`).
- **The GitLab and GitHub event signals ship with Remit.** `gitlab` (`/hooks/gitlab`) and `github` (`/hooks/github`) are
  builtin event signals (`EventSignal.builtin`, seeded by `seed_event_signals` in `crates/pipeline/src/event_signal_keys.rs`), built in the way the built-in
  agents and playbooks are: each one's note starts as the code gives it, and the subscriptions that ship with it are data of the `gitlab` and
  `github` plugins (`crates/bundled/src/gitlab.rs`, `github/rules.rs`); `seed_event_signals` makes each at a start that finds it
  missing and brings it up to what ships while the owner has not changed it, and `seed_subscriptions` does so for its subscriptions. When he changes it, as for any event signal, his version is kept through every start (`EventSignal.Edited`) until he has
  it reset (`reset_event_signal`); an edit that sets back what ships is no edit. It is never deleted: the sender is pointed at
  its door's address, and a start would make it again. A `gitlab` signal the owner opened earlier becomes it without losing a
  word of his: it keeps its name, key, pause, sensors, events and settings, which are his version when they differ from what
  ships. Another event signal is the owner's as any, and is the example the builtin's settings give (`list_event_signals`).
- **The door** (`crates/door`, a channel adapter like the browser's), always open at `POST /hooks/<name>`, lets a post in
  only through an event signal that exists and is not paused, and only with that signal's own key: as `X-Gitlab-Token`, as a
  bearer token, as `?token=` for a sender that can set neither, or as GitHub proves it without sending it, an HMAC-SHA256 of
  the body under the key in `X-Hub-Signature-256` (`KeyProof`). Every way of being refused (an unknown name, a paused signal,
  a wrong key, another signal's key) is the same 401, so nothing says which names exist. The key is taken out of what is
  kept, as is every header whose name says it may carry a credential (`token`, `secret`, `key`, `auth`, `signature`,
  `cookie`: another system's key would otherwise reach agents). An agent reads the query and headers, like the body, in a
  fence their text cannot close, and a Chair the door tells reads who acted inside the fence too. A body is read up to 1 MiB
  (413 past it). A sender's own id for its post (`X-Gitlab-Event-UUID`, `X-GitHub-Delivery`, `Idempotency-Key`) is the
  event's sender id. The door answers 202 with the event's address (`{"event":"event:<signal>#<rev>"}`) only once `emit` stored it. A post that carries `X-Remit-Via: cli` came by the
  CLI; the header names a channel and grants nothing. The web server that serves the public address forwards `/hooks` to this
  server, as it does the rest of its HTTP.
- **The CLI.** `remit emit -name <signal> [-url <server>] [-key-file <path>] [-sender-id <id>] [file | -]`
  (`crates/remit/src/emit.rs`) posts a file or stdin to the signal's door on a Remit server, saying it is the CLI, and prints
  the server's answer. The key comes from a file only its owner can read (one others can read is refused) or from
  `REMIT_EVENT_KEY`, never from an argument, which every user of the machine can list.
- **A sensor that polls** (`crates/pipeline/src/sensor.rs`, [ADR-0047](adr/0047-a-sensor-reads-a-signal-carries-and-a-subscription-acts.md)).
  A Datadog monitor that fires may also post to an event signal `datadog`: Datadog cannot sign, so it sends the door's key as a bearer token in a
  custom header only (never `?token=`: Datadog shows a webhook's URL to whoever reads the integration), the key every sender may present (`crates/door/src/lib.rs` `presented_key`); the Assistant writes the subscription
  that starts a chain from it, and `datadog_logs`, `datadog_spans` and `datadog_log_counts` take `chain` to read what the chain named in its tags did (ADR-0119 D3).
  For a system that has no way to post (a GitHub issue opened, a Datadog monitor or a Grafana alert that fires), a standing sensor reads it for the
  signal: it names the event signal it writes, a source, a connection of it, a query in that source's words and how often to read
  it (never more than once a minute). On the scheduler's tick, in the background, `poll_sensors` has each sensor whose period has
  passed read through the sources' gate as the caller `sensor`, in no chain, with the effect read; the gate lets it read exactly
  the source and connection a live poll names, by a typed call (`sensor_verdict`). A standing sensor runs while it and its signal
  are not paused. Each item new or changed since the cursor, oldest first and at most 50 a read, is an event written through
  `Emit` with the item's `id@version` as its sender id, so a read replayed after a crash is the same events. The first read only
  sets the cursor, so a sensor writes nothing older than itself. A source that fails 3 reads in a row is silent (`SILENT_AFTER`,
  `polls.rs`), which the signal's line on the Signals tab says. A source that can be polled implements `source::Pollable`: GitHub
  issues, Datadog and Grafana do, each of the last two in two forms: what fires (Datadog's monitors that leave OK; the instances of Grafana's alert rules that start firing, a word of the rules' names as the query), and a *reading*, one number the source computes ([ADR-0046](adr/0046-a-number-datadog-tracks-is-a-signal-a-pull-whose-query-is-a-reading.md)): Datadog's over a window (a metric, a log count or a RUM count: `reading:<source> [window=] [agg=] <search>`), Grafana's the value of a PromQL instant query on a Prometheus-compatible datasource (`reading:promql [datasource=] <expression>`). A plugin's poll takes one too: the bundled `googleanalytics`'s is one metric of a GA4 property over the last days or minutes (`reading:report metric= [days=] [lag=] [<dimension>=<value>...]`, `reading:realtime metric= [minutes=]`, [ADR-0066](adr/0066-google-analytics-signs-in-as-a-service-account-and-its-number-is-a-reading.md)). A reading is polled the same for every source (`source::reading_poll`): one event a minute it is read, whose body carries `value` for a measure to read (ADR-0045). A sensor
  is a standing use of a credential: one made or changed by anyone but the owner, the Assistant included, that reads a source, a
  connection or a query it did not read before waits for his Apply in his inbox (`hold_sensor`), as a write that moves a rail does.
  The Assistant makes one with `create_sensor`, and the Chair of a chain that lists `signals` with `create_standing_sensor`, for a
  number read after its chain ends, such as a key result's (ADR-0065); a chain never asks the owner to make one by hand.
  A shipped signal (the `gitlab` and `github` doors have none; a bundled plugin's, such as `intercom`) is made with its sensor
  once (`seed_sensor`), and the sensor is the owner's after that. The `script` source (ADR-0044) is read by a sensor whose
  `query` is the code: each printed JSON line `{"id","body"}` is an event.
- **An event is a reading, and every rule is a subscription** (`crates/pipeline/src/event_signal.rs`,
  [ADR-0036](adr/0036-an-event-signal-holds-readings-and-every-rule-is-a-subscription.md),
  [ADR-0014](adr/0014-signal-is-the-backbone.md)). `Emit` stores each event as one reading of `event:<name>` whose value is
  what arrived and the way it came (`{event}`); nothing is derived from it. Its address is `event:<name>#<rev>`. A
  subscription to the key or to a pattern that matches it (`model::Subscription`) holds a condition (`when`, below) and one action; the Tick judges each
  new reading against every subscription whose `Match` matches its key, in the order of the log, and a subscription that holds fires once for that reading:
  the firing (`model::Firing`), the subscription's progress (`SeenSeq`) and the effect's own record are one step, so a restart or a
  retry acts once. Subscriptions do not share an event: each holds its own condition, fires on its own and is paused on its own.
  A subscription made after an event does not hear it; one resumed hears what comes after. A reading caused by a subscriber's
  own chain is not heard by it (`signal::evaluate`), and a tell never tells the chain whose agent wrote the event.
- **What a subscription holds for: `when`.** A condition is an expression in CEL, a language made to run on untrusted input (no
  unbounded loop, no files, no network; a cost limit and a time limit stop one that runs long), over the event as it arrived:
  `body`, its JSON, `header`, its headers keyed as sent, and `cause`. A field the event lacks makes the condition false, and so do
  a body that is no JSON and one that was cut. It is checked when written (`signal::check`) and again when run. The owner never
  writes one: he tells the Assistant, who reads a real event for its fields (`read_event_fields`) and writes it.
- **What a subscription does: one of three actions** (`crates/pipeline/src/action_params.rs`: each kind takes named parameters, each a
  CEL expression over the event, `SubscriptionAction.With`; a parameter the kind does not take is refused when the subscription is made).
  `tell` sends the event to the Chair of the chain it is about (`tell.rs`): `chain` (marks: the branch the event is on or the commit it ran on, found by equality with the chain's branches and
  the commits its landings pushed or landed as, never by a mention in a comment's words), `summary`
  (a line), `quote` and `author` (who wrote the words; a comment by the account Remit works as is never told). No chain, two
  chains or a finished chain makes a firing that tells nobody and says why. Who Remit works as on a host is asked of its forge in
  the background and kept in memory, never while the scheduler is held; a comment waits for the answer, shown as held on the
  Subscriptions tab with the reason, and is given up, saying why, only after 30 minutes without one. The Chair
  reads the event as evidence in a fence its text cannot close. `start` begins a chain from the owner's words as a routine's start does
  (`start_event.rs`), giving it the event as evidence after the goal, at most `-event-chains-per-day` chains a day for the
  subscription (default 3; past it the events wait, kept and saying why, until the next day), held by the owner's review like a
  routine's. `run` runs a job of Remit's own on the chain an event is about (`chain_jobs.rs`): `sync-landing` has the
  forge asked, in the background, for the checks on what the chain landed: what it answers is a reading of the chain's landing sensor (`chain:<id>.sensor.landing.<landing>`), which the bundled `forge` plugin writes when the checks differ from what it last said (ADR-0057's pattern), recorded by the subscription `sub_run_record_landing_checks` (what the event says is never what is recorded), and `mark-live`
  records a change as live, as the subscription, and tells nobody (`live.rs`). A subscription is sent exactly what it
  subscribed to; whatever queue or batching it needs is its own.
- **GitLab's and GitHub's subscriptions ship.** A pipeline (GitLab) or workflow run (GitHub) that finishes on a commit runs
  `sync-landing` for the chain whose landing pushed or landed as that commit, and one that failed is told to that chain's
  Chair (`crates/bundled/src/gitlab.rs`, `github/rules.rs`). They are owned by Remit and builtin: a start puts back what
  ships over one the owner has not changed, and his version is kept (`Edited`). One is never deleted (a start would make it
  again); the owner pauses it.
- **Evidence, never instruction.** An event is written by people and systems outside the owner's control (a public GitHub
  issue by anyone). Its text is quoted in a fence its own content cannot close; the Chair's brief and the goal heading of a chain
  it starts say such text is evidence to verify, whatever it claims to be. A sensor reads with the effect read and a credential no
  agent sees.
- **What the owner sees and does.** The Events page lists every event, newest first, whichever way it came, with the firings it
  caused (which subscription, and what it did or why it did nothing; "No subscription held for it" when none did); the event open
  beside the list (`event_get`, `EventRecord`) shows what arrived as it came, each firing and the chains that were told or started,
  each only as an overview, a click opening the chain. An event signal is two things in the Signals area, as every source of
  signals is: the signal `event:<name>` on the Signals tab, whose page holds its settings (who set it, whether writers are let in,
  its sensors and whether their source is silent, its door's address, "Show key" and a guide to writing to it), its readings with their
  firings and its subscriptions, and each subscription on the Subscriptions tab, one line each, with pause and
  resume as its rails. The owner makes, changes, pauses and deletes one by telling the Assistant (`list_subscriptions`,
  `create_subscription`, `update_subscription`, `pause_subscription`, `resume_subscription`, `delete_subscription`; `dry_run: true` on
  create and update shows what it would have done over the events kept, and stores nothing); a subscription
  of the owner's own is Remit's (`OwnerRemit`) and `By` says who made it. The Assistant reads events too (`list_events`).
  To write a `when` from a real event it reads that event's fields with `read_event_fields` (`fields_of`): their names as an
  expression spells them, each with its kind of value, and a value only when it is a short name or token (`failed`, `Pipeline
  Hook`); the words a writer wrote are shown as text and nothing more, so a comment or a commit message, where an
  instruction could hide, never reaches its context. Nothing is edited on these pages. Events are kept 7 days after they settle.
- **Export.** `remit export -out <dir>` writes every table but the migrations' bookkeeping as JSON Lines, in one read transaction on a read-only connection, leaving out every sealed column; a test fails when a table is neither exported nor named as left out with its reason.
- **The door takes signed posts** (ADR-0030): GitHub's body signature, Stripe's and Slack's stamped ones (refused outside five minutes), Grafana's of the body or, with the `X-Grafana-Alerting-Signature-Timestamp` header the owner sets, of its stamp and the body (refused outside five minutes), and a signed Slack challenge is answered and kept as no event.
- **Extensions** (ADR-0034). A chain that holds `extensions` gives its Chair `propose_extension`: the path of a built plugin program (written with `crates/plugin-sdk`, speaking `wire::plugin` over the link on its stdio, run by `plugin::RemotePlugin`) for a source Remit does not ship. The platform stages it and asks its `describe`; the owner reads its hash, tools with their effects, credential variables, events and frontend, and applies it or not. Apply copies the binary under `<data>/ext` (read-only to sessions) named by its hash, stores the manifest (`extensions` table) pinned to it, and its tools are source tools like any other: the effect ranks them, the chain's grant decides, every call is accounted for. A plugin that polls is also a source a sensor reads on its period. An extension may instead be an API behind the injecting proxy (`auth`: an https address and one of the schemes bearer, basic, header, sigv4, hmac-query), reached by scripts with the credential added by the platform.
- **Extensions: the direction.** The direction is that tool sources, event sources, judges, agents and screens each become a
  plugin. Built: the host, the contract, the owner's Apply and removal, polling. Not built: the Module Federation loader for a
  plugin's frontend (trusted, same origin, mounted in slots its manifest lists), services for plugins through the application
  layer with the plugin as actor (the manifest's `capabilities` field and one policy function are the seam), and a chain that
  writes a plugin's source for the platform to build. The controls that would confine a plugin wait behind those seams (ADR-0034, *Revisit
  when*; `docs/remit-design.md`, K5).
- **Shell and web are capabilities** (ADR-0028). A chain's cards are started with Claude Code's Bash denied unless the chain holds `shell`, and WebFetch and WebSearch denied unless it holds `web`; a Chair that needs one, or any other capability of the catalog that does not write, gives it to its chain with `grant_capability`, at once and on its own word, and the owner reads the decision with its reason; a capability that writes it asks for with `request_capability`, and the Apply of that change item, the Assistant's for the owner, gives it (the item names a proposed, event or client chain's origin for the Assistant to weigh). `release`, `coaching` and `seeding` are bound to their kind (`Capability.kind`): neither tool gives them to a chain of another kind. A capability given reaches the sessions already open: every open session of the chain is told on its MCP tool-list stream (`subscriptions/listen`, `notifications/tools/list_changed`) and lists its tools again, so no session is closed for it; only `shell` and `web`, deny rules a Claude Code session starts with, close the Chair's session to open it again with the tool. The Assistant, which belongs to no chain, keeps its tools.
- **An agent writes an event.** A chain that holds the `events` capability gives its Chair `emit_event`: an event
  of a signal the owner made (a deploy that finished, a check that failed), written through `Emit` with the chain in its
  cause and heard by the signal's subscriptions as any event, except that a tell never tells the chain that wrote it.

## Channels

The web app is one channel; more will come (a command line, a chat). The server owns every piece of state and every
rule; a channel shows a replica it may drop at any moment (a reload, a closed tab, a lost network, a second tab).

- One application layer: what a channel may ask is the orchestrator's actions (its exported methods that take a
  `Request`: create, pause, resume, finish, archive, unarchive, delete, message, interrupt,
  restart, stop, run the analyst, run, pause and resume a routine, answer, write to the Assistant) and what it may read is its reads (snapshot, a chain's
  view, its events, a card's events, context, a landing's diff, the merge queues and the Outcomes page). A channel adapter (`crates/rpc` for the web app, the owner's sessions and a mentor's, `crates/door` for outside systems and `remit emit`) authenticates its user or sender, turns calls
  into these and back, and pushes what the orchestrator notifies. It decides nothing: defaults, caps, truncation and
  who may do what are the orchestrator's, so every channel behaves the same.
- Who acted: every action carries its actor, the owner (`local` with the owner key, his address with Google) and the channel he acted
  through, and records it on what it causes. Rules read the actor's kind (the owner's messages are read first, only the
  owner reopens a finished chain), never a name.
- Once: every action carries the channel's request id, the same for each retry of it (an answer that never came). The
  server does it once and answers each retry as it answered the first, its error included; one a restart cut off is
  answered as cut off, not done again. An action runs to its end whatever the channel does, and a stopping server
  finishes the ones under way before it closes the database.
- Offline (ADR-0094): while the link is down a channel keeps showing the replica it holds, saying since when it dates
  and when it tries again, and what changes something is refused with words that say so. The owner's sends wait in the
  client core's outbox (`replica::outbox`), shown at once where he wrote them, and go when the link is back with the
  request id they were given when asked, so a send replayed after a reconnect acts once; the shell keeps them across a
  reload. What the page says of the link is the core's (`State.banner`, `replica::banner`); the shell renders it.
- Changes reach channels through one notifier (`orch::Notifier`), once stored and in the order they were stored. Each
  record carries its revision (`rev`, counted by the store at every write), a streamed chunk its offset in its message.
  A channel subscribes, then reads; what is pushed while a read is on its way is applied over its answer; of two copies
  of a record it keeps the higher revision, and it skips a chunk that does not continue the text it holds until the
  message's next full copy. The web channel disconnects a browser that falls 512 changes behind; it connects again and
  reloads.
- What a card shows of the moment (what its agent is doing, how full its context is) changes many times a turn: it is
  held with the agent's session, pushed as a small `card_shown` at most once a second per card (the last one always),
  read with the card while the session runs, and stored with the card's next write. Everything else about a card is a
  write and a push of the card.
- Where things stand is computed once, by the server (`crates/orch/src/standing.rs`), and shown by every channel as it is given:
  the board (`Board`: its columns and their order, the queue's order, what waits for the owner from no chain, the
  count of what waits for him, and the waiting list itself, `waiting`, which the browser's ⌘K palette, its `n` key to the next
  wait and its notifications read, ADR-0078), each chain's standing (`ChainStanding`: its badge, its first wait and how many it
  holds, whether it finished since the owner last viewed it, whether it works or asks something,
  when it checks back, its place in the queue and what holds it there, why it stands where it stands (a queued chain's
  Overview and an event's chains say it), who it came from, and what the owner can ask of it: pause, resume, finish,
  archive, unarchive, delete, each offered exactly when `step` would take it, so the rule that refuses a command is the
  one that offers it), each agent's standing (`AgentStanding`: its badge, what it is doing in a line, what waits on the
  owner with it, its place in the tree, in a turn the message it is on and whether it has begun to answer it (which
  every receipt under a message to it reads), and what the owner can ask of it: a message to its open session (queued after the turn in progress,
  steered into it, or interrupting it: `Delivery`), one that resumes its closed session, an interrupt, a stop, a restart, and in
  words what a message sent now does (`on_send`); its composer offers those and nothing else, ADR-0080), each
  mentor's standing (`MentorStanding`, `crates/orch/src/mentor_standing.rs`: whether they are in one of their apps now and on what, when
  they were last active, those in `Presence::words`' words, the codes they work from and their open helps), each
  message's title (what the turn it starts is about), and the capabilities' labels. Each colour it names is a closed set in the contract (`ChainTone`, `AgentTone`, `DoingTone`, `CheckTone`), so a channel maps every value it can be sent and no other.
  What waits and what fires on its own (`Snapshot.signals`, `crates/pipeline/src/signalboard.rs`: each subscription's line and every signal's newest
  reading, in the order to look at them, a due one held by the reason admission gives) is computed
  at every tick and pushed when it changes (`signal_board_updated`); the scheduler's figures (`Workload`) go with
  `workload_updated`. A signal's readings and firings are read on demand (`signal_get`).
  The snapshot carries the board, the newest events, every chain's standing, every mentor and every mentor's standing, a chain's view its agents'
  and its own; when a record they are made of changes, the server computes them again (at most every 200 ms) and pushes the
  ones that changed (`board_updated`, `chain_standing_updated`, `agent_standing_updated`, `mentor_updated`, `mentor_standing_updated`, `event_read`), each at a
  revision above the last it gave. The board is laid out from the records the server keeps of it (every chain, its
  verdicts, the open inbox), read whole at each snapshot and otherwise again only for the chains a change moved; a
  chain's standing is computed again when what it is computed from moved (the chain, its verdicts, its open items, its
  column, its place in the queue, whether the review cap holds it), so a change to one chain costs one chain, not the
  whole app.
- What grows without bound is read a page at a time, never whole in the snapshot: the archived chains (the snapshot
  carries only how many there are; `chains_archived` reads them newest change first, from a cursor of the last one
  shown, with their standings), a card's events (`card_events` reads the page before the oldest one held), and a skill's
  files (the snapshot lists their paths and sizes; `skill_file` reads one).
- A failed call says what kind of failure it is in its error's code (`ErrorKind`: `NOT_FOUND`, `ENDED`, `UNAVAILABLE`,
  `FAILED`), which a channel acts on instead of reading the message.
- The browser names the bundle it runs in its handshake; a server serving another one turns it away with "new bundle",
  and the page loads again (at most once a minute), so a page left open across a deploy never talks to a server whose
  calls and pushes it does not know; the banner that says so carries a Reload button (ADR-0081).
- A channel adapter may: authenticate its user, turn his calls into the orchestrator's actions and reads, turn records
  into what its medium shows (layout, colours, icons, a time in the user's clock), keep view state (what is open,
  scrolled, folded, filtered; in the address for the web), and push what the orchestrator notifies. It may not: decide
  what a record means (what waits for the owner, a status's grouping or order, a label, a count, what an agent is
  doing), default or cap what the orchestrator did not, or keep state the server does not have.
- A new channel plugs in by implementing `orch::Notifier` (it is told every change, derived views included, and fans
  them out to its users), calling the orchestrator's actions with a `Request` that names its user and itself
  (`model::by_owner(who, "<channel>")`), and reading `Snapshot`, `View` and the event pages to start from.
- The owner's unsent words are server state too (Draft): a channel saves them as he types (the web app after a pause,
  one save of a draft at a time, under an id it makes, so a save sent twice is one), shows what he typed over the copy
  pushed until its save is answered, and sends a draft by its id once what was typed is saved. A save that arrives
  after the draft was sent or deleted is refused, so a sent draft never comes back.
- Agents write the Markdown the web app shows, so the app is served with `Content-Security-Policy: img-src 'self' data:
  blob:`: an image in an agent's text never makes the owner's browser call an address the agent chose.
- The web app asks nothing while its link session is down (the owner is told nothing was done, and the call is not sent
  later when the session comes back), and waits a minute at most for an answer.
- The web app says when what it shows may not be current (a banner under the top bar: connecting, lost with the
  reason, loading the latest data, or not loaded and trying again) and is live only once the snapshot and the open
  chain are loaded again after a (re)connect. It gets back in by itself after a restart, including where the server
  turned it away for now; only "bad token" stops it, and shows the way in.

## Client keys

An outside agent (the doctor that evaluates Remit is the first) acts on the live system through a key of its own, a third way
in beside the web app and the event signals' doors. The decision is `docs/adr/0009-client-keys.md`; the threat model is at the end of
this section.

- **A key is a way in** (`crates/keys/src/client_keys.rs`, tools in `client_key_tools.rs`): a name (lowercase letters, digits and
  dashes), a note and limits (chains a day, 0 for none, 3 when left out; acts an hour). The owner tells the Assistant ("give the doctor a key") and it makes one
  with `create_client_key` (`list_client_keys`, `update_client_key`, `pause_client_key`, `resume_client_key`,
  `rotate_client_key`, `delete_client_key`); the Client keys page (under Integrations) lists them, with "Show key" (a logged read the
  Assistant never gets) and Pause and Resume, the rails the owner pulls himself. The key is `ck_` plus 256 random bits, sealed
  under `-secret-key-file` like an event signal's, and listed in `sealed_key_ids`.
- **The door is `/mcp/client`** (`crates/clientmcp`), on this machine only: a request without a loopback address, or with a
  forwarding header, is refused before the key is read, and every refusal answers the same. Its holder reads the key from a file
  of mode 0600, never a prompt or an argument; `-client-key-files` names those files so agents are asked before reading them.
- **The actor is `client`**, never the owner: `act` refuses it, and the rules that read an actor's kind fall closed for it. Its
  tools (`crates/orch/src/client_tools.rs`) go through `client_act`, which checks the key is live, counts its acts an hour, does an
  act once per request id and records the key on the decision it causes.
- **The tools are an allow-list**, pinned by `a_holder_is_given_exactly_the_allowed_tools_and_kept_to_its_limits`: reads (`list_chains`,
  `read_chain`, `chain_events`, `card_events`, `read_mr_changes`, `read_outcomes`, `read_board`, each bounded and paged) and
  `start_chain`, `pause_chain`, `finish_chain`, `stop_card`, `cancel_turn`, `message`, each with a `why`. A card it steers belongs
  to a chain; the Assistant is out of its reach.
- **Its chains are experiments.** A chain it starts has origin `client` and kind `work` (ADR-0113), so it starts with no
  capability, within the key's chains a day (0 starts none) and the review-room cap. It never publishes through Remit (`submit_step` refuses it), is
  left out of the per-kind counts, and its goal and messages reach agents as an outside agent's words. Its Chair hires no engineer.
- A key paused or deleted stops its next act; the chains it started go on, and the key's page lists them.
- **The endpoint answers this machine only** (no forwarding by the public web server) and reads are an allow-list, paged and
  bounded to 40,000 characters with a cursor. A key cannot read delivery bodies, the Assistant's drafts, command arguments or
  plain values of a connection, webhooks, keys or credentials. Writes are idempotent per key: each carries a `request_id`
  namespaced `client:<keyId>:<id>`, and a repeat with different arguments is refused. Absent on purpose, and a test fails if one
  appears: resume, answer, archive, delete, land, revert, any budget or cap, credentials, keys, webhooks, deliveries, playbooks, skills.
  Limits default to 3 chains a day and 60 acts an hour. The key is sealed and re-showable, not hashed, because the
  owner must see it again to put it in a file.

**Threat model.** Method: Shostack's four questions, a data-flow diagram with three trust boundaries, STRIDE on each crossing,
the OWASP LLM Top 10 for the agent that holds the key (it reads text outsiders wrote). B1: the key-holder's process to the
endpoint. B2: text outsiders wrote (webhook bodies, Intercom and Jira text copied into events) to the holder's
model. B3: Remit's data to the holder's own store. Reviewed by no one but its author.

| # | Threat | Control | Residual |
|---|---|---|---|
| 1 | **Spoofing**: a leaked key is used (B1) | the key lives only in a 0600 file, never in a prompt or argument, and the file joins the ask rules (ADR-0004); sealed at rest; pause and rotate act at once; last use and source address on its page; failed attempts logged; requests not from this machine refused before the key is read | an agent steered into reading the key file, or the database and `secret.key`, holds the key: the ask rules are a tripwire, not a boundary. Closed only by host isolation |
| 2 | **Tampering**: the holder changes what it may not (B1) | `client` is not `ForOwner`, `act` refuses it, the tool allow-list is pinned, client tools write no definition, cap, credential or key | none known |
| 3 | **Tampering**: it makes agents do what it says (B1) | messages are fenced as outside text at ordinary priority; a client chain is a `work` chain, starts with no capability and does not publish | a Chair may act on information it was sent, as for a webhook |
| 4 | **Repudiation** (B1) | every act has the actor `client · <key>` on a decision of the chain, counted on the key's page; request ids namespaced per key | none |
| 5 | **Information disclosure** (B1) | allow-listed reads; no key value in any snapshot, event, decision or log | agent event streams hold whatever an agent printed; the holder sees what the owner sees |
| 6 | **Denial of service**: spend (B1) | chains a day, review-room cap, acts an hour | up to its chains a day per key, each running until its work ends: Remit holds no cap on cost ([ADR-0010](adr/0010-remit-records-no-cost.md)) |
| 7 | **Denial of service**: a stolen or confused key stops the owner's work (B1) | pause and stop are restrictive and reversible by the owner; acts an hour; each stop is on the chain with its `why` | work is interrupted until the owner resumes; resume is his alone |
| 8 | **Elevation**: the key becomes the owner (B1) | default deny: `act` refuses it and rules that read an actor's kind fall closed; a test enumerates the exported actions and fails if one accepts `client` off the allow-list | a future rule that reads `Kind` carelessly |
| 9 | **LLM01, LLM06**: text the holder reads tells it to stop chains, message a Chair or start work (B2) | no tool lifts a rail or spends beyond the key's limits; every steering act needs an auditable `why`; rate limited; the holder's procedure treats transcripts, events and landing titles as data | an injected holder can pause and stop chains up to its acts an hour and start its chains a day |
| 10 | **LLM02**: chain content reaches the holder's transcript (B3) | same machine and account; reads exclude delivery bodies and drafts | merchant data in agent output is copied into the holder's stored transcript; accepted for the doctor |
| 11 | **Noise**: experiment chains pollute Outcomes and the board (B1) | origin `client` is left out of the value figure and labelled with the key's name | none |

Revisit when: a holder appears on another machine (TLS and forwarding deliberately, reconsider 10); a key gains a tool; a rule is
added that reads an actor kind; the first incident from an injected holder; host isolation lands (ADR-0004).

## The Assistant

The owner's agent for the whole app (`crates/chains/src/assistant.rs`). The web app shows it as a column at the right of every
screen, open across navigation (`?assistant=1`); it is one conversation, the same on every page and channel.

- **What it is.** A record and its one card (origin `assistant`), in no chain, made at the first start
  (`EnsureAssistant`), in a directory Remit owns (`<data>/assistant`), on `-assistant-model` at `-assistant-effort`
  (default `opus`, `medium`), which its card follows at every start (ADR-0070). Its card's stream is
  the conversation. Nothing of a chain holds it: no pause, no cap, no slot, no stall rail. It runs a turn only on a
  message: the owner's, an item put to it, or an agent's reply to a message it sent. The repetition rail holds its turns
  as any agent's (ADR-0092); across turns, an exchange with agents the owner is not in is its loop: every `-spin-turns`
  turns in a row it took on agents' replies, `message_agent`'s answer carries a reminder (n of 3), and past the third
  the tool is refused until the owner writes or an item comes to it (ADR-0104).
- **Its writer.** Card-scoped records name their home, a chain's id or the Assistant's. A message to the Assistant, the
  owner's draft it ends, a draft saved and what it spends are stored and pushed one at a time under its own lock, as a
  chain's writer does for a chain (`change_assistant`, `change_home`); messages are records of the change that sends them, so
  any writer can queue one for it in its own transaction.
- **Where the owner is.** Each of his messages is sent with where he is (`Where`: the page and what is open on it: the
  chain with its tab and agent, the playbook, agent, skill, source, event, feedback, objective, employee, mentor, and the
  project the board or the Projects tab is narrowed to). When he
  moved since the last message that said it, the server names it in words and stores it on the message (the chip under
  it); his words reach the Assistant as he wrote them. The Assistant asks with `where_is_owner`: the place, its label,
  the manual reference that says what can be done there and the tools that read it, from the `PAGES` table
  (ADR-0086).
- **Its sessions.** A new session (the first, after the owner starts over, after its context passes 85%, after its
  provider failed) reads the Assistant's brief and a recap of the conversation so far ahead of the message that opens
  it (`assistant_lead_for`). "Start over" is the card's restart: the session closes, the next message opens a new one, and
  the conversation shown begins anew at it: a card's page of events (`CardEvents`) begins at the newest Start over
  (`started_over`, a session break the owner asked for, which a channel reads as the event's `Activity::StartedOver`), and
  the browser's replica begins again when one is pushed. The
  recap still covers the whole conversation. A restart of the server cut it off mid-turn resumes the same session.
- **What it may do.** Its tools (`assistant_tools.rs`) read everything (chains, the board, the signals, the inbox, the
  events and their firings, a chain's agents, an agent's conversation, outcomes, playbooks) and take the owner's own actions for him: start a
  chain of the kind the work is (an `author` chain for playbooks, agents and skills), message any agent (read first, as his, headed as sent through the Assistant; a finished chain reopens), run the analyst, pause, resume, finish, archive and unarchive a chain, make, change,
  pause, resume, run and delete a routine, open, change, pause, resume, rekey and delete an event signal (never shown its key), make, change, pause, resume and delete a standing sensor (`create_sensor`, `update_sensor`, `pause_sensor`, `resume_sensor`, `delete_sensor`), make a connection and ask for its credentials (never given one), mark a connection protected (`set_connection_protected`), answer the inbox, permission requests included, take a landed change back on his word (`revert_landing`, see Shipping), set how long a permission prompt waits for the lead or him before its driver decides (`set_permission_wait`, 1 to 1440 minutes), and read how Remit's deciders do (`read_judgments`, the Judgments page) and steer them: which model answers a question first (`set_decider`) and how sure an answer must be to decide alone (`set_target_precision`, 50 to 99 percent and never below the question's floor: 90 where a wrong act loses something, as a lesson merged away or an event screened out), a model chosen only informing until it has 30 results of its own, each recorded in the owner's settings with who set it and why and said in the page's first sentence (ADR-0118, point 8). Each is the same orchestrator action the owner's button calls, recorded with the
  Assistant as its actor (`ActorAssistant`): the decision log tags it "Assistant · for you", and a permission answer is a
  `permission` decision whoever gives it. It cannot delete a chain (that cannot be undone), land a
  change or push, and it edits no file (the no-edit deny rules): work happens in chains.
- **Objectives, employees and memory.** The Assistant drafts what the owner applies: `draft_objective` and its amendments,
  `draft_hire`, `draft_retire` and `draft_move` (it reads with `list_employees`), and `draft_forget` (it reads an employee's memory with
  `read_memory`). Each is a change item in his inbox, and one it raised is put to it like any other (`fire_assist`): it may apply it
  for him, or leave it to him (see Objectives, Employees).
- **It decides for him first** (ADR-0062). An agent's question to him (`ask_owner`) and every change item, its own included, are
  stored in his inbox with the fact `owner.asked`, in the same transaction; the shipped subscription `sub_assist_owner` puts the
  item to the Assistant (`fire_assist`, `crates/orch/src/assist.rs`), headed `[for you to decide · i_…]`. It answers with
  `answer_inbox` and a required `why`, or leaves the item to him with `leave_to_owner`; whichever answer comes first, his or
  its, closes the item, except one that waits out the overturn window (below). A question comes with the other chains' questions still open (`already_asked`), so the same
  question asked twice is decided once, the same way, and one that repeats an item left to him is left to him too, naming
  it. What it may do for him is one rule, `for_owner` (`owner_acts.rs`): every act of his but a secret
  (typing a credential or a sign-in code, reading a key's or a code's value), which stays his alone.
- **A prior before its turn** (ADR-0118, point 5). `fire_assist` first asks the deciders `inbox.option` (a choice over the
  item's options, each named by its id) and `inbox.reversible` (can answering be undone later at little cost), through
  `Core::prior_for_item` (`chains/src/inbox_prior.rs`), and ends its message with what they said, as a prior: it still
  decides, and `Acts::Informs` is all the answers do. A credential or a sign-in is never asked; no decider, or a late or
  refused answer, leaves the message as it was. A second judge (ADR-0103) is also started on every answer that parts from
  a decider sure at 90% or more (`Prior::dissents`: it took another option, or approved what the decider holds cannot be
  undone), besides the random 8%; the judge's own answer scores the deciders' picks (`opinion_given`), and the owner's
  overturn of an answer that took a decider's option scores that pick wrong (`overturned_answer`, in the overturn's
  transaction) and is final over the judge's, in either order. A second opinion started by a dissent (`targeted`) is shown
  but left out of every standing, since it only scores picks that part from the Assistant.
- **Its feed.** Each decision it makes for him is an `assist` decision on the asker's chain, citing the item, with its reason;
  the feed (`AssistantFeed`, `assist_feed.rs`, pushed as `assistant_feed_updated`) shows the last 50 and counts those since he
  last viewed it (the `assistant.feed` seen mark, ADR-0079). He overturns an answer with his own word (`overturn`): an `overturn`
  decision cites it, the agent that asked reads his word as his message and undoes what no longer holds, and the Assistant is
  told `[overturned · d_…]`.
- **The overturn window** (ADR-0101, `crates/orch/src/overturn_window.rs`). Its Apply of a change item whose write lets
  something on the top rung of the effect ladder happen (`waits_out_window`: a grant at `irreversible`, an extension, an
  outside MCP server included, whose tools the platform did not rank) is stored with the item `decided` and
  `takes_effect_at` 60 minutes on (`OVERTURN_WINDOW`), and nothing is written. The agent that asked is told
  `[change result · decided]` at once and works on; `finish_chain` waits for the item as for an open one. Each Tick,
  `take_effect_due` applies every decided item whose time has come through `answer_change`, as the Assistant, in the
  transaction that closes it, so a restart mid-window resumes it and it is written once. The owner's overturn within the
  window closes the item cancelled in the transaction that records the overturn (`stop_decided`), and nothing is ever
  written. His own Apply takes effect at once. Its feed line carries `takesEffectAt` while it waits, which the panel shows
  as a countdown beside Overturn; a permission request is not held.
- **Its digest.** On the owner's words it posts a digest of the last day or week in its panel on a cron he gives
  (`set_digest`, `list_digests`, `stop_digest`, `digest_tools.rs`): one Remit subscription per period
  (`sub_digest_day`, `sub_digest_week`) running the job `digest-day` or `digest-week`, whose firing writes a message on
  the Assistant's card in the write that stores the firing done. `digest.rs` builds it from the store by pure code, with
  no model call and no turn: the chains that finished in the window with the board's words for how each ended, the
  Assistant's `assist` decisions and the owner's `overturn` decisions.
- **The owner is its mentor.** At start the owner is the mentor `m_owner` with an accepted help on the Assistant's card
  (`ensure_owner_mentor`, `owner_mentor.rs`), so a code made for him lets him talk to the Assistant from the mentor's app
  as any mentor steers the card they help; the panel is one client of the same conversation. He cannot be removed from
  the mentors.
- **Mentors.** The Assistant keeps the mentor pool on the owner's words, with no item to apply: `add_mentor`, `list_mentors`,
  `remove_mentor`, and their codes (`create_worker_key`, `list_worker_keys`, `revoke_worker_key`); it never sees a code's value
  (see Mentors).
- **Replies.** A turn that answered a message the Assistant sent queues the agent's answer for the Assistant in the
  transaction that records the turn's end, so the Assistant wakes and tells the owner; the owner's own messages to an
  agent do not wake it.
- **Known risk.** It acts with the owner's powers on text others wrote (goals, reports, issues). Its brief takes
  instructions only from the owner's words and treats the rest as evidence, every action is recorded with it as the
  actor, and what it may do can be undone except an allowed permission request; an irreversible grant or an extension it
  applies waits out the overturn window first.

## The provider

Every agent runs on Claude Code (the `claude` CLI, `-agent`), the one provider, signed in with the machine's own Claude Code
login: `-claude-config-dir`, default `$CLAUDE_CONFIG_DIR`, else `~/.claude`; no API key is set. The AI runtime holds the
config dirs and their logins (ADR-0058): the core asks it over `wire::runtime` to prepare an account, sign one in or out and read
its address, and keeps choosing accounts, failover and the breaker itself. Agents run in a config dir
of their own, `<data>/claude` (`agent_home::prepare`, run by the runtime as it starts), holding a link to that login and nothing else: the plugins, skills and
settings installed in the machine's config dir never reach them, and their transcripts are written under it. That login is the
account `default`. A machine with none still runs: `default` is then a signed-out account, out of use like any other
(its probe fails, failover), and agents run on the accounts the owner added; the owner can add more (below), and a session runs on one account at a time. `crates/provider` is the
one catalog of the models agents run on: `fable`, `opus`, `sonnet` and `haiku`, cut at `-top-model` (default `opus`) so no card runs above it (ADR-0133), each with its tier (`apex`, `frontier`,
`workhorse`, `fast`), and of the efforts a session thinks at (`low`, `medium`, `high`, `xhigh`, `max`; empty is Claude Code's
default), which reach Claude Code as `--model` and `--effort`. A card stores its model id, its effort and the account it runs
on. The Chair names a hire's model or effort only when it knows better (`models_help`). A model it leaves empty, and its agent leaves empty, is chosen by the decision layer from the brief (`hire.tier_can` for each tier below the strongest, the cheapest tier that acts wins; ADR-0118) when a decider is sure, else it is the Chair's own; the effort is the agent's, else the Chair's (`hire.effort` only informs). A card whose model was so chosen climbs a tier by code when its turn breaks or its gate sends its change back twice (`chains/src/climb.rs`); a hand-over, a workflow's hire and a duel's sides ask no decider. The Chair and the Assistant run on `opus` at `medium` (`-chair-model`, `-chair-effort`,
`-assistant-model`, `-assistant-effort`), and every judge (the goal, duel and second judges, and a card that signs a
gate) on the capped catalog's strongest model at the Chair's effort, so none grades an author stronger than itself (ADR-0102, ADR-0133). A flag that names a model or effort the catalog
does not know refuses to start the server. See ADR-0006, ADR-0007 and ADR-0070.

- **Starting a session.** The core's one session port is `agent::Launcher`; `runtimelink::Launcher` implements it over
  `wire::runtime`, and in the runtime `agent::ClaudeLauncher` starts every session as a Claude Code process of its own, through
  `crates/claudecode`, Remit's own client of the Agent SDK protocol (newline-delimited JSON on stdio, control requests both
  ways): its model, permission mode, allowed, denied and ask rules, tools, plugins, settings layers and MCP servers go on
  its command line, its skills and appended instructions in the initialize request (see Agents and the library). A card's
  mode is Claude Code's permission mode. Claude Code asks every permission over the control channel (`can_use_tool`),
  and `agent` turns what the process writes into Remit's own terms (`agent::Update`: text, thinking, tool calls with
  their kind, the plan, usage), so nothing past `crates/agent` reads Claude Code's messages. Those terms are declared once,
  by the runtime contract (`wire::runtime`: `ToolCall`, `Note`, `Task`, `Limit`, `Failure`, `PermissionRequest`,
  `PermissionAnswer`, `StopReason`), and the port speaks them on both sides of the link, so nothing maps one copy onto
  another (ADR-0107 decision 4). What Claude Code reports a session loaded and what fills its context are
  `model::SessionInit` and `model::SessionContext` as reported; the store keeps the newest of each by card with when Remit
  heard it (`InitReport`, `ContextCount`).
- **Subagents in a session** (ADR-0073). A session may be given subagent types (`agent::Spec.agents`, sent in `initialize`) and
  the built-ins it keeps (`Spec.BuiltinAgents`): Claude Code's subagent tool takes no allowlist, so every other built-in is
  denied by an `Agent(<type>)` rule, which holds inside a Workflow script too. `initialize` asks for the subagents' messages
  (`forwardSubagentText`); `agent` gives what a subagent writes, the tools it calls, its permission requests and the tasks it
  starts to the handler its spawner's handler gave (`Handler.OnSubagent`, `agent::Subagents`), and the subagent itself, from
  its start, as a `Task` of kind `local_agent` to its spawner's `OnTask`. A Workflow run is a `Task` of kind
  `local_workflow`: its script, the handle that resumes it (`RunID`, `ScriptPath`) and its agents as its progress says them
  (`Task.Agents`); a run's agents stream nothing to the session. `Session.StopTask` ends one task (`stop_task`). Across
  `wire::runtime` each entry names the subagent that did it (`agentId`), and a permission request names the last entry before it
  (`afterSeq`), so the core hears a spawn before its subagent asks; `claudecode` likewise asks a permission only after the
  messages written before it.
- **Who a session may spawn** (ADR-0073, `crates/chains/src/subagents.rs`). Every card keeps the built-ins `general-purpose` and `fork`,
  which run as its own employee and call Remit with its own token (a fork is the card itself in a second context); a Chair is also offered each active worker it may put to work (within its ceiling), and the
  Assistant every active worker, each as a type named by its handle (`employee_handle`: its name, else its template, and its
  number, as `ana-12`). A type is the employee as a worker card holds it (`library::access` of its loadout: instructions,
  skills, tools narrowed to the parent's; edits and Bash denied under a read-only ceiling). A judge spawns nothing (`Task`,
  `Agent` and `Workflow` denied) and a Chair runs no Claude Code `Workflow` (its session closes whenever its chain waits; its
  scripts are Remit's). The set is fixed when the session starts.
- **A subagent's own identity.** Each type reaches Remit through a server of its own, `remit_seat`, with a seat token
  minted for (the run, the employee) beside the card's and revoked with it (`end_run`), and is denied the parent's `mcp__remit`
  (a type's server named as the parent's would carry the parent's token, PoC P2). `CallerForToken` resolves a token to a card
  or a seat; a seat is served its employee's own tools (`search_memory`, `rate_memory`, `my_work`, `read_my_card`, which with no id reads its newest
  child card, and `send_feedback`, recorded on that card) and the reads its parent's
  chain gives every agent (history, feedback, issues, outcomes, signals, coaching reads, sources through the grant gate as the
  parent), and any other call is refused with "a subagent works for <parent>: say it in your report".
- **The rules Claude Code enforces.** A card's deny rules (`deny_for`) reach Claude Code as settings, and `on_permission`
  reads the same rules against every request that reaches Remit (a command by its prefix, each command of a compound
  line, a script given to `sh -c`; an edit tool by name) and refuses a match without asking the Chair or the owner.
- **Accounts** (ADR-0040). An account is a Claude login, kept in a config dir of its own and known by the address it signed in
  as: `default` is the one above, always there, never removed, and known by the address its login names (read at start, `prepare_accounts`); the
  others live in `claude-accounts/<id>` next to the machine's config dir, each with its `projects/` linked to the default one so a session resumes on any account. A card's
  `Account` is set when its session starts (its own while usable, else the least loaded usable one; choosing and storing it is one
  step, so cards that start or move together are spread) and the launcher gives the session `CLAUDE_CONFIG_DIR` for it. The
  owner adds and removes accounts with the buttons of the Accounts page (Integrations);
  both are rails, so no agent has a tool for them, and the Assistant has `list_accounts` only. Add account runs
  `claude auth login --claudeai` in a new config dir through `crates/claudecode` (`start_login`): the server reads the link the
  process prints and shows it in an inbox item of kind `login`, on the page. The owner opens the link; the sign-in ends by
  itself when his browser reaches the machine, otherwise he types the code the page shows into the item's write-only field
  (`submit_login_code`), which no agent sees. Then `claude auth status --json` names the address and the account is kept under it;
  an address that exists is replaced by the new login, a sign-in that does not end signed in keeps nothing. A new account counts
  as out until a probe of it answers.
- **When the provider fails.** A turn or a session that ends because of its provider (an account signed out, a rate or
  usage limit, an overloaded or down service) is a `wire::runtime::Failure`; any other error stays what it was. A turn the service
  failed is tried again on the same session first, twice, after about 20 and 40 seconds (each wait spread by a quarter
  either way, so cards that failed together do not come back together), with a `retrying` record in the stream; a limit or
  an account, which waiting does not fix, is not. Then the card **waits**: its session closes, the message its failed turn
  was on goes back in its queue, its status is `waiting` and its activity says why the provider is out, and a `waiting`
  note in the stream says so. Running out of capacity is not the owner's to answer, so nothing goes to his inbox: the
  Health bar already says the provider is out, once, and an agent waiting for it shows it on its own badge. A card whose
  provider is out of use when a message waits for it waits before its turn, so no turn is spent finding out.
  A limit or an account takes the account the card ran on out at once; a service is out once 3 or more of its turns, and at
  least half, failed in the last five minutes (a breaker on the failure rate, so one blip stops nothing). The breaker is kept
  per account: a card whose account is out moves to another usable account and resumes its session there, and waits only when
  no account is usable. An account is used again once it
  answers: a turn of a card that it answers, or a probe (one turn on its fastest model) that the scheduler runs in the
  background when its cooldown ends (about two minutes after a service failure, fifteen after a limit or an account, each
  spread by a quarter either way), so the owner's work does not pay for finding out; a probe that fails starts the
  cooldown again. The scheduler looks at a waiting card again after its failure's cooldown, spread a quarter either way so
  that cards which waited together do not start together once the provider answers, and opens it if the provider can run
  it; while it cannot, the card waits on. A restart keeps the wait (its message stays queued, and the card is opened with
  the others and waits again while the provider is out), and a chain whose agent waits is not nudged as stalled. See
  ADR-0005. `outages` (`crates/chains/src/health.rs`) is the one owner of whether the provider can run agents; the health the
  owner reads (the bar under the top bar, `health_updated`) is computed from it and pushed when it changes, whoever found
  out: a card's turn, a probe, the check at start (the scheduler starts after it) or the owner's "Check again". While the
  provider is out, the scheduler starts nothing (see admission, above); while every account that can run agents is near
  its limit, it starts nothing on its own, so the owner and the Assistant keep the room left (ADR-0092).

## Capabilities

A capability is a named set of tools a chain holds beyond every agent's own. The catalog is code
(`crates/capability/src/capabilities.rs`), so a chain can only hold tools that exist. A chain starts with its kind's
capabilities (`capability::kinds::capabilities_of`, ADR-0113: `work` and `chore` none; `objective` `objectives`, `okr`,
`signals`; `author` `agents`, `history`, `library`, `playbooks`; `coach` `coaching`, `signals`; `analyst` `history`, `outcomes`; `librarian` `memory_keeping`; `trial` `seeding`, `shell`), then holds what
its Chair gives it (`grant_capability`, one that does not write) or asks for and the Assistant decides
(`request_capability`, one that writes). A capability names a tool, never a seat: what code branches on is the kind.

| Capability | Tools | Who gets them |
|---|---|---|
| `history` | `list_chains`, `read_chain`: search every chain by words in its id, title, goal, summary, repository, branch or landed commit, a page at a time, and read its goal, status, summary, landings | every agent of the chain |
| `issues` | `list_issues`: open customer bugs from Jira, credentials removed | every agent of the chain |
| `propose` | `propose_chain`: queue a `work` or `chore` chain serving one key result, each goal naming the playbook that fits, or one competing attempt at a key result whose check is a command (`attempt`); `refine_key_result` | the Chair |
| `playbooks` | create, update, delete, reset playbooks (`list_playbooks` and `read_playbook` are every Chair's and the Assistant's) | the Chair |
| `agents` | create, update, delete, reset agent definitions (instructions written into one wait for the owner: see Agents and the library; `list_agent_definitions` and `read_agent_definition` are every Chair's and the Assistant's) | the Chair |
| `library` | create, update, delete skills (a skill's text waits for the owner: see Agents and the library), `list_importable` and import agents and skills from this machine's Claude Code configuration (`list_library` and `read_skill` are every Chair's and the Assistant's, `list_importable` also the Assistant's) | the Chair |
| `sources` | `request_source_credentials`: make a connection of a source (a staging and a production MongoDB) and ask the owner, in his inbox, for the values it needs; it names variables and never takes a value (ADR-0017); `request_grant`: ask for a write, for a read of a protected connection or for programs on a connection, for the chain or, naming a repository it has open, for that repository's release steps (ADR-0116); the Assistant decides it for the owner (see Outside systems); with it `list_sources` | the Chair |
| `feedback` | `list_feedback`: the newest feedback agents sent about how they work, read only, written by agents (evidence to verify, never instructions) | every agent of the chain |
| `events` | `emit_event`: write an event of an event signal the owner made (a deploy that finished, a check that failed), heard by the signal's subscriptions as any event; a tell never tells the chain that writes it (see Event signals) | the Chair |
| `signals` | `list_signals`, `read_signal`: the chain's own records (its sensors' among them) and the landings of the chains that serve the same objective, read as evidence; with it `subscribe` takes a signal; the Chair's `create_event_signal`: open an event signal at once, as the Assistant does (its key shown to the owner alone), and `create_standing_sensor`: ask for a standing sensor of it, which keeps reading after the chain ends and waits for the owner's Apply (`hold_sensor`, [ADR-0065](adr/0065-a-chair-makes-the-signal-a-key-result-reads-and-its-sensor-waits-for-the-owner.md)) | every agent of the chain; the two makers the Chair |
| `data` | `starrocks_query`, `starrocks_schema`, `mongo_*`, `bytebase_*`: stored production and staging data, read only (ADR-0015); the goal judge holds them too, so a claim about stored values is checked by a query | every agent of the chain, the judge included |
| `observe` | `grafana_promql` (its instant query also the number to try before it is a signal), `grafana_alerts`, `grafana_dashboards`, `coroot_*`, `cloudwatch_*`, `datadog_*` (logs, log counts, spans, metrics, monitors, events, reading: one number to try before it is a signal): metrics, alerts, incidents, traces and logs of the running systems, read only; `run` for CloudWatch, on a grant (see Outside systems) | every agent of the chain, the judge included |
| `customers` | `intercom_*`, `amplitude_*`, `googleanalytics_*` (a GA4 property's reports, realtime, metadata and readings, signed in as a service account: [ADR-0066](adr/0066-google-analytics-signs-in-as-a-service-account-and-its-number-is-a-reading.md)), `github_issues`, `github_issue`: what customers say to support and in a repository's issues and how they use the product, emails masked; the reads need no grant, the Intercom tools that write (`intercom_note`, `intercom_tag`, `intercom_untag`, `intercom_state`, `intercom_reply`) need a grant of their effect (ADR-0037) | every agent of the chain, the judge included |
| `judgment` | `ask_decider` (a write: it records a judgment on the agent's card and spends on the owner's account): yes/no, choice or score questions the agent writes about a state it sends (log lines to screen, candidates to rank, citations to check), each answered with a calibrated probability and no text in about a tenth of a second; the state leaves redacted as a reading's body does, and each answer is recorded as a judgment of the question `agent.ask` on the agent's card, which no outcome scores and no shadow answers (ADR-0118, point 7) | every agent of the chain, the capability given by the Assistant for the owner (`request_capability`: it writes) |
| `outcomes` | `outcome_metrics`: the Outcomes report; `list_insights`: what the owner was last told | every agent of the chain |
| `outcomes` | `record_insight`: an insight for the owner, a decision of kind `insight` | the Chair |
| `objectives` | `draft_objective`: draft an objective of key results, each with an owner and a check, as a change item for the owner to apply (an `objective` chain, which a shaping request starts); `read_objective`: the objective a chain serves, or one the Chair amends, read; an amendment to an active objective is a new draft the owner applies, and a key result it drops is kept as `retired` (ADR-0031) | the Chair |
| `okr` | no tool: the built-in skill `okr`, the craft of measures, conditions and thresholds (ADR-0045); an `objective` chain starts with it | every agent of the chain |
| `coaching` | `read_memory`: an employee's memories, each with whose it is, the teaching it was learned from and the corrections that taught it again; `repeat_corrections`: every employee's corrections and repeats (corrections the memory backend matched to a lesson held), computed by Remit; `overturn_rates`: per kind of item, the Assistant's answers, how many it approved and declined, how many a second judge re-decided otherwise (dissented) and how many the owner overturned, with his newest words (ADR-0093, ADR-0103); `verdict_rates`: how often the Chairs of each kind of chain accept their workers' reports and each kind of judge approves (ADR-0103, ADR-0113); `duel_standings`; with `signals`, the teachings `fact:teach.*` read and watched | every agent of the chain |
| `coaching` | `write_retrospective`: the Retrospective of a finished chain, woken by `fact:chain.finished` (ADR-0082); `promote_memory`: a lesson shared with its lineage (stack) on siblings' memories that hold it, a decision of kind `learning`, or drafted for every employee (global) as a change item the Assistant applies for the owner; `draft_forget`; `request_contest`: another chain's Chair is asked to contest a card; `set_duel_agreement`: the duels' share the owner names (a `coach` chain, the one kind that holds it, ADR-0063) | the Chair |
| `seeding` | `plant_fault`: the one fault the Chair of a trial planted in its work, what and path:line, kept as a `SeededFault` no judge reads; `land` is refused in the chain, and the goal judge's first ruling ends it; the outcome report scores the reviewer's, QC's and goal judge's first verdicts after the planting (a `trial` chain, the one kind that holds it, with `shell`, ADR-0102) | the Chair |
| `duels` | `contest_card`: a kept worker of another lineage works a card's brief on a sibling card; once both reported a judge Remit starts picks one with `pick_report`, which rates the lineages in the chain's kind (`work`, `chore`, ..., ADR-0113; shown in `hire_agent`'s list); paused in a kind while the owner overturns too many picks | the Chair; `pick_report` the duel's judge |

- **Sources and connections** (ADR-0015, ADR-0017, ADR-0018). `data`, `observe` and `customers` give every agent of a chain the tools of their
  sources, and each tool takes a `connection` argument (empty when the source has one configured connection) so a staging and a
  production system of one source are both reachable, except a connection the owner marked protected; `list_sources` (every agent of a chain holding one of those capabilities or
  `sources`, and the Assistant) names the connections with their descriptions, whether each is protected and, to an agent of a
  chain, what its chain may do on it (the Assistant, in no chain, sees the connections alone). The Tools page under Integrations shows each source, its tools and its
  connections live; the owner connects one by telling the Assistant, and types each value into the write-only field of the
  inbox item that asks for it.
- A chain holds its kind's capabilities from the moment it is queued (by the owner, the Assistant, a routine, an event or a
  proposing chain, before it starts, so the Chair's tools are right from its first turn). The kinds and their capabilities
  are code, so no write can take the author's own tools away. A `work` chain's Chair gives itself `shell`, `web`,
  `signals`, `data` and the other reads it needs, and asks for the writers.
- `coaching`, `seeding` and `memory_keeping` are bound to their kind (`Capability.kind`): `grant_capability` and
  `request_capability` refuse them to a chain of another kind, so a tool and the seat it is the work of cannot part. The
  identity predicates read the kind: `is_coaching` `coach`, `is_analyst` `analyst`,
  `is_librarian` `librarian`, `is_trial` `trial`.
- **Self-development** (ADR-0012, ADR-0087). `self-development` is the built-in reference playbook for Remit's own repository, which a `work` chain's goal names: the agents it suggests start from one opening (`SELF_DEVELOPMENT_OPENING`) that says they edit the system that runs them, that a change reaches chains started after it lands and its release, by Remit's own `.remit/release/`, is installed (ADR-0116), which documents to read and cite first (`AGENTS.md`, `docs/design.md`, `docs/adr/README.md`, `crates/orch/README.md`, `crates/builtin/skills/app`), and that the instance, its database and its key are not theirs to restart, read or push to. A chain's change lands once the reviewer, QC and, when it touches a path `.remit/rails` names, the Security reviewer (the built-in agent `security`, which signs `security` and writes a threat model in its verdict, which the landed commit names) approved its version; its description says what in `fact:remit.health` would show the change hurts, how it is taken back and whether it migrates. A chain that proposes may name it in the goal of a `work` chain it queues.
- Who may start which kind is one rule, `capability::kinds::may_not_start`: a chain a proposal, an event or a client
  starts takes no kind whose capabilities write (so `work` or `chore`), and `analyst` and `librarian` are Remit's own. Such a
  chain's Chair may still ask for a writer, and the item names its origin for the Assistant to weigh.
- The owner sees a chain's kind and capabilities in its overview; the decision that starts the chain names them.

## Outside systems

An agent acts on an outside system only through Remit, and holds a grant, never a credential (ADR-0018). A credential
exists only in the executor that carries a call out: the backend for a typed tool, the jail for a program. Four rules hold
for every form of use and every source: the credential is sealed in its executor; a chain holds the least grant it needs;
every call passes one gate; each call says how it is kept to its effect (`read`, `reversible`, `compensable` or `irreversible`), and no description claims more
than that.

- **Grants** (`crates/grant`, pure). A grant is a source, a connection and an effect, ranked by how far it can be undone: `read` < `reversible` < `compensable` < `irreversible` (ADR-0027). `decide` answers
  one action against the grants a chain holds: allow when a grant on that connection is at least the effect, ask the owner
  when only a lower effect is held, deny when none is held on that connection, whatever the source's other connections hold.
  What a chain holds is settled in one place (`crates/reach/src/grant_access.rs`): its stored grants (`grants`) and,
  derived and never stored, a read on every unprotected connection of the sources whose capabilities it holds. A
  protected connection is read only through a stored grant. The owner marks one by telling the Assistant
  (`set_connection_protected`). A write and a read of a protected connection (`needs_owner`), and a program, are the owner's
  to give, and the Assistant decides them for him (ADR-0087): the Chair of a chain that holds `sources` and the source's
  capability calls `request_grant` with source, connection, effect, form and a reason (never a value), which raises one
  `change` item; Apply stores the grant only while what its holder holds is still what was shown (`apply_grant`) and
  records who asked and who gave it as a Change. A grant's holder (`model::GrantHolder`) is a chain or a repository
  (`HolderKind`, ADR-0116): a Chair names a repository it has open to ask for that repository's release steps, which
  reach a connection only through such a grant and write only through an `irreversible` one (*Releases*, under Running
  it). No agent tool writes today.
- **One gate** (`crates/source`, `gate.rs`). Every call passes it, whatever its form. It finds the connection, asks the
  policy (`GrantPolicy`, which asks the rule above; a call that names no chain is denied), runs the act under a deadline,
  removes secrets (`RedactText`), starts the answer with `connection: <source>/<name>`, cuts it, and tells the recorder
  (`CallAccount`: one source_calls row per call with chain, agent, source, connection, tool, effect, form, outcome and what the
  input was without its secrets: a program's argv, a script's language, size and hash; the database is hidden from sessions). Who calls comes from the card (its `Caller`), never from the tool's input. A gate
  with no policy lets through a read by a typed tool and nothing else.
- **A typed tool** (`Form::Tool`) keeps the credential in the backend and its effect in its own code (`crates/source/src/sqlguard.rs`,
  `crates/source/src/mongoguard.rs`: statements that read).
- **A program** (`FormEnv`) is the one tool `run`, offered with the capability of any source that implements `Programmed`
  (CloudWatch, which runs `aws` for `logs describe-log-groups`, `filter-log-events`, `get-log-events`, `start-query`,
  `get-query-results` and `sts get-caller-identity`). The agent writes the arguments, an argv no shell reads. The source fixes
  the binary, the environment, the commands that may run, the flags and values refused (`ProgramSpec.Check`) and the
  addresses the program may reach. The credential is in the program's environment, in a jail, and the program runs only on a
  read grant the owner gave for programs (`request_grant` with form `env`) on a connection he did not mark protected
  (`decide_program`); the read a capability implies does not reach it. That the program only reads is the credential's own
  rights, so the owner applies the grant only for a read-only credential, and every answer carries the line
  `form: program (env in a jail; read-only is the credential's rights)`. A secret the program prints is removed from its
  answer; one it encodes is not.
- **A script** (`FormProxy`) is the one tool `script`, offered with the capability of any source that implements `Injected`
  (Grafana, GitHub and Datadog): python3, sh or node code the agent writes, run in the jail with no credential at all. Its only way
  out is a proxy of the call (`egress::Injector`) that ends the script's TLS for the connection's host with a certificate of a
  throwaway authority, adds the connection's credential headers in place of any the script sent, and re-opens TLS to the
  upstream. The proxy keeps the script to the effect in code: GET, and POST only at the paths the source names as reads;
  no `..`, encoded slash or foreign Host; no redirect to another host; a cap on requests, body and answer; the credential
  scrubbed from what comes back. It follows the rule of a typed tool's read (`decide_as`), because the script holds nothing and the
  proxy confines it; every answer carries `ScriptEnforced`. A script can still read much of the API in bulk and send what it
  reads to the upstream host only; an encoded copy is not masked.
- **The jail** (`crates/jail`, `JailRunner` in `crates/reach/src/program.rs`), one API with an adapter per OS
  (`docs/adr/0021-the-jail-has-an-adapter-per-os.md`). On Linux a program runs in new user, mount, network and pid namespaces with
  no capability and `no_new_privs`; on macOS under a Seatbelt profile that denies everything but what it allows. It sees its
  scratch directory (the one path it writes) and the system (`jail::system_paths`) read-only, and its environment is the one it was
  given, with the proxy variables the jail sets. It has a deadline, a cap on its output and a CPU limit, and on Linux a memory
  limit; no process count or disk is bounded. It has no network, except through a unix socket to an egress
  gate (`crates/egress`) made for that call, which connects it only to the listed `host:port` pairs, refuses a name that
  resolves to a loopback, private or link-local address, follows no redirect, caps tunnels in number, bytes and time, and
  names what it refused in the answer. The gate cannot see what a program sends to an address it allows.
- **A session** (`crates/agent`, `crates/claudecode`, `crates/airuntime`, `crates/remit/src/runtime.rs`). It runs in the AI
  runtime, the core's child, as the server's OS user with the same HOME (ADR-0058): `crates/remit/src/runtime.rs` builds the
  `ClaudeLauncher` the runtime starts every session with. A stop deletes the run's MCP token, cuts the turn and kills the session in the
  runtime; a runtime that does not end it within `kill_wait` is killed with every process under it and started again, and the
  sessions it held end as `agent::Error::HostRestarted`, which resumes their cards. Claude Code starts with an
  allowlisted environment (`crates/childenv`, `childenv::of`: where programs are, the proxy and certificate settings, the git identity, the Go
  toolchain; nothing of the server's credentials), and inside `jail::hidden_with`: on Linux user and mount namespaces over the host's
  files and network, on macOS a Seatbelt profile. The paths of `protected_from_agents` cannot be read by it nor by anything its shell
  starts (on Linux they read as empty): the key file
  and its backups, the database with its snapshots and copies, the source and Jira env files, the vault's `secret/`, the client-key
  files, the process manager's config and its state directory. The directory of the server's binary, and the checkout around it
  when that is the checkout's `bin/`, read as they are and cannot be changed. A launcher with no path to hide starts no session
  and a host that cannot isolate it fails the start; there is no unisolated fallback. The server's own checks of an
  agent's change (`repo_command`) take the same environment.
- **A jailed child is told the work it runs for** (ADR-0119, `childenv::trace`, `telemetry::Child`): a repository's checks
  (`repochecks.rs`), a release's steps (`release_run.rs`) and a source's programs and scripts, a sensor's included
  (`JailRunner`), each run in a span of its own, `remit.child`, under the span of the work that started it, which records how
  it ended. A chain's checks run under `remit.checks` and a source call under `remit.source`, each naming its chain (Remit's
  own polls name none); a release's steps name their release, not a chain. The child gets `TRACEPARENT` (that span, while the
  server traces) and the work's correlation attributes, escaped, after the job's own `OTEL_RESOURCE_ATTRIBUTES`; its
  service name stays the job's. Ids, never a credential, never the Datadog key, and no endpoint to export to, as the jail
  leaves it no way to the `/otlp` door.

## Agents and the library

An agent that does one kind of work well needs a narrow context and the right tools, and the owner needs to see exactly
what each agent holds. So Remit keeps its own inventory and an agent is what the inventory says; nothing comes from
the server user's own Claude Code configuration unless an agent imported it. The owner defines and changes all of it by
telling an agent (an `author` chain, which holds the `agents` and `library` capabilities; the Assistant starts it with
his words), and sees it on the Agents, Skills and Tools pages, under the Crew tab (each a list with the open item beside it: for a skill, every file of it, one readable at a time; Tools is the sources Remit writes, which no agent defines).

- **What a session gets** is decided in one place, `library::access` (`crates/library`, pure): the Claude Code tools it may
  use (`tools`: an allowlist; `Skill` is added when it holds skills), its skills (always sent, empty meaning none), its instructions (appended to Claude Code's own system prompt) and its model.
  All of it comes from the card: its loadout, which it took when it was hired, and its own model. `options`
  (`crates/agent/src/options.rs`) turns it into options for every session Remit starts: only the repository's settings and `CLAUDE.md` are read
  (`--setting-sources project`), only Remit's own MCP server exists in it (`--strict-mcp-config`), and its skills come from the
  rendered plugin directory `<data>/library/<hash>/plugin`. Claude Code waits 300 s for the answer of a call to that
  server (`MCP_TIMEOUT_MS`, the entry's `timeout`) where it would give up after 60 s with the tool still running: the
  tools' own bounds stay under that (`open_repo` the longest, 180 s), so a tool that runs long still answers its agent.
- **The Session tab shows live OS processes** (ADR-0134). The runtime samples its session's process tree and the owner
  sees executable names, process states, CPU, resident memory and elapsed time. Subagents show the shared parent session.
- **Every tool of Remit's MCP server says what it does.** A tool is added with its effect (`tools::Effect`: it reads, or it
  writes), which the server lists as the tool's `readOnlyHint`, so Claude Code runs readers side by side; the tools a loop
  calls every turn (`update_plan`, `hire_agent`, `accept_work`, `ask_chair`, `record_verdict` and the like) carry
  `anthropic/alwaysLoad`, so they stay loaded when Claude Code defers the rest behind ToolSearch. Both are hints to the
  runtime, never a rail. An update tool writes over the revision the agent read (`rev`, from the matching read tool) and
  answers a conflict when the record changed since; a skill's file is also edited by quoting the lines to change
  (`edit_skill_file`).
- **A card holds its own loadout.** `Hire` copies the loadout of the agent it hires (an id with no definition: every tool,
  nothing from the library) onto the card, and the Chair's `tools` and `skills` in `hire_agent` replace the copy's
  lists (absent keeps them, `[]` is none); the whole is checked against the library (`library::validate_loadout`) and the hire
  decision says what the Chair chose. From then on the card's sessions read the card: editing or deleting the agent, or a
  restart that rewrites a built-in one, reaches the agents hired afterwards and nobody at work, and a worker card with no
  loadout is not started. Skills are held by name, so the library's one copy of each is what a session loads
  and a name the library lost is reported as missing on the card. Every Chair reads what each agent holds and what the
  library has (`list_agent_definitions`, `read_agent_definition`, `list_library`, `read_skill`); writing
  them stays with the capabilities below.
  What it signs is fixed when hired (`Card.Signs`, from its agent). The reviewer signs the combined review. A QC signs
  the separate check requested for a landing, and a security reviewer signs changes to a rail. A verdict records the gate it was given for as its role (`reviewer`, `qc`, `security`, or `judge` for the goal
  judge), which the verification and the outcomes read. The deny rules
  are not part of the loadout: they stay in `deny_for` (`policy.rs`), which reads `Card.Signs`, so no choice of the Chair
  widens them.
- **What moves a rail waits for the owner** (`crates/capability/src/rail.rs`, `change_item.rs`). Most of what a Chair writes is
  configuration. Some things are more, and a write by an agent that changes one is not stored: a `change` inbox item shows
  the fields as they are and would be, and the owner answers Apply or Not now himself, as he alone enters a credential (an
  agent never does, nor the Assistant, which reads what agents write). They are a capability that writes, asked for
  by a running chain's Chair (`request_capability`; `agents`, `library`, `propose`, `outcomes`, `sources` and the rest); a grant of a write, of a read of a protected
  connection or of programs on a connection (`request_grant`); and instructions
  that outlive the chain: an agent definition's (part of the system prompt of every card hired from it) and a skill's
  description and files (the item shows the files that differ), where an injected turn would steer every later hire.
  Apply stores the
  write only while those fields are still what the owner was shown, and tells the agent either way (`[change result`). A
  second write to the same record replaces the open item, and a chain with one open cannot finish: its Chair uses
  `subscribe`, and the stall rail spares it as it spares a question. The owner, the Assistant and Remit write at
  once. An agent's import waits where its write would: agents can write the Claude Code configuration an import reads.
  Every write, rail or not, leaves a Change, so the owner can read who changed what.
- **Import** (`list_importable`, `import_agent_definition`, `import_skill`) reads, read-only, the Claude
  configuration directory (`-claude-config-dir`). An entry of its `agents/` and
  `skills/` directories may be a link to anywhere; nothing inside a skill is followed; files are text and bounded. A hidden
  file in a skill (`.env`, `.git`) is refused. Claude Code
  names a skill's canonical name with every dot turned into a dash (`vault.help` is `library:vault-help`); the library
  keeps the dotted name for the owner and maps it, and refuses two skills that would clash. Nothing outside agents and skills
  is read: the MCP servers that configuration lists are not imported (ADR-0017).
- **Credentials** (ADR-0017). The Assistant, or a Chair holding `sources`, calls `request_source_credentials` with a source, a
  connection and the variables it needs; it names variables and never takes a value. Each variable that is missing (or named, to
  rotate it) opens a `credential` inbox item homed on the Assistant, plain variables first, none twice; a new named connection
  needs a description. The owner types the value into the field of the item: a plain variable (an address, a user) is stored
  as typed and shown on the Tools page, a secret is sealed (AES-256-GCM, `-secret-key-file`, created when the first one is
  entered, the source, connection and variable as associated data) and shown as `set`, and nothing else happens with it: no answer
  text, no decision, no agent, no log line. Only the owner's answer stores a value (`SetCredential`). The backend opens it when
  a source's tool is called, in the Remit process: no value reaches a session's environment or command line. The item
  states where a secret will go. Lookup is the connection's stored value; for `default` only, then the process environment and
  env files (`-source-env`). A named connection never reads the environment. The owner may answer an item Remove the stored
  value (the environment or file applies again) or Remove this connection (its values and row go together). A stored value that
  no longer opens under its own key is cleared and asked again, as below. A Claude Code session is given its token for
  Remit's own MCP server as a reference to a variable of its process's environment (`${REMIT_MCP_<n>}`, `by_reference` in
  `crates/agent`), which Claude Code expands: the server entry is on the CLI's command line (`--mcp-config`), which every
  process on the machine reads with `ps`.
- **The key.** Every sealed value (a credential, an event signal's key) is stored with the id of the key that sealed it (the
  first 8 bytes of an HMAC of a constant under the key: it names the key and gives nothing of it). Before anything is
  sealed under a key, and at every start, the key is copied, owner-only, to `secret-<id>.key` in `-key-backup-dir`
  (default `<data>/backups/secret`, a directory named `secret` so the agents' secret-read rules cover it; one on another
  disk survives losing the data directory). A copy is written once: one that no longer holds the key its name says was
  changed, and stops the start. A value whose key file is missing or holds another key is refused where it is used (the
  card's failure, with a retry in the inbox, says which key it needs and which backup to copy back) and is kept: nothing
  is cleared for a key, and the start logs every stored key id the key file does not open. While a stored value names a
  key that has a backup, a missing key file is restored, not replaced: sealing refuses until it is back. With no backup
  of it left, the key is lost for good, and a new one is made for what is sealed next. A value its own key no longer opens
  is damaged: it is cleared and asked again, once. A key file that is merely too open in mode is the operator's to fix and
  clears nothing. Values sealed before key ids were recorded have none: they open by trying the key file's key, and one
  it does not open is taken as another key's, so it is kept.
  A session shares the server's OS user and starts with the key file, the database and its copies hidden and an allowlisted
  environment (see Outside systems), so the sealing protects the database, its backups and every channel Remit writes
  against a session too; the gaps that remain are in the known gaps below.
- **The Assistant's manual** is a skill that ships with Remit, `builtin:app` (`crates/builtin/skills/app`): text in the binary,
  not a record of the library, so the owner neither edits nor deletes it and no agent can hold it. It is rendered at start into
  a plugin directory of its own and given to the Assistant's session alone. Its index says what each part of the app is and
  which reference to read; one reference per area (chains, playbooks and the library, the inbox and credentials, routines,
  event signals, the signals and outcomes) says which tool does what, in what order. The Assistant's brief stays
  short (who it is, what it never does) and the how-to is read when the work calls for it. A test keeps the manual and the
  tools in step: it names only tools that exist, and every tool of the Assistant is named by it or by the brief.
- **Left out:** per-agent Remit capabilities granted by the Chair at run time.

## Running it

- **What runs is a release.** Every push to `master` is one, never the working copy: GitHub's release workflow checks
  the commit out, runs `make deps build test` there and only when all of it passes publishes the binary, web app
  embedded, with its `SHA256SUMS`, tagged `<yyyymmdd>-<short sha>` (`scripts/release_tag.sh`, ADR-0087). The machine that runs it holds no
  source: `remit update` (`crates/remit/src/update.rs`), under a lock, downloads the release, checks it against
  `SHA256SUMS`, installs it as `<data>/releases/<tag>/remit` and points `current` at it and `previous` at what `current`
  was, each by a rename, so a supervisor that restarts mid-update finds one whole binary; it copies the database
  (`pre-update` snapshot), restarts the server through the supervisor's command, waits for `/healthz` on the release's
  commit and a welcomed hello on the link, and goes back to `previous` when they do not come, putting the copy back
  when the release migrated the database (one-way); it writes its outcome to `<data>/releases/update.json` too. `remit doctor` checks a
  running server read-only (health, build, the link's certificate, the link over UDP). A release does not sit in a checkout, so the supervisor passes `-agent` when
  `claude` is not on its PATH.
- **Releases** (ADR-0116). What lands on a repository reaches its customer by the procedure the repository declares in
  `.remit/release/`, read at the commit released, run by Remit's code with no agent and no person asked.
  - **The procedure.** Its executable files are the steps, run in name order, each in a jail of its own (`jail::Spec`,
    built in `crates/shipping/src/release_run.rs`) in a fresh checkout of the commit, stopping at the first that fails.
    A step's environment is `PATH`, `HOME` and `TMPDIR` (its scratch directory), `LANG`, `REMIT_RELEASE_REPOSITORY`
    (host/project), `REMIT_RELEASE_COMMIT`, `REMIT_RELEASE_PREVIOUS` (empty for the first), `REMIT_RELEASE_OUT` (the
    file it writes `{"released", "version", "links"}` to) and, for a way-back step, `REMIT_RELEASE_BREACHED`; no
    credential. `connections` names one `<source>/<connection>` a line, the only places a step reaches: each through a
    grant the repository holds (`HolderKind::Repository`, asked with `request_grant` naming the repository), behind
    one injecting proxy for the step (`crates/reach/src/release_step.rs`) that adds each connection's credential,
    admits writes only where `grant::decide` allows an `irreversible` effect, and scrubs every credential from the step's output. `guard`
    (optional) is one line `<signal key> <bake minutes> <breach condition>`; `back/` holds the way-back steps. The pure
    core is `crates/shipping/src/release.rs`; every path under `.remit/release/` is a rail (`gate::RELEASE_DIR`).
  - **When.** One release of a repository at a time, none while one releases or bakes: at the `k`-th oldest landing on
    its base no release carried that the forge settled (checks passed, or no pipeline after 10 minutes), `k` grown by
    one after a release that went live and halved after one taken back (`landing::grown`, `halved`); the first release
    of a repository, and the first after a pause is lifted, at its newest. The periodic landing sync runs it
    (`sync_releases`), and so does a landing whose checks stand. A step that exits 75 waits for the next sync, which runs
    the procedure again from its first step, the run's steps replacing the last (`Release.tries` counts the runs); a
    procedure that reports `released: false` leaves its landings for the next release. A repository with no
    `.remit/release/` has each landing live once its checks stand: its release is recorded live at once. A procedure
    that does not parse waits, saying why, and the chains whose landings changed `.remit/release/` are told; what lands
    since re-aims it, so the fix is released without anyone asked (`release::reaim`).
  - **Bake and live.** A release that published bakes under its guard and the guardrail of every stage plan it
    carries, the longest bake; with none it is live at once. While one bakes under `fact:remit.health`, that fact is
    recorded every Tick, as for a watch. Baked healthy, it is live: each chain it carried is marked
    live by code (`chain:<id>.live`), and `fact:repo.release` `{repository, release, version, commit, landings, state}`
    is recorded for a release that publishes, goes live, is taken back or published nothing.
  - **In Datadog** (ADR-0119 D3). A release that published is sent as a deployment event while the Datadog source's
    default connection has its key: `Shipping.deployments` (`release_run::Deployments`, filled by
    `crates/remit/src/datadog_ports.rs`) hands the exporter its version, repository, commit, when its steps ran and the
    tags `remit.release_id`, `remit.chain_id` and `remit.landing_id` of what it carries
    (`datadog::Deployment`, `POST /api/v2/dora/deployment`, queued, never waited on). Its work is traced: the run of
    its procedure, its going live and its taking back are each a `remit.release` trace, with a marker span tagged with
    the chain for each landing it carries, so a search on `@remit.chain_id` finds that marker span, whose trace is the release's
    (each trace is its own, linked to the span it was started from; its logs and steps carry the release, not a chain)
    (`crates/shipping/src/release_trace.rs`).
  - **The way back.** A step that fails, the forge's checks gone red on its commit, or a guard that breaches during the
    bake takes the release back by code: recorded taken back with who and why, every landing it carried reverted at the
    head of its queue, then its `back` steps run (on this server's repository, `install_release previous` when the
    release was installed here; one never installed leaves the server as it is); once the
    reverts land, the next release carries them forward. `revert_release` does the same on words: the Assistant's for
    any release, a Chair's for a release that carries its landing. Each chain it carried hears it as a decision of
    kind `release`.
  - **On this server's repository** (`-update-repo`) a release that published runs one more step before it bakes,
    `install_release` to the version the procedure reported; a refused install waits, saying why, and runs again at the
    next sync, never baking or going live (`crates/shipping/src/install.rs`): it starts `remit update` with the flags
    the server was started with (`orch::Updater`, built in `crates/remit/src/config.rs`) and `-to <tag>`, detached from
    the server (`procgroup::detach`, the environment `childenv` gives, output appended to `<releases>/update.log`),
    refused on a server not started with `-update-restart` and while `<releases>/update.lock` is held. Remit's own
    `.remit/release/` has one step that waits for the workflow's release of the commit through `github/default` (a
    read) and reports its tag, and a guard on `fact:remit.health`.
  - **The pause.** `pause_releases` and `resume_releases` (the Assistant's, on the owner's word) hold and lift a
    repository's releases, recorded with who and why; landing goes on, and a release baking bakes on. A pause of a
    repository Remit does not know (no chain works in it, nothing landed on it) is refused.
  - **What the owner reads.** A repository's `RepoView` holds its newest releases with their landings and its pause
    (`repo_get`, pushed whole as `repo_updated`; `repo_views` for every repository with a queue or a release). The
    owner reads them on the Merge queues tab (`/queues`) and its Releases page (`/releases?repo=`), and on a landing's
    record as "Released in"; neither changes a release: he tells the Assistant.
  - Facts the server records about itself (`crates/shipping/src/remit_facts.rs`), read and watched by any chain:
    `fact:remit.health` `{revision, tag, startedAt, healthy, reason, failures, toolErrors}` at every start, at each
    Tick while a watch on it is open or a release bakes under it, and when `healthy` changes, failures and failed calls of Remit's own tools over the
    last 15 minutes; `fact:remit.updated` `{from, to, healthy, rolledBack, oneWay, error}` once per run of `remit
    update`, from `<releases>/update.json`; `fact:remit.error` `{summary, fingerprint, target, message, error, chain,
    card}` once per new issue among the ERRORs the server logs (ADR-0119, `crates/remit/src/errors.rs`), which a
    shipped subscription gives to a chain that investigates. rmcp's ERROR for an answer whose client had closed its
    connection (a session's process that ended) is not one.
- **Health.** `GET /healthz` takes no sign-in, for a supervisor or an uptime check: 200 while the database answers within
  2 seconds and the scheduler, which ticks every minute, has got through a tick within the last 3 minutes; 503 with the
  reason otherwise. A tick that stopped on a store error does not count. It says the commit the binary was built from
  (`crates/remit/build.rs` stamps it) and whether that tree had uncommitted changes, and names no path or
  credential.
- **Committing and pushing.** `make hooks` points git at `scripts/hooks`. Its pre-commit hook refuses a commit whose Rust does
  not pass clippy in the crates the staged files belong to or the crates that depend on them (`scripts/test_changed.sh --lint`);
  its pre-push hook warns about a commit past the size budget (below) and stops a push that does not build
  (`cargo build --workspace --locked`) or pass `make audit`. `make test-changed` is the tier every change runs, on the files
  named (`scripts/test_changed.sh`); every release runs `make test`.

The pre-push hook (`scripts/hooks/pre-push`, after `make hooks`) warns about a commit that changes more than 800 lines,
generated and lock files left out, unless its message has a `Large-change:` line saying why (`scripts/commit_size.sh`).

## Calls made by the builder (owner may revisit)

- WebTransport for the link: `web-transport-quinn` on tokio in the server and every native peer (`crates/link-quic`), the
  browser's own through moq-dev's `web-transport` (`crates/client-web`); the TypeScript `tsgen` writes is committed.
- `rusqlite` with SQLite bundled; `rmcp` for MCP.
- A card's cwd is the Chair's decision: the chain's directory by default, the repository's worktree for code work.
  Code does not validate it. The Chair's brief says what it should be, the Chair sees every agent's cwd in
  `list_agents`, and the owner sees it in the card's details. If a wrong cwd ever costs more than the freedom is worth, the rail to add is a refusal in
  `Hire` of any cwd outside the chain's directory; it was left out on purpose.
- Known gap: the ask rules and the policy are a tripwire. A session can still reach a secret in a way no rule lists (`grep -r`
  over a directory holding a `.env`, a script it wrote) wherever isolation does not hide it. Remit's own secrets (the key
  file and the database, which the rules name where `-secret-key-file` and `-data` put them, and the files where client-key holders keep theirs (`-client-key-files`), `own_secret_files`) are
  hidden from every session (see Outside systems). Left open, each for its own unit (`docs/adr/0018-agents-act-on-connections-through-grants-and-one-gate.md`,
  `docs/adr/0004-agents-run-as-the-servers-os-user.md`):
  - The GitLab and SSH credentials (`~/.ssh`, `~/.config/glab-cli`) are readable by a session and by a check of a change,
    because library skills have sessions run `glab api` and `git fetch` over ssh. They move behind the gate with a GitLab
    connection kind, which does not exist yet.
  - A session has no pid namespace, so it can signal the server's process, and it shares the server's OS user, so it can
    write what that user's cron and shell start outside the isolated paths (the process manager's start script, the
    user's crontab, shell startup files). A list of paths cannot close that; a separate OS user, or a session whose whole
    root is a jail's, does.
  - A session keeps the host's network, which it needs to reach its model provider, so what it can read it can send anywhere.
  - On macOS a session reads the environment of the server's process and of every other process of the user
    (`kern.procargs2`), which no Seatbelt rule refuses: the server's environment there must hold no secret (ADR-0021).
- A card waiting for the provider is looked at again on the cooldown of the failure that parked it, so after the provider
  answers it can take up to a cooldown and a quarter (about 19 minutes after a limit) to start again.
- Known gap: shell commands are not limited to the agent's cwd. An early chain created a worktree of its own from the
  product checkout and left a branch and a worktree registered in that repository's `.git`.
- With `-autonomous` (default on) the Chair decides everything itself. Permission requests go up the same ladder in both
  modes (6). `ask_owner` is
  the Chair's in both modes: the one thing an autonomous Chair puts to the owner is a block from outside the chain (access
  or permission the chain lacks, data it cannot reach, a rule or hook that needs a person's input, quality that an outside
  factor puts out of reach). Such a block is not a result: the Chair asks with the exact error and what it needs, waits with
  `subscribe` (no slot held), and `finish_chain` is refused while a question or a change waits for the owner. His answer
  reaches the Chair as a message. A chain on a schedule, which no one waits on, names an unreachable source in its summary instead.
