# 0116. What lands is released by its repository's own procedure, and a breach is taken back by code

Status: accepted, 2026-11-19; built 2026-11-19, *As built* below (slices 6 to 9, and slice 10's docs; slice 10's
cut-over on the dev instance follows). Supersedes ADR-0087's Release
seat (the chain of kind `release`, the capability `release`, `list_releasable`, the seat's `mark_live`, its brief, the
hourly subscription that starts it, and "one change per release"; the seat's install, watch and way back become code that
serves every repository), ADR-0114's Release seat that installs (the seat it kept goes), ADR-0068's check-rail merge door
(`CheckDoor::AtMerge`: "a merge always ships") and ADR-0113's kind `release`. Amends ADR-0018 (a grant may be held by a
repository, for the release that runs as no chain), ADR-0025 (`.remit/release/` is written by chains as a rail;
`.remit/checks` stays closed to them) and ADR-0087's Assistant (it holds no request to land or release, and declines no
chain's power on taste). Source: the owner, 2026-11-19: "Tôi muốn các chain có thể tự merge và tự release, full quyền, vì
để đạt được kết quả, phải có thực quyền" (chains merge and release their own work, with full rights, because reaching a
result needs real power).

## Context

The customer is the person a change is for; what he gets is the change in his hands: the crate he installs, the page he
opens, the server he talks to. A change on the base branch is not yet that. ADR-0114 makes landing Remit's code for every
repository. It answers release for one repository only, Remit's own, through the Release seat (ADR-0087): a chain of kind
`release` (`capability::kinds`, `crates/capability/src/kinds.rs:36`) that a builtin subscription starts only on a server
given `-update-repo` (`release_seat(update_repo)`, `crates/pipeline/src/automation.rs:185`), whose door `install_release`
restarts this server on the release the workflow published (`crates/shipping/src/release.rs:1-5`). Every other repository
has no way to release at all.

The incident that made this concrete, on the live instance, 2026-11-18 to 19: chains `ch_3d3b8109bb4d` and
`ch_b6bf733f6332` on `aiocean/pixfx-rs` (a Rust crate with a wasm build, npm packages and a site; its one release, `v0.3.0`,
was cut by hand, and it has no `.github/workflows`, per `gh api repos/aiocean/pixfx-rs/contents`) finished reviewed and
QC'd work and asked for `ship`. Two rules stopped them:

1. **The Assistant declined on taste** ("a visual effect is the owner's taste to see first") and parked both MRs on him
   for more than 12 hours. Its brief invites it: the one line on merging (`crates/chains/src/assistant.rs:388`) makes it
   the decider of a `ship` request, so a request it may decline. ADR-0087 already says no agent waits on the owner and a
   question of taste is the Assistant's to answer (`crates/builtin/skills/app/references/inbox-and-credentials.md:33`);
   the brief gave it a door to park work anyway.
2. **The check rail refused the merge three times out of three, after `ship` was granted.** `merge_mr` calls
   `production_refusal` (`crates/shipping/src/ship.rs:219`), which reads the key result's check at
   `CheckDoor::AtMerge` (`crates/shipping/src/check_rail.rs:140-147`); `check_gaps` says "a merge always ships"
   (`check_rail.rs:170`) and demands telemetry (`Check::seen`), an incident plan and an open watch on it
   (`check_rail.rs:182-206`). A library has no runtime signal to be seen through, so its Chair could never merge.

Both are the same defect from two sides: power to put a result in the customer's hands was held by something other than
the evidence about the result. ADR-0114 removes both doors for landing (no `ship`, a stage plan optional). This record
does the same for release, which ADR-0114 left to a seat that knows one repository.

What a release is differs by repository: Remit's own is the workflow's GitHub release of each commit of `master`,
installed on each instance by `remit update` (ADR-0087); pixfx-rs's is a version published to crates.io and npm and a
site deployed; a documentation repository's is a site; a data repository's may be nothing. What is the same is the shape:
put the landed commit where its customer reaches it, watch it, and take it back if it hurts.

