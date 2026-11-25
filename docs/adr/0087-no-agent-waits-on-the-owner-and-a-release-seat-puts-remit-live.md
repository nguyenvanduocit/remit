# 0087. No agent waits on the owner, and a Release seat puts Remit live

Status: accepted, 2026-11-12; its merging by the Release seat superseded by ADR-0114 (the queue lands, the seat installs and takes back), and the Release seat itself by ADR-0116, built 2026-11-19 (a repository's own procedure releases what lands, in code, and the seat goes; `install_release` stays, as the last step of a release of this server's repository), its admission paced to the merging agents superseded by ADR-0115. Supersedes every rule that makes work wait for the owner except the true-human acts below:
ADR-0012's "nothing merges but the owner" and the owner-only steps of ADR-0001, 0013, 0018, 0024, 0029, 0031, 0032, 0033,
0034, 0035, 0045, 0047, 0050, 0061, 0065, 0067, 0077, 0078, 0080 (its next steps on a merge) and 0083; extends ADR-0050 (`merge_mr` merges another chain's MR
for a seat that holds `release`). Keeps ADR-0017, 0040 and 0074 on secrets and sign-in. Amended by ADR-0101 (an Apply on
the top rung of the effect ladder takes effect after an overturn window). Source: the owner's words of
2026-11-12 (translated): "add an agent or something that owns release; I don't want to keep waiting for me. Our principle is
Agent-centric, not Human bottleneck. Not only this one, but everything else too: if agents can't do it and must wait for a
human in cases like this, nothing ever gets finished."

## Context

Chain `ch_300e2c91b505` (Self-development, the dev instance) finished `done` with a draft PR its reviewer and QC approved,
and stopped there: every agent of that playbook is told "Never merge, restart, rebuild or redeploy the running instance"
(`SELF_DEVELOPMENT_OPENING`, `crates/orch/src/playbooks.rs:28`), and the playbook holds `history`, `shell` and `web` only
(`playbooks.rs:157`). Nobody released it. Putting a change of Remit live is four steps a person does: push to
`origin/master`, `make release` (`scripts/release.sh`), about 18 minutes of the release workflow, then `remit update` on the
machine (AGENTS.md, *Go live*). The built-in `release` playbook ("Release train", `playbooks.rs:183-190`) only prepares:
"never tag, merge, deploy or announce. The owner releases."

The release is one of many waits. An inventory of the working copy on 2026-11-12 found these:

| # | What waits | Where | Who acts today |
|---|---|---|---|
| 1–4 | Cut a release, install it, restore the database after a rollback, merge a self-development MR | `scripts/release.sh:17-21`, `crates/remit/src/update.rs:1-17`, `docs/deploy.md:147-177`, `playbooks.rs:28,157` | nobody |
| 5 | Merge without `ship` | `policy.rs:53-70`, `capabilities.rs:467-469`, `prompts.rs:48-51`, `goal.rs:405-413`; the Assistant's brief forbids merging (`assistant.rs:373`) | the Assistant, through a `ship` request |
| 6 | A writing capability a chain asks for mid-chain | `capability_request.rs:1-8,143-150`, `change_item.rs:329` | the Assistant, when he asks it to |
| 7 | A writing capability for a chain a proposal, an event or a client key started | `capabilities.rs:231-235`, `capability_request.rs:183`, `lifecycle.rs:223-229` | nobody, not even the owner |
| 9 | Put live | `live.rs:8-10`, `assistant_tools.rs:543-548`, `chain_jobs.rs:1-5` | the Assistant on his word, or a deploy event |
| 10 | New work held while `-max-waiting-mrs` MRs wait for his review | `admit.rs:24-26,176-201`, `flags.rs:171-176` | him |
| 11, 18 | `ask_owner`, including amending a key result's check | `tools.rs:1152-1153`, `goal.rs:584-591`, `check_rail.rs:84-90,137-145` | the Assistant, which may `leave_to_owner` |
| 12–15 | Change items, grants above read, standing sensors, extensions and outside MCP servers | `change_item.rs:32-166`, `grants.rs:400-472`, `sensor_settings.rs:308`, `extensions.rs:690-696`, `mcp_servers.rs:443-595` | the Assistant, when it is asked |
| 16 | An extension on or off | `extension_state.rs:248-264` | him only |
| 17, 20, 21 | Objectives, hiring, forgetting a memory: code lets the Assistant apply, 14 texts tell agents "he applies it himself" | `objective_tools.rs:119,133,315`, `playbooks.rs:214,226`, `memory_tools.rs:98`, `employee_change.rs:526-530,657`, the `app` and `okr` skills | him, by the texts |
| 19, 22–25 | Exceptions, a permission request a Chair passes up, a gate that asked for changes three times, an error item, a held subscription | `objective_tools.rs:470-504`, `check_review.rs:291-352`, `permission.rs:628-660`, `gate.rs:458-530`, `inbox.rs:190-316`, `record.rs:220-263` | him; none is routed to the Assistant |
| 29 | Stop, cancel or restart a card | `request.rs:290-298`, `assistant.rs:373` | a Chair for its own agents, else him |
| 31 | Product decisions, naming a repository, running a backfill | `playbooks.rs:111,143-144,175,250` | him |

