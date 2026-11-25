# 0114. Remit lands every change itself, and a breach is reverted

Status: accepted, 2026-11-19; built 2026-11-19 (*As built*, below); the Release seat it kept superseded by ADR-0116, built 2026-11-19 (what lands is released by its repository's procedure, in code, `crates/shipping/src/release_run.rs`). Supersedes ADR-0050's `merge_mr` and the `ship` capability (the stage rule, a
plan with exposure, guardrail, bake and way back judged by `rollout::vet`, carries onto a landing), ADR-0057's
merge-request sense (its pattern, a bundled plugin's sensor that reads the forge through a read-only host service, carries
the checks of a landed commit), ADR-0068's headline (owner hours per confirmed result: no MR waits for him), ADR-0087's
merging by the Release seat (the seat installs, watches and takes back; the queue lands), ADR-0055's MR readings
(`chain:<id>.mr.<repository>` becomes `chain:<id>.landing.<repository>`) and ADR-0020's merge requests on a forge (the
forge hosts the code). Amends ADR-0039 (`fact:chain.merged` becomes `fact:chain.landed`), ADR-0078 (`waiting::merging`
goes), ADR-0080 (an MR's next steps go with the MR) and ADR-0110 (the landing reads the kind's gates where `submit_mr` and
`merge_mr` did). Source: the owner, 2026-11-19: "I don't like MRs, especially draft MRs; I want us to be able to merge
straight away", "don't be bound by what exists today", and the aim: "fully automate a multi-agent system: very many agents
together reaching goals instead of tasks; our job is to create as many conditions as possible for AI". The owner approved
the direction (Remit is the integrator, a queue it owns, automatic revert, a `Landing` port for a forge's MR) the same day.

## Context

The customer is the person a change is for; what he gets is the change working on the main branch, soon, and a main branch
that stays green while many agents change it at once.

Today every change ends as a draft MR (`shipping::submit::submit_mr` opens it, `model::MRRef` on `RepoWork.mr` records
it) that an agent merges through the forge: the chain's Chair with `ship` (`merge_mr`, ADR-0050), or the Release seat on
the instance's own repository (ADR-0087). The MR does four jobs, and Remit already does three of them itself:

| Job of the MR | Who does it already |
|---|---|
| Hold the verdicts on a version | Remit: reviewer, QC and security verdicts on the change's stable patch id (`shipping::gate::patch_id`, `MRRef.change`, `MRRef.gates`) |
| Run the checks | Remit: the language's checks and `.remit/checks` in a jail at submit (ADR-0025); the forge's CI also runs on the MR (`MRRef.checks`) |
| Put the change on the base | the forge, asked by `merge_mr` (`chains::forges::Forge::merge`) |
| Show the diff and take comments | the forge (`Forge::diff`, `shipping::threads`), mirrored in Remit |

No agent pushes: the deny rules refuse `git push` in every session (`crates/chains/src/policy.rs:60`), and only Remit's
backend pushes. So Remit already holds the verdicts, the checks and the only hand that writes to the remote. The MR adds a
wait state, a second record of the change's standing that Remit polls every few minutes (`shipping::merge_request_sense`),
a forge-specific merge path in eight places, and a way back that is a whole chain: a breach on the instance's own
repository makes the Release seat propose a chain that writes the revert, is judged and merged (`capability::briefs`,
`RELEASE_CHAIR_BRIEF` step 8), which ADR-0050 itself called the slowest part of a bad hour.

What the MR never did is test the combined result. A change is judged and checked against the base it started from; two
changes that each pass can break the base together. The owner's own measurements in concord
(`docs/findings.md`, 2026-11-18) ran three colliding agents through every setup: plain
git worktrees, git with Mergiraf, jj, jj with Mergiraf, GitButler and coordination without a gate all landed broken
code; the combined-test gate before landing was the only layer that caught every semantic bug (a renamed function still
called by its old name, an int that became a float). The research behind this direction (`reports/Đa agent hướng kết
quả.md`) adds that agents run independently amplify errors 17.2 times against 4.4 times with a verifying orchestrator
(Google Research and MIT, arXiv 2512.08296), and that Cursor, at hundreds of agents, accepted a small stable error rate
with a fixer pass rather than demand that every commit be right, because demanding it serialised the work
(cursor.com/blog/self-driving-codebases).

Mental models used: **Bottlenecks** (Goldratt: the integration point is the system's constraint, so it is designed first
and everything upstream is paced to it), **Inversion** (what would guarantee a broken base: landing a change tested only
against an old base, and a way back that needs a turn of an agent), **Second-order thinking** (six months on, with a
hundred chains a day, what does the MR make harder: a forge call per change, a poll per open MR, a wait per merge).
Patterns: Queue-Based Load Leveling and Priority Queue (vol. 3), Safe Hands-Off Deployments (vol. 4: automated
rollback, the release train that bounds a batch), Hexagonal ports (vol. 2) for the seam to a forge's MR.

## Options

1. **Keep MRs; let the forge merge them** (GitHub's merge queue, GitLab's merge trains, auto-merge on green). The forge
   tests the combined result, but each forge does it differently (GitLab's trains are a paid tier), the MR stays the
   record the owner dislikes, Remit keeps polling a second source of truth, and a revert is still an MR. Rejected: it
   keeps every cost of the MR and makes the integrator a forge feature Remit cannot change.
2. **Push each approved change straight to the base, test after, revert what breaks** (Cursor's trade). Fastest, no queue.
   The concord measurement is the counter: without a combined gate every setup landed broken code, and every chain that
   starts from a broken base inherits it. Rejected as the default; kept as the direction the queue can move toward when
   its wait is measured as the constraint (*Revisit when*).
3. **Adopt concord as the integrator** (its crate as a library, or its CLI protocol in every worktree). Its gate is the
   right idea, but it is built on jj workspaces (a jj fork in process, colocated jj state in every clone), a board in a
   JSON file under `flock` beside Remit's store (two owners of who works on what), function claims behind a plan barrier
   (which would serialise chains that live for days), and Mergiraf, which on ConflictBench resolved 27 of 159 cases right
   and 26 wrong (`concord/docs/findings.md`, *Public benchmarks*): a wrong merge lands silently unless a test catches it.
   Rejected as a dependency; its findings are taken as rules (below).
4. **Remit is the integrator: a landing queue per repository and base branch, owned by Remit's code, that combines each
   approved change with the current base, runs the checks on the combined tree, pushes the base forward and records a
   `Landing`; the forge's CI on the landed commit and the change's guardrails are read after, and a breach is reverted by
   code.** The forge hosts the code. **Chosen.**

## Decision

Option 4.

### What a change is, and how it enters the queue

- A chain changes a repository in its worktree as today. Its Chair submits a version with `land` (replacing `submit_mr`):
  the repository, `priority` and `priority_why` as today, and an optional stage plan (below). Remit runs the language's
  checks and `.remit/checks` on the worktree's head in a jail (ADR-0025), as the submit does now, records the version
  (its worktree commit and its patch id against the base it forked from) and asks for the gates the kind names
  (`ChainKind::gates()`, security added for a rail, `required_gates`, ADR-0110). Nothing is pushed and nothing is opened
  on the forge.
- A version whose every required gate approves its patch id enters the queue by itself: no second call, no capability, no
  agent who merges. A verdict stays tied to the patch id, which a clean combination with a newer base does not change; a
  version whose patch changes (the Chair merged the base in and resolved a conflict) is a new version and is judged again.
- `ship` and `merge_mr` go: landing needs no leave, because what made a merge dangerous (an unreviewed, unwatched change
  reaching everyone, with a slow way back) is answered by the gates, the combined checks and the automatic revert for
  every change, not by who may press a button.

### The queue

- One queue per repository and base branch, run by one writer in Remit's code (the pattern of one writer per chain). It is
  a pure core (`shipping::landing`: which entries form the next batch, how a failed batch is split, what a revert must take
  with it) and an imperative shell (a landing worktree of its own per repository, the jail, the push).
- **Order.** Reverts first; then entries by the time they became ready, oldest first. The Chair's priority orders what
  the owner reads, not the queue: a high-priority change that waits behind three small ones waits minutes.
- **A batch.** The shell takes the next `k` ready entries, applies each onto the current base in order as one commit of its
  own (below), and runs the checks once on the top. `k` is measured, not set: it grows by one after a batch that passes
  and halves after one that fails (additive increase, multiplicative decrease), from 1; a batch never spans a stage that
  bakes (below).
- **Combining.** Each entry's change (its worktree diff from its merge base) is applied onto the batch's tree with git's
  three-way merge. A textual conflict is never resolved by the queue: the entry leaves the batch and goes back to its
  Chair with the conflicting paths and the landing that touched them, the version the Chair resubmits is judged again. An
  entry whose combined diff is empty fails as lost work (concord's empty-change check).
- **The checks on the combined tree.** The same checks the submit ran, on the batch's top commit, in the jail. They pass:
  the batch is pushed. They fail: the batch is split in two and each half is tried again (bisection), until the entry
  that fails is found alone; it goes back to its Chair with the check's output, and the rest land.
- **The push.** A fast-forward of the base to the batch's top (`git push <remote> <sha>:refs/heads/<base>`, never forced).
  A refused push means the base moved outside the queue (the owner, another instance, a forge bot): the batch is combined
  again onto the new base and checked again. The base's history only grows: nothing on it is ever rewritten, and a way
  back is a new commit.
- **One commit per landing.** Each entry becomes one commit on the base whose tree is what the checks passed, authored by
  the account Remit pushes as, with the employees who made it as `Co-authored-by:` trailers and trailers naming the chain,
  the key result it serves and the verdicts (`Remit-Chain:`, `Remit-Serves:`, `Remit-Verdicts:`). One landing, one commit,
  one revert; a change reverted and fixed lands again as a new commit, which a merged branch could not (git would take
  its commits as merged already). After a landing the chain's worktree takes the new base by a merge, so its later work
  is diffed against what landed and its own history only grows.
- **The worktree is not the remote.** Remit no longer pushes a chain's branch to the forge. `push_ref`, `pushable` and the
  rewrite of commits onto `refs/remit/push/<branch>` go: the landing commit is made by Remit and is the pusher's own.

### What the base moving tells others

A landing records `fact:repo.landed` (repository, base, commit, chain, paths changed). Every chain with a worktree on that
repository whose own changed paths overlap is told once, by a message to its Chair, which landing moved the base and what
it touched, so it merges the base in before it submits again. This is concord's contract notice (its `finish` exit 10),
the layer its findings show prevents the stale-contract bug rather than only catching it; a function-level claim board
is not built (*Revisit when*).

### After the push

- **The forge's CI on the landed commit.** The forge runs the repository's own pipeline on the push. A bundled plugin's
  sensor (kind `landing`, replacing `merge-request`) reads the checks of each landed commit through a read-only host
  service the host lends for the call (`Forge.Checks`, replacing `Forge.Standing`; the forge's login never leaves the
  server, ADR-0057's pattern). A chain cannot change the pipeline (`forbidden_path` refuses `.github/workflows/` and
  `.gitlab-ci.yml`), so this check is one no agent writes.
- **A breach is reverted by code.** A push is breached when the forge reports the checks on the commit it pushed failed
  while those on the base before it passed, and a landing is breached when a stage's guardrail breaches during its bake
  (`rollout::vet`'s guardrail, read by the watch the landing set). Remit then queues the revert at the front of the queue:
  of the landing whose guardrail breached, or of every landing the breached push carried (a forge's pipeline runs on the
  pushed head, so a batch is one result). A revert needs no gate (it puts back a state that was judged), passes the
  combined checks like any batch, and lands. The entries of a reverted batch re-enter the queue as ready, the batch size
  halves, and the one whose push breaks again alone is returned to its Chair with the pipeline's output. A revert that conflicts takes
  with it, newest first, every later landing it conflicts with; each of their chains is told why. The landing's chain is
  told and its Chair woken to understand and land a fix. While a breach on a repository is being reverted the queue lands
  nothing else there.
- **The owner overturns by telling an agent.** "Take back X" to the Assistant, or to the chain's Chair on its screen, is
  `revert_landing` (the Assistant's for any landing, a Chair's for its chain's), the same path the breach takes, recorded
  with who asked and why. A revert is reversible (the change can land again), so it acts at once (ADR-0101 holds only the
  irreversible).

### Stages

`merge_mr`'s stage rule moves onto `land` whole: a version may carry a plan (exposure, guardrail key and condition, bake,
way back); `rollout::vet` refuses it unless the stage before on that repository baked and every guardrail reads and holds;
the landing sets the watch in the same act; a stage that cannot be watched never lands. A plan is optional: a change with
none lands to everyone the base reaches, guarded by the forge's CI and the gates. A stage needs `signals` to watch.

### The record

- `model::Landing`, an entity of its own (table `landings`, its notification, carried in `ChainView` as `Decision` is),
  because a chain lands without bound: id, chain, repository, base, version (worktree commit, patch id, gates and the
  verdict ids that approved it, `by` employees), priority and why, stage plan, state (`ready`, `combining`, `checking`,
  `landed`, `returned` with the reason and the output, `reverted` with the revert's landing id, who and why), batch, the
  landed commit, the forge's checks on it, and the times it became ready, landed and was reverted.
- `MRRef`, `MRState`, `MRChecks`, `RepoWork.mr`, `RepoWork.earlier_mrs`, `RepoWork.pushed_sha` and `pushed_from` go;
  `RepoWork` keeps where the chain works (repository, project, host, worktree, branch, base).
- Readings: `chain:<id>.landing.<repository>` replaces `chain:<id>.mr.<repository>` (ADR-0055): state, version, checks,
  latest verdicts and who made it, one reading whenever any moves. `fact:chain.landed` replaces `fact:chain.merged`, and
  the goal judge's recheck after a merge (ADR-0039) answers it.
- Every place bound to an MR reads a landing instead: the board's column and attention ranking (`orch::standing`), the
  archive lifecycle, the event matching of a forge's webhook to a chain (by commit, not MR number), the goal judge's
  brief, `waiting::merging` (which goes), admission's merge queue (replaced by the queue's measures, ADR-0115) and the
  Outcomes KPIs (below).

### What the owner sees

- A chain's screen lists its landings, live: each version's state, its diff (Remit's own, from its clone: the forge is not
  asked), its verdicts, the batch and checks it passed or the output it was returned with, the landed commit, the forge's
  checks on it, and a revert with who and why. He comments by writing to the Chair there; the forge's review threads
  (`shipping::threads`) go.
- Each repository's queue is a view the server derives (ready, the batch being checked, landed in the last day, returned,
  reverted), opened from the repository or the chain, in the order the queue will take them.
- No landing waits for him; nothing in the inbox asks him to land.

### Outcomes

The KPIs that read an MR's wait for the owner go (owner hours per confirmed result, merged without a review comment,
average hours an MR he decided waited, MRs with no decision). The headline becomes **landings taken back per confirmed
result**: reverts by CI, by guardrail and by the owner's overturn, over the results the goal judge confirmed, with results
confirmed beside it as its constraint, because a revert is the defect that reached the base and an overturn is the one
that reached him. Beside it: time from ready to landed, the share of entries returned for a conflict or a failed combined
check, and batch size. These are the measures ADR-0115 schedules by.

### The seam to a forge's MR

`Landing` is the port the queue's shell lands through (`shipping::landing::Landing`, declared by the crate that consumes
it): it takes a checked batch and the base it was combined on, and answers the commits that landed or why not. Remit
builds one adapter, `DirectPush`. A team whose forge requires a merge request for the base (branch protection Remit's
account cannot bypass) would get a `ForgeMerge` adapter that opens the MR for the batch's top, lets the forge merge it
and answers when it did; it is not built.

### Stored data and the cut-over

One numbered migration (the next free number when this lands), each statement idempotent: each `RepoWork.mr` whose state
is merged becomes a `landed` landing (its MR's URL kept as its reference, its head as its commit); each open one whose
version's gates approve becomes a `ready` landing at its pushed-from commit; an open one not approved becomes the
version waiting for its gates; a closed one is dropped from the chain and kept in its decisions. A one-time job after the
migration closes on the forge the draft MRs it carried over, with a comment naming the landing; it runs once per MR
(idempotent by the MR's state); the forge call it needs to close an MR is written for it and deleted with it.

## Threats

A rail changed: the act that puts code on the base is now Remit's code for every change, and the account Remit pushes as
needs the right to push to the base. Threat modelling (`aio-threat-models`) runs when the owner asks for hardening; the
controls below are those built, and the seam for each one not built.

| Threat | Control | Seam not built |
|---|---|---|
| An unjudged or tampered change lands | The queue takes only a version whose patch id carries every required gate's approval; the commit it lands is the tree the checks passed; agents cannot push (`policy.rs` deny rules) | A signature over the verdicts in the commit trailer |
| A change started by outside text (an event, a proposal) lands to everyone | The same gates on the strongest model (ADR-0102), security on a rail, combined checks, the forge's CI, the automatic revert; a client's chain submits nothing (`shipping::submit`, origin `client`) | `land_policy(origin, kind, paths)`, the one pure function that adds gates or requires a stage: by origin, by protected paths the owner names (migrations, workflows), by repository |
| Two changes that pass alone break the base together | The combined checks before the push; bisection finds the one; the overlap notice tells the others | A function-level claim board (concord's), if the conflict rate says chains collide |
| A wrong automatic merge lands silently | The queue never resolves a conflict; the author's chain does, and its new version is judged again | A structural merge driver, if its wrong rate on Remit's own conflicts is measured below its right rate |
| A flaky pipeline reverts a good change | A breach needs the pushed commit red while the base before it was green; a reverted change lands again by itself | A re-run of the failed jobs before reverting |
| The push account is used outside the queue | The base's protection stays the forge's: the operator gives Remit's account the right to push to the base (a bypass in GitHub's rulesets, an allowed role in GitLab's protected branches) and nothing else; sessions still share the server's OS user (`docs/remit-design.md` K2), so a session that reads the machine's forge login could push | The session's own OS user (K2) |
| A crash between combining and recording | The landing is recorded before the push, the push is a fast-forward to a known commit, and on restart a base already at that commit reads as landed: run twice, the same state | |
| A revert that cannot be made | A revert is a commit; one that conflicts takes the later landings it conflicts with; the base returns to its last judged state | |

## Consequences

- A change reaches the base minutes after its verdicts, not after a merge someone schedules; a breach is taken back by code
  in the time a revert takes to pass the checks, not by a chain.
- The forge is where code is hosted and its pipeline runs. `chains::forges::Forge` shrinks to who Remit works as
  (`account`), the base's head (`branch_head`) and a commit's checks; `open_mr`, `open_mr_of`, `update_mr`, `retarget`,
  `merge`, `standing` and `diff` go, with the MR halves of `crates/forge`, `shipping::merge_request_sense`,
  `shipping::threads`, `shipping::ship::merge_mr`, `model::mr_checks` and the `merge-request` sensor kind of the bundled
  `forge` plugin. An outside forge (`docs/remit-design.md` K5) becomes three read calls and a push, which lowers its bar.
- The Release seat (ADR-0087) no longer merges: the queue lands on the instance's repository like any other. The seat
  installs the newest release the workflow published once the one before baked, watches `fact:remit.health`, and on a
  breach installs `previous`; code reverts every landing in the breached release. Its brief loses steps (2) to (5) and
  the revert chain of (8).
- A repository whose base the forge protects against Remit's account cannot be worked on until the operator gives the
  account the right to push, or the `ForgeMerge` adapter exists. The install guide says so beside the forge's login.
- Every chain's work lands, so the main path of a chain changes: its rail tests (the gates, a version that changed being
  judged again, the combined check, the push that never forces, the automatic revert, the migration of stored MRs) are the
  tests the change that builds this adds; none is named before it exists.
- Not decided here: the queue's measures as the back-pressure on what starts (ADR-0115).

## As built

Where the build differs from the decision above, or decides what it left open:

- **The port is `shipping::landing::Lander`**, not `Landing`: `model::Landing` is the record, and one name for two
  concepts would read wrong at every call site. `DirectPush` (`shipping/src/direct_push.rs`) is its one adapter;
  `ForgeMerge` stays an unbuilt seam.
- **States.** `judging` comes first: a version recorded by `land` waits there for its gates, and one rule
  (`shipping::land::approve`) makes it `ready` at `land`, on each verdict, and in `sync_landings` for a version whose
  gates approved it already (one migration 0119 carried over with its verdicts). A batch is recorded `combining` before
  it is combined and `checking`, with its commits and push, before its checks run, so a restart puts a `combining`
  landing back to `ready` (nothing of it reached the forge) and settles a `checking` one as `landed` when the base holds
  its push, else back to `ready`.
- **Bisection is the batch size.** A red batch of more than one goes back to `ready` and `k` halves (`halved`), so the
  landing that broke it is found alone and returned to its Chair (`ReturnReason::Checks`); no explicit split is kept.
- **The combined checks are skipped for one landing alone on the base it forked from** (`forked == onto`, not a
  revert): its tree is the one `land` checked.
- **One commit per landing (squash)**, made with `git merge-tree --write-tree` and `commit-tree` as the account the
  forge works as (`repo::pusher_on`), with the trailers `Remit-Chain`, `Remit-Serves`, `Remit-Verdicts`,
  `Remit-Reverts` and `Remit-Landing`. After a landing, the chain's worktree takes the landed commit by a merge only when
  its HEAD is still the version that landed and its tree is clean (`bring_up`); otherwise its Chair merges the base in.
- **A refused push** goes back to its Chair with the forge's words (`ReturnReason::Refused`) and raises an item for the
  Assistant: a protected base is the operator's to open, and `ForgeMerge` is the seam for a forge that never will.
- **The forge's CI runs after the push.** A push is reverted when its checks failed and the base's before it passed
  (`landing::verdict_on`); a push whose base has no reading yet, or was red, stands, and the bundled rules tell the
  Chair of a failed pipeline. A landing is followed for a day after it landed (`FOLLOW_FOR`).
- **A guardrail breach is found by polling.** `sync_landings` runs every 3 minutes (`LANDING_SYNC_EVERY`) and on each
  finished pipeline the forge posts (`sync-landing`); it reverts each stage whose guardrail breached during its bake. The
  watch the landing sets still wakes the Chair.
- **Reverts need no verdict.** A revert carries what it takes back (`Landing.reverts`: the target, the cause, who asked
  and why); the target stays `landed` until the revert lands, then reads `reverted` with the same words
  (`landing::taken_back`), and a landing being taken back is the one an open or landed revert names (`reverting`).
  `revert_landing` is the Assistant's for any landing and a Chair's for its own chain's.
- **What the queue records is caused by Remit in no chain** (`queue::queue_cause`), so a chain's own watches on its
  landings hear the queue; a revert a chain's own agent asks for is caused by that chain.
- **A revert that conflicts** takes back first only the landings after its target whose paths the conflict names.
- **Finishing waits for the queue:** `finish_chain` is refused while a landing of the chain is `judging`, `ready` or
  `checking`.
- **Deleting a chain withdraws what of it has not landed.** Deleting a chain is the owner, or the Assistant for him,
  taking that work back: in the transaction that deletes it, each of its landings still `judging`, `ready` or
  `combining` is returned with `ReturnReason::Withdrawn` (`chains::commit::withdrawn`, the one path that writes it) and
  never lands. One the queue is `checking` refuses the delete until it has landed or come back, because its push may be
  under way and the record would otherwise say withdrawn of a change on the base. A landed landing stays as it is, and
  an open revert still lands: a revert is how a landed change goes. A `combining` landing withdrawn mid-pass makes the
  queue's next write conflict, so the pass stops and the next one recombines without it. No path removes a repository
  from a chain (`RepoWork` is only added or edited), so a chain leaving a repository has nothing to withdraw today; a
  path that adds it withdraws there by the same rule.
- **The Release seat** this record kept is gone with ADR-0116's build: a release of the repository, run by code,
  installs on this server as its last step, and a release taken back reverts every landing it carried (`Landing.release`
  names them, so no tag ancestry is read).
- **A rail's check** (`check_rail`) runs at a stage (`CheckDoor::AtStage`): a chain ships when one of its landings
  carries a plan.
- **Stored data: migrations 0119 and 0123.** The released 0119 made the tables and carried each MR over; 0123 makes
  `landings` again with the queue's generated columns (host, path, base, state), gives each carried landing its forge
  host and path, its version's number, title and body and its time of change, keeps the open draft MRs it carried in
  `carried_mrs`, and takes the MR fields, `ship`, the review threads and the MR sense from the rest. A merged MR becomes a `landed` landing, an open one a `judging` landing at the
  commit it was pushed from (the sweep above takes it on when its gates approved it), a closed one is dropped and kept in
  its decisions; review threads and the comment and reply drafts go, a message draft keeps its words; the
  `merge-request` sensors and the MR-sync subscriptions go, and watches on `chain:<id>.mr.*` and `fact:chain.merged`
  move to their landing names. Stage readings are not carried over: a stage that was baking when the migration ran
  bakes no more. The draft MRs carried over (`carried_mrs`) are closed once on their forge by `close_carried_mrs`,
  run in the background at start; its tests use a fake forge only. Landings outlive their chain.
- **Admission reads the queue (ADR-0115).** `-max-waiting-mrs` and `admit::merge_queue` go: a key result's share grows
  by one for each landing of its work that holds and halves for each reverted or returned for a conflict or a failed
  combined check (`admit::fates`), a key result whose landings waiting in the queues number its share takes no new start
  (`admit::waiting`), and `admit::landed` reads a landing on its base, so a reverted one is no progress.
- **What the owner sees.** `ChainView.landings` (`LandingRead`, pushed by `landing_updated`) and `landing_diff` show a
  chain's landings; a repository's queues and releases are its `RepoView` (orch's `repo_view.rs`, ADR-0116), read by
  `repo_get` and `repo_views` and pushed whole as `repo_updated`.

## Revisit when

- **The queue is the constraint:** the time from ready to landed, measured, exceeds the time a change takes to judge, with
  batches already large. The seam is the queue's core: land first and check after (option 2) for chains whose kind or
  repository the measures show rarely break the base, with the revert as the net.
- **Reverts by the forge's CI** exceed a few percent of landings on a repository (its pipeline tests what Remit's checks do
  not): run the pipeline on the batch before the push, on a ref of Remit's (`refs/heads/remit/landing`), in the shell's
  check step.
- **A team requires merge requests:** build `ForgeMerge` behind `Landing`.
- **Chains on one repository conflict often** (returned for a conflict above the rate they land): a claim board of what
  each chain changes, concord's barrier and notices, kept in Remit's store; or a structural merge driver measured on
  Remit's own conflicts.
- **Hardening is asked for:** `land_policy` by origin and protected paths, a signed verdict trailer, the session's own OS
  user so no session can reach the push account, a re-run before a revert, a daily bound on landings per repository.