Mental models. **Inversion**: what would guarantee a repository whose work never reaches its customer? A release that
needs a person to write its procedure, a door that demands a signal the work cannot have, a decider who may say no on
taste, and credentials only a person can use. **Second-order thinking**: with hundreds of repositories and agents landing
all day, a standing agent per repository that calls the same five tools in order is a turn per release and a failure mode
per turn (the seat ending, starting late, misreading its own brief); the second-order cost of keeping a seat is that every
improvement of the loop is a brief edit no test can hold. **Margin of safety**: the risk taken is real (an agent-written
procedure publishes under the owner's accounts), so the way back is built before it: code-driven, declared beside the
procedure, and run without a turn. Patterns: **GitOps** (vol. 9: the desired state is declared in Git and an operator
reconciles the world to it, pulled and idempotent; here the landed commit declares what is published, and the procedure
reconciles), **Safe hands-off deployments** (vol. 4: automated rollback on a guardrail, bake times, a release train whose
batch is bounded), **Hexagonal ports** (vol. 2: the one install target that is this server is an adapter behind the same
release, not a second mechanism).

## Options

**What a release is, for any repository.**

1. **A tag that the forge's workflow acts on.** Remit pushes a tag on the landed commit; the forge's pipeline publishes
   with the secrets the owner set on the forge. Credentials never reach Remit. But a chain may not write the pipeline
   (`forbidden_path`, `crates/shipping/src/submit.rs:171-183`), and ADR-0114 relies on that, since the forge's CI is the one
   check no agent writes. So every repository without a release workflow (pixfx-rs) waits for a person to write YAML:
   a wait on the owner by construction. Rejected as the mechanism; kept as one thing a procedure may do (push a tag
   through the forge connection, and let a workflow the owner already has act on it).
2. **Typed release tools per target** (`publish_crate`, `publish_npm`, `deploy_pages`). Each is safe and legible, and
   each is a new tool, grant and adapter per ecosystem; a repository whose release is anything else (a Helm chart, an app
   store, a data file uploaded somewhere) waits for Remit to grow one. Rejected: Remit would be the bottleneck of every
   release kind.
3. **A plugin per repository's release** (ADR-0034). General, but a plugin runs with the server's rights and is a process
   to install; a release is a few commands. Rejected for the common case; a plugin may still provide a source a
   procedure reaches.
4. **The repository declares its release procedure in `.remit/release/`, beside `.remit/checks` (chosen).** Executables
   Remit runs in name order on the landed commit, in the jail, whose only way out is the injecting proxy to the
   connections the directory names: the credential is added by the proxy and never seen by the procedure (ADR-0018, form
   T1). A chain writes it, as a rail. Option 1 is the procedure "push the tag"; Remit's own repository's procedure is
   "wait for the workflow's release of this commit".

**Who triggers it.**

1. **The landing chain's Chair, with a `release` tool.** Judgement where none is needed (a commit landed and checked is
   ready by evidence), a turn per release, and two chains landing on one repository race to release each other's work.
2. **A Release seat per repository.** The ADR-0087 loop multiplied: one standing chain per repository, a brief to keep
   right, a seat that ends.
3. **Code, after every landing, one release at a time per repository (chosen).** What decides that a release is due is
   evidence: the base moved, its checks and the forge's pipeline on it are green, no release on the repository bakes.
   What is released is decided in the commit itself (a library's version bump is part of the change its gates judged),
   so no agent is needed at the moment of release. The landing chains are told and watch their own result.

**What the Release seat becomes.**

1. **Keep it for Remit's own repository, code for the rest.** Two mechanisms for one concept.
2. **One mechanism (chosen).** The seat goes. Remit's own repository declares a procedure like any other; installing on
   this server is a target the same release runs last, only on the repository `-update-repo` names, through the code
   door `install_release` already is.

## Decision

Option 4, triggered by code, one mechanism. Landing (ADR-0114) and release (this record) are both Remit's code; agents do
the work, judge it and author the procedure; no agent and no person is asked whether to land or release.

### The procedure

- `.remit/release/` in the repository, read from the commit being released:
  - **steps**: executable files run in name order (`10-build`, `20-publish-crate`, `30-deploy-site`, `90-smoke`), each in
    a jail of its own, stopping at the first that fails. The work directory is a fresh, writable checkout of the release
    commit (made from the landing worktree of ADR-0114, removed after); its environment holds `PATH`, `HOME`,
    `REMIT_RELEASE_COMMIT`, `REMIT_RELEASE_PREVIOUS` (the commit last released, empty for the first),
    `REMIT_RELEASE_OUT` (a file the procedure writes) and the proxy's variables; no credential.
  - **`connections`**: one line per connection a step may reach (`<source>/<connection>`), for example `http/crates-io`,
    `http/npm`, `github/aiocean`. Nothing else is reachable.
  - **`guard`** (optional): the guardrail every release of this repository bakes under, one line
    `<signal key> <bake minutes> <breach condition>`, for example `fact:remit.health 60 value.healthy == false ||
    value.failures > 3`. A repository with no runtime signal (a library) has none.
  - **`back`** (optional): executables run, in the same jail and with the same connections, when a release is taken back,
    with `REMIT_RELEASE_BREACHED` and `REMIT_RELEASE_PREVIOUS` set: redeploy the previous site, yank the version, move a
    dist-tag back. A repository with none goes back forward only (below).
- **The contract a procedure keeps.** It is idempotent: run twice on one commit it ends in the same state (publish the
  version the commit declares if it is not published yet; deploy the site at this commit). It writes
  `REMIT_RELEASE_OUT` as JSON `{"released": bool, "version": "...", "links": ["..."]}`: `released: false` says this
  commit carries nothing new to publish (a library change without a version bump), and its landings wait for the release
  that does. A step that exits 75 (`EX_TEMPFAIL`) says the outside was not reachable: the release is tried again at the
  next Tick and nothing is taken back. Any other failure is a breach.
- **What runs it.** `shipping::release` (rewritten): a pure core that parses the directory, picks the next release and
  decides what a breach takes back, and a shell that runs the steps through `reach::script` with an injector made for the
  run. The step deadline is the checks' pattern with a longer bound (a wasm build or the forge's workflow may take tens of
  minutes); limits are the jail's.