The Assistant may already do every owner act that is not secret (`for_owner`, `crates/orch/src/owner_acts.rs:53-58`); what it
lacks is being asked, a few tools, and briefs that let it. Four acts are secret by code: typing a credential
(`ACT_CREDENTIAL`, `owner_acts.rs:31-36`), typing a sign-in code (`ACT_SIGN_IN_CODE`, ADR-0040), seeing a key's or a code's
value (`ACT_SEE_KEY`, `owner_acts.rs:45-49`), and one setting on the forge outside Remit (`docs/design.md:416-428`).

For the release in particular:

- What the owner's merge protected: a judge apart from the builder (built: `missing_approvals`, `crates/orch/src/gate.rs:94`);
  a rail change not landing unseen (only his eye, ADR-0012 *Threats*); a bad release taken back (partly built: `remit update`
  goes back to `previous` when health does not come, `update.rs:1-20`; nothing watches a release that starts healthy and then
  hurts); the instance not restarted under the chain that changed it.
- No tool installs a release, and Remit's pipeline does not deploy on merge, which ADR-0050 assumes.
- No signal reads Remit's own health or release. `merge_mr`'s guardrail must be a signal the chain may watch with a reading
  (`crates/orch/src/ship.rs:40-46`); a chain watches only its own records, and the teachings and finished chains with
  `coaching` (`readable_by`, `crates/orch/src/watch.rs:319`).
- An older binary refuses a database a newer one migrated (`crates/store/src/migrate.rs:12-22`), so the way back after a
  release that migrated ends with a server that does not start, today, with no one watching.

## Options

**Who decides what waits for the owner today.**

1. **The owner, faster** (notifications, a shorter inbox). He stays the constraint of every chain.
2. **The Assistant decides every non-secret wait as it is raised (chosen).** It holds his say already (`for_owner`); each item
   is routed to it, decided and recorded with who and why; he reads after and overturns.
3. **Each Chair decides its own chain's waits.** A Chair would grant itself capabilities and lift its own rails: the judge
   and the party would be one.

**Who merges Remit's own changes.**

1. **Self-development holds `ship`; the builder merges and installs.** Several builders would install onto one running
   version, so a breach names no change; the builder restarts the instance it runs on; the pod, where no builder runs, still
   needs someone to install.
2. **A Release seat on each instance (chosen).** A standing chain that takes approved MRs on the instance's own repository
   one at a time, merges, installs, bakes and takes back. What an instance runs is one concept, with one owner.
3. **Merge on approval, in code.** Nothing watches the result, nothing judges a rail change, nothing takes it back.

**Who cuts and installs.**

1. **Tools `cut_release` and `install_release`.** The tag rule would get a second maker beside the workflow, for a step no
   judgement goes into.
2. **The workflow publishes every commit of `master`; an `install_release` door installs (chosen).** Cutting needs no
   judgement, so it is code; installing needs a moment and a watch, so it is an agent's. Each instance pulls its own release,
   as a GitOps operator reconciles its own cluster.
3. **The seat runs `make release` and `remit update` in its shell.** No door, no record, a restart from any command.
4. **Each server installs the newest release by itself** (docs/remit-design.md K11, for customers). Nothing ties a breach to
   the change or reverts `master`, so the next update installs the bad change again.

**What reads a release's health.**