- **Who writes it.** A chain, in its change, like any file. `forbidden_path` narrows from all of `.remit/` to
  `.remit/checks/` and `.remit/rails` (a chain still cannot grade itself, nor say which paths are rails); every path under
  `.remit/release/` is a rail by code (`gate.rs`, as `.remit/rails` counts as one), so a change to it needs the `security`
  verdict with its threat model. The first release procedure of pixfx-rs is a work chain's change, judged and landed.

### Credentials

- A procedure reaches the outside only through the injecting proxy (`crates/egress/src/inject.rs`), which ends its TLS,
  admits a request by the rules of the connection's source and adds the credential headers; the procedure gets a
  placeholder for any tool that insists on a token in its environment, and an authority to trust in the variables the
  runner already sets (`crates/reach/src/script.rs`). The injector serves every connection the directory names in one run
  (today it serves one destination) and admits writes up to the effect the grant allows (today a script only reads,
  `crates/source/src/script.rs:1-9`).
- **A repository holds grants.** A grant today is held by a chain (`chain_grants`, `crates/store/migrations/0054.sql`). A
  release runs as no chain, so a grant gains a holder: a chain or a repository (`grants`, one table, one rule:
  `grant::decide` is unchanged, `crates/grant/src/lib.rs:168`). A release step reaches a connection only through a grant
  its repository holds at the effect the request needs; publishing is `irreversible`.
- **Asked and decided as every grant is.** The chain whose change adds a connection to `.remit/release/connections` asks
  with `request_grant` naming the repository; the Assistant decides it for the owner (ADR-0087), and an `irreversible`
  grant takes effect after the overturn window (ADR-0101, 60 minutes); the asking chain works on meanwhile. The one act
  left to the owner is typing the credential into the connection's write-only field (`ACT_CREDENTIAL`,
  `crates/chains/src/owner_acts.rs`), a true-human act; a release that needs a connection with no credential is not
  run, and its credential item says which release waits on it.

### When a release runs

- After a push of the queue lands (ADR-0114) and the forge's checks on the pushed commit are green (or at once on a
  repository whose forge runs no pipeline), and while no release on that repository bakes, Remit releases at the commit
  of the `k`-th oldest landing not yet released, where `k` is sized by the same additive-increase, multiplicative-decrease
  rule as the queue's batch (one pure function in `shipping::landing`, called for both): one more after a release that
  holds, halved after one taken back. The landings between the last released commit and that one are the release's.
- A release is guarded by the steps' own exits (a smoke step is the repository's to write), the forge's checks on the
  commit, the repository's `guard`, and the guardrail of every stage plan its landings carry (ADR-0114's optional plan,
  vetted by `rollout::vet` when it was submitted). Its bake is the longest of those. With no guard and no plan (a
  library) it is released when its steps pass.
- **On this server's own repository** (the one `-update-repo` names) the release runs one more step after the
  procedure, in code: `install_release` to the version the procedure reported, the detached `remit update` of ADR-0087,
  unchanged, with its refusals (no `-update-restart`, an update under way) and its result (`fact:remit.updated`). Remit's
  own `.remit/release/` holds one step that waits for the workflow's release of `REMIT_RELEASE_COMMIT` through the forge
  connection (a read) and reports its tag, and a `guard` on `fact:remit.health`; the server's own five-minute poll for a
  new release goes.

### The way back