1. **A script sensor the seat makes over `/healthz`.** Needs a source that runs scripts, reads only up or down.
2. **Facts the server records about itself (chosen):** `fact:remit.health`, `fact:remit.released`, `fact:remit.updated`.
3. **ADR-0085's telemetry as the guardrail.** `telemetry.db` holds no signal; its counts feed option 2's reading instead.

**What replaces the owner's eye on a rail change.**

1. **He reviews rail changes only.** A wait on a person stays on the changes that matter most.
2. **A third verdict, `security`, on a change touching a path the repository declares a rail (chosen).**
3. **Agents may not change a rail.** Remit could not improve its own rails.

Models applied. **Bottlenecks**: the owner is the one constraint every chain passes; work added anywhere else queues at him.
**Inversion**: what would guarantee harm once he is out of the loop? An item decided by the party that raised it, an install
nobody watches, two changes in one bake, a guardrail with no reading, a way back that cannot start the old binary, a rail
change judged by its author's reviewer. Each has a control below. **Second-order thinking**: with agents deciding, rail
changes and grants land without a person, so the decider is a different agent from the asker, every decision is recorded with
its reason, and rail changes get their own verdict.

## Decision

### No agent waits on the owner

- **What stays the owner's** are the acts no agent can do for him: typing a credential, typing a sign-in code, seeing a key's
  or a code's value, and a setting on a system outside Remit that only his account reaches. Overturning a decision stays his
  and blocks nothing. Authentication, pause and stop stay code, pulled by him or the Assistant for him.
- **What cannot be undone waits out an overturn window** (ADR-0101). The Assistant decides it at once, like any item, but
  its Apply of a grant at `irreversible` or of an extension takes effect 60 minutes later, unless he overturns it first;
  the agent that asked is told at once and works on meanwhile.
- **Everything else is decided by an agent as it is raised.** An item that waits for the owner today is routed to the
  Assistant, which decides it through the owner acts it holds (`for_owner`) and records who decided and why. A builtin
  subscription (ADR-0035) on `fact:owner.asked` (`crates/orch/src/fact.rs:53`) wakes the Assistant; the asker is never the
  decider. This covers rows 5–7, 10–25 and 29 of the inventory:
  - change items, grants, standing sensors, extensions, outside MCP servers, extensions on or off, objectives, hiring and
    forgetting (rows 12–17, 20, 21): the Assistant applies or declines; the 14 texts that say "he applies it himself" say the
    Assistant does;
  - a writing capability mid-chain (row 6): `request_capability` goes to the Assistant. A chain a proposal, an event or a
    client started still starts with no writing capability (`may_not_have`), but it may ask for one, and the Assistant
    decides with the origin in front of it (row 7);
  - `ask_owner` and a key result's amendment (rows 11, 18): the Assistant answers. A question of taste is answered with its
    best reading of the owner and told to him; `leave_to_owner` is kept for the true-human acts only;
  - exceptions, a permission request passed up, a gate that asked for changes three times, an error item, a held
    subscription (rows 19, 22–25): routed to the Assistant like any item;
  - stopping, cancelling or restarting a card (row 29): the Assistant gets these tools, as it has pause and stop. Deleting a
    chain stays a manual act: nothing waits on it and it cannot be undone;
  - a merge without `ship` (row 5): the Chair asks for `ship` and the Assistant decides; merging happens only in a chain,
    through `merge_mr`. The Assistant's brief stops forbidding merges and says this;
  - product decisions and naming a repository (row 31): the Assistant answers as for `ask_owner`.
- **The inbox is what agents did, plus the true-human acts.** Each decided item shows its decider, decision and why, with
  Overturn. `Board.waiting` and Needs you (ADR-0078) list only the true-human acts.
- **Admission paces to the merging agents** (row 10). The review room counts approved MRs not yet merged by the seat or Chair
  that merges them; `-max-waiting-mrs` keeps its number and now names that queue.

### The Release seat

- **The playbook.** The built-in `release` playbook becomes the Release seat (group `system`): a Chair that lives by subscribe,
  as the coach does (`crates/orch/src/coach.rs:1-9`), holding `release` (new, writes), `ship`, `signals`, `propose` (to start the revert chain) and `history`. Its
  notes, scope and risks are what the Release train drafted, now written for each release it makes. A builtin subscription
  starts it every hour with action `start`, origin `routine` and overlap `buffer_one`, as a routine does
  (`crates/orch/src/routine.rs:61,95`), only on a server started with `-update-repo` (the repository this instance runs). So
  each instance has one, and a seat that ended is replaced within the hour.
- **What it releases.** `list_releasable` (`release`, read only): open MRs on the instance's own repository whose chain
  finished `done` and whose pushed head carries every verdict the change needs, oldest first. A pure server function; the
  repository decides, never a playbook id or an origin.
- **Merging.** `merge_mr` takes `chain`, another chain's MR, when the caller holds `release` and the MR is on the instance's
  own repository. The door checks that chain's verdicts and pushed head as for its own (ADR-0050) and records the decision of
  kind `ship` on both chains. One change per release: the seat merges the next once the last has baked.
- **Cutting.** `.github/workflows/release.yml` runs on every push to `master` and tags what it publishes
  `<yyyymmdd>-<short sha>`, the form `remit update` reads (`commit_of`, `update.rs:39`). `scripts/release.sh` and
  `make release` are deleted.
- **Released.** A server started with `-update-repo` asks the forge for the newest release every 5 minutes and records
  `fact:remit.released` `{tag, commit, publishedAt}` when it changes. It wakes the seat.
- **Installing.** `install_release {to, why}` (`release`, the Chair; `to` is a tag or `previous`) is the one door that restarts
  the instance. It refuses on a paused or stopped chain, on a server not started with `-update-restart` (the dev instance runs
  from a working copy), and while an update is under way. It spawns `remit update -repo <-update-repo> -restart
  <-update-restart> -to <tag>` in its own process group and no longer the server's child (forked twice, so it is not in the
  tree a supervisor kills, whatever its `treekill` setting, `docs/deploy.md:83-84`) and with no parent-death tie, records a decision of kind `release` (to, from, why) and returns at once. The
  Chair subscribes to `fact:remit.updated` and ends its turn; the restart finds it waiting.
- **The update's result.** `remit update` writes its JSON result to `<releases>/update.json` beside stdout. The server records
  it once per update, keyed by the update's id, as `fact:remit.updated` `{from, to, healthy, rolledBack, oneWay, error}`: when
  the child exits while the server lives, or at the next start.
- **One-way releases.** `remit update` compares the schema version the release knows with the database's; a release that
  migrates is one-way. When a one-way install fails its health check, the way back restores the pre-update copy with
  `previous`, losing only what the failed release wrote in those minutes. After a healthy one-way install the way back is
  forward: a revert of the code that keeps the migration.
- **Health.** The server records `fact:remit.health` `{revision, tag, startedAt, healthy, reason, failures, toolErrors}` at every
  start, at each Tick while a watch on it is open, and otherwise when `healthy` changes. `healthy` is `/healthz`'s rule;
  `failures` counts typed failures the server raised (ADR-0081); `toolErrors` counts `claude_code.tool_result` events whose
  `success` is false for Remit's own tools (a `tool_name` starting `mcp__remit__` or `mcp__remit_seat__`, the servers named in
  `crates/orch/src/access.rs:10` and `crates/orch/src/subagents.rs:28`; the attributes as `crates/telemetry/src/summary.rs:119-133`
  reads them, ADR-0085);
  both over the last 15 minutes.
- **Reading its own facts.** A chain holding `release` reads and watches `fact:remit.*` (`readable_by`, as `coaching` reads
  the teachings).
- **Guardrail, bake and the way back.** The seat merges with the guardrail `fact:remit.health`, a breach condition it sets from
  the readings of the release before (for example `value.healthy == false || value.failures > 3`), and a bake that covers the
  workflow, the install and the watch (at least 60 minutes). A breach wakes it: first `install_release` to `previous` (on a
  one-way release, the revert goes forward instead), then `merge_mr` with `undo` so `master` stops publishing the change. The revert is a Self-development chain the seat starts with `propose_chain` (the seat holds `propose`): judged like any change, listed by `list_releasable`, merged with `undo`.
- **Live.** When the bake ends on a healthy release, the seat marks the builder's chain live (`mark_live`, for a chain whose MR
  it merged): `chain:<id>.live` is recorded and the rechecks after going live run.
- **The builders.** Self-development's opening and brief say a change reaches users when the Release seat merges and installs
  it. Its agents finish at an MR approved by the reviewer, QC and, when it touches a rail, security; the MR states what in
  `fact:remit.health` would show it hurts, how it is taken back, and whether it migrates. They never merge, restart, rebuild or
  redeploy; the playbook holds no `ship`.