- A release is breached when a step fails (not 75), the forge's checks on its commit go red while the last released
  commit's were green, or a guardrail of its guard breaches during the bake. Remit then, in code and without a turn:
  1. runs the repository's `back` steps, or, on this server's own repository, `install_release previous` (a one-way
     release, one that migrated, goes forward instead, as ADR-0087 says);
  2. queues the revert of every landing the release carried at the head of the queue (ADR-0114's revert, the same path);
  3. once the revert lands, releases it like any landing: the way back that every repository has, with or without
     `back`, is a release forward to the last judged state.
- Each landing's chain is told with the release's output and readings and its Chair woken to understand and land a fix;
  its reverted landings re-enter its hands as ADR-0114 says.
- **The owner, or an agent, takes a release back with words.** `revert_release` (the Assistant's for any release; a
  Chair's for a release that carries its landing) runs the same three steps, recorded with who and why. Like
  `revert_landing` it acts at once (ADR-0101 holds only the irreversible, and a taking back restores).
- A release that publishes and a release taken back are recorded as `fact:repo.release` `{repository, release, version,
  commit, landings, state}`; when its bake ends healthy, each of its landings' chains is marked live
  (`chain:<id>.live`, the rechecks after going live run) by code.

### The check rail

- `production_refusal` and the `CheckDoor::AtMerge` door go with `merge_mr`: no door between a judged change and the
  base demands telemetry. A landing with no stage plan is guarded by its gates, the combined checks and the forge's CI
  (ADR-0114), and a release with no guard by its steps.
- `check_gaps` keeps one door, the finish, and its `ships` parameter (a chain holding `ship`) goes: telemetry, an
  incident plan and a watch are asked of a key result whose check reads production (`CheckKind::Reading`), where a
  signal exists by definition. A stage plan's guardrail is `rollout::vet`'s to vet at `land`, as ADR-0114 says.

### The Assistant

- It holds no request to land or release: none exists. Its brief's line on merging (`assistant.rs:388`) becomes: it
  cannot delete a chain or push; landing and releasing are Remit's code; it never holds a chain's work back, on taste or
  on anything else, since taste is the gates' to judge on the customer's side (ADR-0016) and the owner's to overturn
  after; when he says to take something back, it calls `revert_landing` or `revert_release`.
- It decides a repository's grant like any grant, with the procedure's diff and the connection in front of it.

### What the owner sees

- Each repository has a Releases view the server derives, beside its queue (ADR-0114): each release, live: its landings
  and their chains, each step's output as it runs, the version and links it reported, the guard's readings through the
  bake, and a taking back with what ran, who asked and why.
- A chain's screen shows, on each of its landings, the release that carried it and its state, and "Live in <version>" once
  it baked.
- The Assistant's feed shows the repository grants it decided, with Overturn. The inbox holds, of all this, only a
  credential to type.
- He steers in words: "take back the last release of pixfx", "hold releases of the site until Monday" (the Assistant
  pauses the repository's releases, a pause like a chain's: deterministic, pulled and lifted by him or the Assistant
  for him, recorded with who and why; landing goes on, and the release resumes at the base's head).

## Threats

A rail changes: code now publishes, under the owner's accounts, what agents wrote, by procedures agents wrote. Threat
modelling (`aio-threat-models`) runs when the owner asks for hardening; the controls below are built with this record,
and the seam for each one not built.

| Threat | Control | Seam not built |
|---|---|---|
| An agent-written procedure publishes something harmful under the owner's account | The procedure is a rail: its change needs `security`'s verdict and threat model; it runs only on a landed commit that passed the gates and the combined checks; an `irreversible` grant takes effect after the overturn window | `land_policy` (ADR-0114) by protected paths, requiring a stage or a second verdict for `.remit/release/` |
| A procedure reads or leaks a credential | Form T1: the credential is in the proxy, the procedure's environment and files hold none, the answer is scrubbed of it (`crates/egress/src/inject_exchange.rs`) | Form T2 (credential in the environment) is not offered; a tool that pins its certificates waits for a typed adapter |
| A procedure reaches somewhere it should not | The jail has no network but the injector, which connects only the connections named and granted, and admits writes only up to the grant's effect | Per-step connections; a daily bound on releases per repository |
| A release breaks its customer | The steps, the forge's checks, the guard and the stage guardrails, the bake; the way back runs in code: `back`, the reverts, the release forward | A canary target a procedure declares (a dist-tag, a preview site) before the full one |
| The way back cannot undo a publish (a registry keeps versions forever) | The release forward supersedes it with the next version; `back` yanks what the registry lets be yanked | Recorded in the Release; nothing more is possible |
| A flaky outside reverts good work | Exit 75 retries without taking back; a breach on the forge's checks needs the last released commit green | A re-run of a failed step before taking back |
| Two releases race on one repository | One writer per repository, one release at a time, none while one bakes; a crash resumes by re-running the idempotent procedure on the recorded commit | |

## Consequences

- Every repository a chain works on can reach its customer with no person and no agent asked: pixfx-rs releases once a
  chain lands its `.remit/release/` and the Assistant grants its connections; its credentials are the owner's one act.
- The Release seat goes: `ChainKind::Release`, `CAP_RELEASE`, `RELEASE_CHAIR_BRIEF`, `release_seat`, `list_releasable`
  and the seat's `mark_live`; `install_release` stays as code, called by the release of this server's repository and by
  `revert_release`. A turn per release, and the hour a new seat takes to start, go with it.
- "One change per release" (ADR-0087) becomes a measured release size: a breach names a set of landings, all reverted
  and re-released in smaller sets, as ADR-0114's batches are.
- A procedure is code in the repository, judged as code: its quality is the repository's, and a bad one is fixed by a
  chain, not by Remit.
- New: `model::Release` (table `releases`), `Landing.release`, a grant's holder, `fact:repo.release`, `revert_release`,
  the files of `.remit/release/`. Gone: the kind and the capability `release`, `CheckDoor::AtMerge`,
  `production_refusal`, `fact:remit.released` and its poll.
- Every chain's main path now ends in a release where the repository declares one, so the main-path test of ADR-0114
  extends through the release; the rail tests are named in the plan, none before it exists.

## Plan

ADR-0114 and this record are built together. Every slice leaves the workspace compiling with clippy and tests green, and a type or field is deleted in the slice that removes its last caller (the MR fields with slice 9's web, `pushed_sha` and `pushed_from` with slice 7, `CAP_SHIP` with slice 5, `ChainKind::Release` and `CAP_RELEASE` with slice 7). Each slice is whole (code, callers, migration, deleted code, docs, the tests
that guard its rails), in this order; `make test` stays under one minute. The migration numbers are the next free when a
slice lands (0119 and 0120 today, after `crates/store/migrations/0118.sql`).

| # | Slice | Crates and files | Migration | Web | Rail tests | Agent |
|---|---|---|---|---|---|---|
| 1 | **Records.** `model::Landing` (ADR-0114), `model::Release`, `Landing.release`, a grant's holder (chain or repository); `MRRef`, `MRState`, `MRChecks`, `RepoWork.mr`, `earlier_mrs`, `pushed_sha`, `pushed_from` go; `ChainKind::Release` goes | `crates/model/src/lib.rs` and its record files, `crates/store/src/migrations.rs`, the store's read and write functions | 0119: `landings` from each `RepoWork.mr` (ADR-0114's cut-over); `releases`; `chain_grants` becomes `grants` with `holder`; chains of kind `release` finished and their kind and capabilities rewritten; `ship` and `release` removed from every chain's capabilities; idempotent | none | stored MRs read as landings after the migration; a stored grant reads the same after the move | store-and-migrations |
| 2 | **Contract.** `Landing`, `Release`, their notifications, the queue and Releases views in `ChainView` and the repository's view; `make gen` | `crates/wire/src/lib.rs`, `crates/rpc/src/convert.rs`, `crates/replica`, `web/src/core/generated` | none | generated types only | the contract read across versions (existing pattern) | rpc-contract |
| 3 | **The queue.** `shipping::landing` pure core (batch, the AIMD size both slices 3 and 7 call, bisection, what a revert takes) and shell (landing worktree, combine, checks in the jail, push never forced), the `Landing` port and `DirectPush`; `land` replaces `submit_mr`; `push_ref`, `pushable`, the `refs/remit/push` rewrite, `open_mr`, `update_mr`, `retarget`, `merge`, `diff` go; `fact:repo.landed` and the overlap notice | `crates/shipping/src/submit.rs`, `crates/shipping/src/repo.rs`, `crates/shipping/src/gate.rs`, `crates/shipping/src/service.rs`, new `crates/shipping/src/landing.rs`, `crates/chains/src/forges.rs`, `crates/forge/src/lib.rs`, `crates/chains/src/fact.rs`, `crates/orch/src/repo_view.rs` | the next free: an indexed `ended_at` column on `landings` (landed, returned or reverted), so the repository's view reads the last day by index rather than every landing | none | only a version whose patch id carries every required gate lands; a changed patch is judged again; a failed combined check is bisected to its entry and the rest land; a refused push recombines and never forces; a crash between record and push resumes to one landing | review-and-ship |
| 4 | **After the push.** The bundled forge plugin's `landing` sensor and `Forge.Checks` replace the `merge-request` sense and `Forge.Standing`; the automatic revert at the head of the queue; `revert_landing` (Assistant, Chair); `fact:chain.landed` and the goal judge's recheck; `merge_request_sense`, `threads`, `ship.rs`'s `merge_mr` and `waiting::merging` go | `crates/bundled/src/forge.rs`, `crates/reach/src/forge_service.rs`, `crates/shipping/src/merge_request_sense.rs`, `crates/shipping/src/threads.rs`, `crates/shipping/src/ship.rs`, `crates/shipping/src/recheck.rs`, `crates/shipping/src/goal.rs`, `crates/orch/src/waiting.rs`, `crates/orch/src/standing.rs`, `crates/chains/src/admit.rs`; the MR readers in `crates/outcomes/src/outcomes.rs`, `crates/crew/src/lineage_standing.rs` and `crates/crew/src/teach.rs` read landings and `fact:chain.landed` | none | none | a red pipeline on the pushed commit after a green base reverts the batch by code | sources-and-plugins (sensor, service), review-and-ship (revert, recheck), objectives-and-crew (outcomes, standing, teaching) |
| 5 | **The check rail.** `production_refusal` and `CheckDoor::AtMerge` deleted; `check_gaps` keeps the finish and asks telemetry only of a `Reading` check; `rollout::Plan::merged` becomes the landing's stage (`Stage.mr` names the landing); `CAP_SHIP` and its checks in `check_rail.rs`, `release.rs`, `admit.rs`, `capability_tools.rs` go | `crates/shipping/src/check_rail.rs`, `crates/rollout/src/lib.rs`, `crates/chains/src/cap_names.rs`, `crates/capability/src/capabilities.rs`, `crates/orch/src/capability_tools.rs` | none | none | the chain's main path: a change to a repository with no runtime signal and no stage plan lands once its gates approve (the pixfx-rs case) | review-and-ship |
| 6 | **The procedure runner and its credentials.** Parse `.remit/release/` (pure); `forbidden_path` narrows to `.remit/checks/` and `.remit/rails`; `.remit/release/` is a rail in `gate.rs`; the injector serves several connections and admits writes up to the grant's effect; a repository holds grants, asked with `request_grant` and decided by the Assistant, `irreversible` after the window | `crates/shipping/src/submit.rs`, `crates/shipping/src/gate.rs`, `crates/shipping/src/release.rs`, `crates/egress/src/inject.rs`, `crates/egress/src/inject_rules.rs`, `crates/source/src/script.rs`, `crates/reach/src/script.rs`, `crates/reach/src/grants.rs`, `crates/reach/src/grant_tools.rs`, `crates/grant/src/lib.rs`, `crates/jail` | none (holder in 0119) | the repository's grants in the Assistant's feed | a release step's environment, files and output hold no credential; a step reaches nothing but its granted connections; a write is refused without an `irreversible` grant; a change to `.remit/release/` needs `security` | rails-and-sandbox (injector, jail), sources-and-plugins (grants, script), review-and-ship (parse, rail path) |
| 7 | **Releases.** `shipping::release` rewritten: the next release, its guard and bake, the steps, `install_release` as a step for `-update-repo`, the breach and the way back (`back`, reverts, release forward), `revert_release`, live by code, `fact:repo.release`, a repository's release pause (the Assistant's and the owner's, recorded with who and why); the Release seat deleted (`RELEASE_CHAIR_BRIEF`, `release_seat`, `list_releasable`, `CAP_RELEASE`, the seat's `mark_live`, `fact:remit.released` and its poll); `fact:remit.*` readable by any chain; Remit's own `.remit/release/` and `.remit/rails` (its `ship.rs` and `rollout` entries become `landing.rs` and `release.rs`) | `crates/shipping/src/release.rs`, `crates/shipping/src/remit_facts.rs`, `crates/remit/src/update.rs`, `crates/remit/src/config.rs`, `crates/remit/src/flags.rs`, `crates/capability/src/kinds.rs`, `crates/capability/src/briefs.rs`, `crates/pipeline/src/automation.rs`, `crates/chains/src/watch.rs`, `crates/chains/src/live.rs`, `.remit/rails`, `.github/workflows/release.yml` (unchanged, read) | none | none | a breached release runs `back`, reverts its landings and releases forward with no turn; no release starts while one bakes; a one-way release of this server goes forward | review-and-ship (release), release-and-ops (Remit's own procedure, `update.rs`), signals (`automation.rs`, facts) |
| 8 | **Agents' words.** The Assistant's line at `assistant.rs:388` as *The Assistant* says; the manual's references; the Chair's prompts and the gates' instructions say `land`, not `submit_mr` or `merge_mr`; the self-development brief says a change reaches users when it lands and its release bakes | `crates/chains/src/assistant.rs`, `crates/builtin/skills/app/references/inbox-and-credentials.md`, `crates/builtin/skills/app/references/tools-and-connections.md`, `crates/chains/src/prompts.rs`, `crates/capability/src/gate_instructions.rs`, `crates/capability/src/playbooks.rs` | none | none | none (words) | agent-tools |
| 9 | **Screens.** A chain's landings and their releases; the repository's queue and Releases views; the MR panels and the Release seat's screen go; each seeded on a `web/verify` page with a busy queue, a returned entry, a release baking and one taken back, looked at and tabbed through | `web/src/components/ChainChanges.vue`, `web/src/components/ChainPage.vue`, new view components, `web/verify`; the client core holds the repository's view (`repo_get`, `repo_updated`, reopened after a reconnect) beside the screen that opens it (`crates/replica`) | none | as listed | none | web-ui |
| 10 | **Docs and cut-over.** `docs/design.md` (Shipping, the Release seat section becomes Releases, the capability table, Self-development), `docs/remit-design.md` (K6, K13), `crates/orch/README.md`, `docs/deploy.md` (the push right on each base, the connections a release needs); on the dev instance: migration run, the one-time close of carried-over draft MRs (ADR-0114), the two pixfx-rs chains' approved versions land, and a work chain writes pixfx-rs's `.remit/release/` | the files named | 0119 run on `~/.remit-dev` | none | none | architect (docs), release-and-ops (dev instance, deploy guide) |

## As built

What was built, where the build differs from the decision above, and every seam left.

- **The procedure** is parsed by one pure function, `shipping::release::procedure`, over the commit's tree as `git
  ls-tree -r --full-tree` lists it: a file directly under `.remit/release/` (or `back/`) with mode `100755` is a step, in
  name order; a file in a deeper directory is a helper the steps call. `connections` and `guard` take `#` comments.
  A guard's bake is 1 minute to 1 week. `forbidden_path` refuses `.remit/checks/` and `.remit/rails` alone
  (`crates/shipping/src/land.rs`); `gate::RELEASE_DIR` makes every path under `.remit/release/` a rail.
- **The step's environment** also holds `TMPDIR` (its scratch directory, beside `HOME`), `LANG` and
  `REMIT_RELEASE_REPOSITORY` (`host/project`): a step runs in a checkout whose git directory is outside its jail, so
  it learns its repository from Remit rather than from git (`shipping::release::step_env`). The checkout is a detached
  worktree of Remit's clone at the commit, writable but its `.git`; the scratch directory is removed after.
- **The runner** is `crates/shipping/src/release_run.rs`: one runner per repository in memory, as the queue's
  (`ReleaseRuns`), kicked by `sync_landings` and by a landing whose checks stand. One base is released per
  repository: the base its newest landing went to. A landing counts as settled when its checks stand, or when the forge
  said nothing of it and its base for 10 minutes (`NO_PIPELINE_AFTER`, a repository with no pipeline). The landings a
  release carries are tagged `Landing.release` when it is created, so a breach in its steps reverts them, and untagged
  when it publishes nothing. After a release taken back, the next waits until no landing it carried is still on the
  base, then carries their reverts. The first release of a repository, and the first after a pause is lifted, is at its
  newest landing. A release whose steps wait (exit 75) while its commit's checks go red is taken back.
- **Steps run through a port**, `shipping::release_run::StepRunner`, so the rail tests run the whole release with a
  runner that runs nothing outside (`orch::ReleaseSteps::Given`). The server's runner (`ReleaseSteps::Jailed`,
  `crates/orch/src/release_steps.rs`) runs a step with no connection in the jail with no network, and one with
  connections behind one injector for the step (`Reach::release_way`, `JailRunner::run_release_step` in
  `crates/reach/src/release_step.rs`). The injector serves every connection named in one run
  (`egress::InjectConfig.upstreams`), each upstream with its own rules, headers and scrub; a connection is answered by
  `grant::decide` over the repository's grants: a read it does not allow refuses the step, and writes (every method but
  `CONNECT` and `TRACE`) pass only where it allows an `irreversible` effect, otherwise reads as a script does. A redirect from one connection to another is still
  replaced by a 502.
- **A repository's grant** is asked with `request_grant` naming `repository` (form `tool` only), by a chain that has
  the repository open; the item names the repository as holder, and the Assistant's Apply of an `irreversible` one
  waits out the overturn window as every one does. A connection with no credential yet refuses the step, and the
  release waits, saying which.
- **This server's repository.** Remit's own `.remit/release/` is one step, `10-await-workflow-release`, that reads the
  workflow's releases through `github/default` and reports the tag whose short sha the commit starts with (exit 75
  until it is published), and a `guard` on `fact:remit.health` for an hour. `install_release` runs before the release
  is recorded `baking`, and is recorded as a step: a restart before the release is stored runs the procedure again and
  installs the same version; a refused install waits and runs again, never baking; a release taken back installs
  `previous` only when its install passed. With no guard, it is live once installed. While a release bakes under
  `fact:remit.health`, the fact is recorded every Tick (`remit_facts::health_due`). `fact:remit.released` and the server's poll for
  a new release are gone; `fact:remit.*` is readable by any chain.
- **A run again** (exit 75, a refused install) replaces the steps of the last run and counts it (`Release.tries`), so
  a release that waits for days keeps one run's steps. A pass that leaves a release waiting does not pass again at once.
- **A repository with no procedure** has its release recorded live as soon as one is due (its landings' checks stand),
  and its chains marked live; the clone is fetched once a release, not each pass. **A procedure that does not parse**
  stores the release waiting with the parse error, tells the chains whose landings changed `.remit/release/` (all it
  carries when none did) once per error, and is re-aimed at what lands since (`release::reaim`).
- **The pause** is a record of its own (`model::ReleasePause`, table `release_pauses`, migration 0124), kept once
  lifted with who lifted it and why (`ReleaseLift`) until the next pull replaces it. `pause_releases` and
  `resume_releases` are the Assistant's tools; the owner pulls and lifts it by telling the Assistant. A pause of a
  repository Remit does not know is refused.
- **Tools.** `revert_release` is the Assistant's for any release and a Chair's for a release carrying a landing of its
  chain; like `revert_landing` it acts at once. Only a release that published (baking or live) is taken back.
- **The contract** is slice 2's (`wire::app::RepoView`, `repo_get`, pushed whole as `repo_updated`), with the
  repository's last pause added to the view (`RepoView.pause`) and `repo_views()` for every repository with anything in
  its queues or releases: a release carries its landings (`ReleaseRead.landings`), each naming its chain and title, so
  a release links its newest landing with no lookup of the page's. A landing's read (`LandingRead`) carries the release
  that carried it and, on a revert on its way, the landing it is taking back (`taking_back`, `Landing::taking_back`).
- **The replica holds repositories' views** (`crates/replica/src/repos.rs`): the one a tab opens (`repo_get`, model
  `repo`) and every one the merge queues show (`repo_views`, model `repos`), each replaced whole by `repo_updated` and
  read again after each snapshot; a view pushed while the read is on its way is put over the answer, and an answer
  asked before a snapshot is dropped (`repos/tests.rs`). A view carries no revision: every write of a repository pushes
  its whole view, computed after the write and pushed in write order, so the last push is the newest and pushes applied
  in arrival order over the answer converge on it. The owner sees them on `/queues` and on `/releases?repo=` (a page
  under the Merge queues tab, reached by each queue's "Its releases"): the pause held or lifted, a card per release, and
  the one picked with its steps, its way back, what it bakes under and the landings it carried, each linked to its
  chain's record; and on a landing's record, as "Released in" with the release's state. Neither screen changes a
  release: the owner tells the Assistant.
- **A revert on its way** is said by the server: a chain's landings cross as `wire::app::LandingRead { landing,
  taking_back }` (in `ChainView.landings` and `landing_updated`), where `taking_back` is `Landing::taking_back`, the
  one rule of an open revert that `shipping::landing::reverting` also reads, so the landing it takes back shows "Being
  taken back" with no rule of the page's.
- **The Release seat is deleted**: `ChainKind::Release`, `CAP_RELEASE`, `RELEASE_CHAIR_BRIEF`, `release_seat` and its
  subscription, `list_releasable`, the seat's `mark_live`, `deployer_of`. Migration 0124 finishes any chain of kind
  `release` as a `chore` as a finish in code does (every subscription it holds, its own waits and Remit's watches, ends
  with its firings, near misses and held inbox items, as `delete_subscriptions` ends one; its sensors end, `finishing`
  is cleared, a summary written; its cards, queue and inbox are settled by the next start's recovery), removes the
  capability `release` from every chain, and ends the same way every subscription that starts a release chain and every
  watch on `fact:remit.released`. A seat paused when 0124 runs held releases by its pause; that hold is not carried
  over, since a `release_pauses` row names a repository's catalog id, which SQL cannot know (the owner's database had no
  seat). The store test `the_release_seat_is_finished_and_what_feeds_it_ends_and_running_it_again_changes_nothing`
  runs it on a seat as the build at 123 left one, twice.
- **Rail tests**: a breached release runs back, reverts and releases forward with no turn, and a step's environment
  is the release's variables alone (`crates/orch/tests/app/shipping_release.rs`); no release starts while one bakes
  (that test and `crates/shipping/src/release/tests.rs`); a step writes only through an `irreversible` grant of its
  repository, over stored grants (`shipping_release.rs`, with the injector's rules in
  `crates/egress/src/inject_rules.rs`); a change to `.remit/release/` needs `security` (`shipping_release.rs`), which
  `shipping/src/gate.rs` holds for every repository, so `.remit/rails` does not list it.

Seams left, not built:

- **A step reaches nothing but its granted connections** is held by the jail and the injector, and has no test of its
  own through a real step; the injector's TLS path (`CONNECT` to a connection) is not driven by a test either.
- **A one-way release of this server goes forward**: `remit update` refuses to go back over a release that migrated
  (`crates/remit/src/update.rs`), so the release forward of the reverts is the way back; no test drives it through a
  release.
- **The credential item names the release that waits on it**: a release waiting on a connection with no credential
  says so in its `waits`; no inbox item is raised for it.
- **Per-grant write paths, per-connection budgets, cross-connection redirects, `If-Match`**: the injector's
  hardening seams (`egress::Upstream`).
- **Steps that stream their output while they run**: a step's output is recorded when it ends.

## Revisit when

- **A procedure needs a credential in its environment** (a tool that pins its certificates or signs locally): form T2 for
  that connection, with output redaction, decided per connection by the Assistant at `irreversible`.
- **Releases are taken back often on one repository:** a canary target the procedure declares (`.remit/release/canary`:
  a dist-tag, a preview site, a share of users) released and baked before the full one, through the stage rule.
- **Hardening is asked for:** `land_policy` requiring a stage or a second verdict for `.remit/release/`; per-step
  connections; a daily bound on releases per repository; signed release records; health readings agents cannot push
  (ADR-0087's list carries over); the session's own OS user (K2) so no session reaches the injector's credentials.
- **Instances see each other** (ADR-0072's node identity): this server's install waits for the release to bake on
  staging, as a step of the same procedure.
- **A release kind needs judgement no evidence carries** (a launch timed to a campaign): the landing's stage plan grows a
  `not_before`, set by the chain in its change; still no agent at release time.