### Rails judged by an agent

- A repository declares its rails in `.remit/rails` (path globs; the file always counts as one, so an MR that edits it needs
  the verdict too). A change touching a rail needs a third verdict, `security`, on the pushed head: the verdicts required are a
  pure function of the paths the change touches (`gate.rs`, beside `missing_approvals`). Self-development seeds a Security
  reviewer whose brief runs a threat model (`aio-threat-models`) and writes it in the MR.
- Remit's own `.remit/rails` names the crates `auth`, `secrets`, `jail`, `egress`, `childenv`, `procgroup`, `clientmcp`, `door`,
  `rollout`; in `crates/orch/src` `policy.rs`, `ship.rs`, `gate.rs`, `owner_acts.rs`, `capabilities.rs`,
  `capability_request.rs`, `lifecycle.rs`, `client_keys.rs`, `entrance_keys.rs`, `worker_keys.rs`; `crates/remit/src/update.rs`;
  `crates/store/migrations`; `.github/workflows`; and `.remit/rails`.

### How the owner sees and steers it

- Each decision shows where the agent that made it works: the Assistant's feed for what it decided, the Release seat's screen
  for each merge, install, health reading and way back, the builder's chain for "Merged by Release" and "Live on <tag>".
- He tells the Assistant or the seat in words ("hold releases until Monday", "take back the last one", "ask me before
  granting production"); the Assistant keeps such a rule and leaves the matching items to him. Pausing the seat stops its
  merges and installs; stopping ends it, and the hourly subscription starts another unless he pauses that subscription. His
  own `remit update -to previous` on the machine stays his.

## Threats

The Assistant now decides grants, capabilities and rail lifts, and the Release seat holds the merge and the restart.

| Threat | Control |
|---|---|
| A chain grants itself power by asking (elevation) | The asker is never the decider: items go to the Assistant, a different agent; a Chair still gives only capabilities that do not write |
| Text in an item, an MR, a log or a reading steers the decider (OWASP LLM01) | The Assistant's and the seat's briefs name every item and reading data; verdicts, the bake and the guardrail are code; what the Assistant applies on the top rung of the effect ladder takes effect after the overturn window (ADR-0101) |
| A decision lands unseen | Each is recorded with decider and why and shown in his feed with Overturn |
| The seat merges a change nobody judged (tampering) | `merge_mr` checks the other chain's verdicts on its pushed head; the forge merges only that sha (ADR-0050) |
| A rail change is judged only by its author's reviewer | A `security` verdict with a threat model is required by the door for any path in `.remit/rails`; the file guards itself |
| A credential reaches an agent | The four secret acts stay `secret: true` in `owner_acts.rs`; `for_owner` refuses them to the Assistant |
| A release the workflow did not build reaches the instance | `remit update` checks the asset against `SHA256SUMS`, which the same release carries. Nothing yet ties a release to `master`: a branch with its own copy of `release.yml` can publish one (see *Revisit when*) |
| An install leaves the instance down | `remit update` goes back to `previous`; a one-way release goes back to the pre-update copy; his own `remit update` stays |
| Two changes bake together and a breach names neither | One change per release, merged only after the last baked |
| A chain started by a proposal, an event or a client gets the seat's powers | `release` writes, so `may_not_have` keeps it from their start; asking for it goes to the Assistant |

## Consequences

- An approved change of Remit reaches every instance that runs releases without a person; an item no longer stops a chain
  until the owner looks.
- The Assistant's judgement now carries what his did: a wrong decision is found after, in his feed, not before.
- Every push to `master` is a release (about 18 minutes of the workflow each). A one-way release cannot go back to the binary
  before it.
- Each instance knows only its own health: staging does not hold the pod back.
- Rail changes cost a third verdict.
- Running a backfill against a shared store stays prepared-only (row 31): no source serves a write yet, so a chain finishes
  with its runbook and the Assistant tells the owner. It is the one wait left that is not a secret act.
- New entities and words: the capability `release`, the decision kind `release`, the facts `remit.health`, `remit.released`
  and `remit.updated`, the server flags `-update-repo` and `-update-restart`, the file `.remit/rails`, the verdict role
  `security`; the `release` playbook changes meaning.

## Revisit when

- **Hardening of the release path is asked for** (the security review of 2026-11-12 named these, deliberately unbuilt):
  signed releases, and `remit update` refusing a release whose commit is not on `master`; a `security` verdict from an agent
  outside the builder's chain (today the builder's Chair hires it); a class of inbox item the Assistant may not decide (the top rung of the effect ladder waits out an overturn window, ADR-0101),
  `release` and `ship` grants to chains an event, a client or a proposal started first; health readings agents cannot push
  (`fact:remit.health` counts telemetry agents send); a way to stop an update already running when its chain is paused; a
  pause the owner pulled himself lifted only by him, and an extension he turned off turned on only by him.
- **Programs outside an agent's session must be kept from the releases directory:** sessions and their shells run with
  `<data>/releases` hidden (`Flags::protected_from_agents`, the one list of server-private paths both jail adapters
  apply), but a plugin runs with the server's rights (ADR-0034) and could still write its links, lock or `update.json`.
  The control not built is a release directory owned by another OS user, or `remit update` checking `current`'s binary
  against the release's `SHA256SUMS` before it restarts onto it. The seam is `UPDATE_ENV` and `Updater` in
  `crates/remit/src/config.rs`: the update runs with only the names listed there besides `childenv`'s.
- **A supervisor kills the whole control group** (systemd `KillMode=control-group`): a double fork stays in the cgroup and
  dies with the server, so `install_release` would start the update as its own unit (`systemd-run`) instead
  (`procgroup::detach`, `crates/procgroup/src/detach.rs`).
- **Hardening is asked for** (`aio-threat-models` on the release path and on the Assistant's decisions): signed releases
  checked by `remit update`, refusing a lower version, a second agent verdict on rail changes, a daily cap on merges and
  grants, paths no agent merges, a class of item the Assistant may not decide. The seams are `.remit/rails`, the verdict roles
  `gate.rs` derives, the checks of `remit update` and the `OwnerAct` table.
- **A source serves writes** (K6's executor, ADR-0027's ladder): a backfill runs through it and the last non-secret wait goes.
- **Instances see each other** (ADR-0072's node identity): live installs a release only after it baked on staging.
- **A breach must not wait for a Chair's turn:** a subscription action that installs `previous` in code (ADR-0050's `undo`
  seam).
- **The schema grows by expansion only:** a binary that accepts a newer schema it does not fully read makes a migrating
  release two-way.
- **Installs disturb work in flight:** the seat installs only when no chain is mid-turn (K11 (c)).
- **An `undo` merge is taken on its word:** while a stage bakes, a chain holding `ship` may merge any approved MR as
  `undo`. The control not built: the stage records its merge commit, and `merge_mr undo` compares the MR's patch with
  that commit's inverse.
- **The base can move between the gate's read and the merge:** a rail added to the target branch in that window is not
  seen, since neither forge's merge call can be pinned to a base commit. The control not built: re-read the base after
  the merge and take the merge back when its rails changed.

## Addendum (2026-11-13): the same question is decided once, and a digest of what was decided

- **Already asked.** When a question is put to the Assistant (`fire_assist`), its message lists the questions other
  chains asked the owner that are still open (`already_asked` in `crates/orch/src/assist.rs`, the 20 newest), each with
  whether it was left to him. The Assistant decides a repeat as the first was decided and names the item it repeats; a
  repeat of an item left to him is left to him too, naming it, so he answers both at once. The list goes to the
  Assistant, not to the asking Chair before it asks: every ask reaches the Assistant first (this ADR), so it is the one
  place that sees them all at the moment it decides, with no race between two chains asking in the same second, and the
  asker could not act on another chain's question anyway. Revisit when the owner's one answer should close its repeats
  by itself (a link from an item to the one it repeats).
- **Digest.** On the owner's words the Assistant sets a digest of the last day or week (`set_digest`, `list_digests`,
  `stop_digest`): a Remit subscription per period, on his cron, running the job `digest-day` or `digest-week`
  (ADR-0035), whose firing posts a message on the Assistant's card, built from the store by pure code
  (`crates/orch/src/digest.rs`): the chains that finished and how each ended as the board says it, the Assistant's
  decisions for him and his overturns. No model call, no turn. Revisit when the owner wants it outside the panel (a
  channel of its own) or wants it to say why a chain did not end done beyond the board's words.
