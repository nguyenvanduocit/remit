# 0050. A chain merges its own change in canary stages, through one door

Status: superseded by ADR-0114 (every approved change lands through Remit's queue; `merge_mr` and `ship` go; the stage rule carries onto a landing), 2026-11-19; accepted, 2026-11-03; `merge_mr` also merges another chain's MR for a Release seat (ADR-0087). Supersedes the merge rail of ADR-0013 (*Ownership is not authority*) for a chain that holds the `ship`
capability; ADR-0012's rule that only the owner merges a self-development chain's MR is superseded by ADR-0087, whose Release
seat merges it. Source: the
owner's words of 2026-11-03: chains do not dare merge or deploy, which is a poor way to work; let them merge and deploy, and make the
control a canary (setup, parallel running, a flag, monitoring), and try it as a playbook.

## Context

A chain ended at an MR the owner merged (design.md, *Merging*): the forge login could not merge, the deny rules refused `glab mr
merge`, `gh pr merge` and the API calls that name them, and ADR-0013 listed the merge among the rails no agent lifts. The cost shows in
every chain: it finishes unverified or waits on the owner for the one step that makes its work real, and the agents that know the change
best cannot watch it reach users. The rail guarded against a bad change reaching users. A wall in front of every change guards that
badly: it adds a wait to the safe change and does nothing to the one that was reviewed wrongly. What bounds a bad change is how many
users it reaches and how fast it is taken back.

## Options

1. Keep the merge the owner's. The owner stays the bottleneck of every chain, and the effect is verified late or never.
2. Merge when review and CI pass (auto-merge). A change reaches everyone at once; nothing watches it, nothing takes it back.
3. A deploy tool that moves traffic (weights on a load balancer, a rollout controller). Right where it exists, but every repository
   deploys differently, so Remit would own a deployer per platform before the first chain could ship.
4. A `ship` capability and one door, `merge_mr`, over a pure stage rule; the canary is a flag in the repository, every stage is a
   merge, and the way back is a merge. **Chosen.** It works for every repository whose pipeline deploys on merge, uses the forge,
   the signals and the watches that exist, and leaves the flag and the deploy to be served by a source later.

## Decision

Option 4.

- **`ship` is a writing capability** (`capabilities.go`): the owner applies it to a playbook once, as any writer, and that is the
  leave to merge. A chain that a proposal, an event or a client started cannot hold it (`mayNotHave`). The built-in playbook
  `canary-release` lists it, with `signals` (to watch) and `observe` (to read the running system); `self-development` does not.
- **`merge_mr` is the one door that merges** (`internal/orch/ship.go`, the Chair's tool). The deny rules of `policy.go` stay: a shell
  merge is refused, so every merge by a chain passes the rule below and is recorded with who made it.
- **A stage is a plan and a merge.** The call says who the merge reaches (`exposure`), the guardrail (a signal key and the condition that
  means it hurts), the bake, and how the stage is taken back. The first stage of a change merges dark: behind a flag the repository
  reads at run time, with the old path still the default and running (parallel running). Each later stage is a small MR that grows the
  flag, merged the same way. The last MR removes the old path and the flag.
- **The rule is pure** (`internal/rollout`, `Vet`): a plan names its exposure, rollback, guardrail and bake; the stage before has baked
  (`BakeUntil` past); every guardrail, the previous stage's too, reads and does not breach now. The orchestrator hands it the previous
  stage (the reading `chain:<id>.stage`, a signal source of its own) and a probe of the guardrail on its newest reading
  (`signal.Probe`, ADR-0045): a signal with no reading guards nothing, so the stage is refused.
- **The way back is a merge that sets `undo`** and passes at once: while a stage bakes and while a guardrail is breached, which is when
  it is wanted. After an undo the next stage starts over.
- **The merge is of the judged head.** The worktree must be at the commit Remit pushed, the reviewer's and QC's latest verdicts
  must approve that change (the submit's check, `missingApprovals`), and the forge is asked to merge that commit's sha, so a push
  after the verdicts is refused by the forge (`forge.Merge`, GitLab and GitHub; a draft is made ready first).
- **The watch is part of the merge.** After the forge merges, `merge_mr` records the stage and a decision of kind `ship`, then
  subscribes the Chair to the guardrail until the bake ends (`watchOf` is run before the merge, so a stage that cannot be watched
  is never merged). A breach wakes the Chair, whose brief says: take it back first, understand it second.
- **The operator's side.** The forge login Remit works as holds merge rights on each repository a `ship` playbook may touch
  (design.md, *Merging*). Its branch protection and required checks stay the forge's: the forge refuses a merge its rules refuse.
- **Rails that stay code:** authentication, pause and stop (a paused or stopped chain merges nothing), the account sign-in, and the
  owner's own merge on the forge. Of the rails ADR-0013 named, the merge is now a door with a rule; the others are unchanged.

## Threats

A rail changed: an agent now holds the act that puts code in front of users. Threat modelling (`aio-threat-models`) runs when the owner
asks for hardening; the controls below are those built, and the seam for each one not built.

| Threat | Control |
|---|---|
| A chain merges an unreviewed change (tampering) | The head merged is the head pushed, `approvedHead` checks both verdicts on it, and the forge merges only that sha |
| A chain merges to everyone at once (elevation) | A stage needs its plan, a baked predecessor and a guardrail that reads and holds; only the playbook's brief says the first stage is dark (a reviewer judges it) |
| A guardrail that cannot fail (Goodhart) | The condition is run on a real reading before the merge; a signal with no reading is refused; the reviewer's brief judges that it reads the harm the change could do |
| Text in a signal or a log steers the Chair to merge or to skip a bake (OWASP LLM01) | A reading is evidence; the bake and the guardrail are code and no argument moves them |
| A merge by shell or raw API call around the door | The deny rules stay; the forge's own protection is the wall the operator keeps (design.md, *Merging*) |
| A merge that cannot be taken back | The way back is a stage's required field, written before it merges; an undo is never refused |

## Consequences

- A chain owns its change from the first line to everyone having it, and finishes on a verdict over the window after its last stage.
- `Forge` gains `Merge`; `Decision` gains the kind `ship`; the signal board gains the source `stage`; `ship` joins the catalog.
- The bake counts from the merge, not from the deploy: `chain:<id>.live` is still the owner's or an event's word. The plan's bake
  covers the pipeline's time, and the brief says to confirm the deploy by a signal.
- A rollback by flag needs no merge only when a source serves the flag; until then it is an MR that passes review and QC like any, which
  is the slowest part of a bad hour.

## Revisit when

- **A flag source** (a plugin whose write is `reversible`, ADR-0027) exists: a stage then grows and shrinks by a call, not by a merge,
  and the way back is seconds. The rule keeps its shape; `Vet` is the seam.
- **A breach must not wait for a Chair's turn:** a subscription action `undo` that merges a prepared way back by code when the guardrail
  breaches (the same seam as the `mark-live` job).
- **The deploy is observable:** a deploy event signal marks the stage live and the bake counts from it.
- **Hardening is asked for:** a per-repository allow list of what a chain may merge to, a daily cap on merges, the owner's list of
  protected paths no chain merges (migrations, workflows), a second approver for a repository he marks.

Enforced by: TestAStageMergesThePushedHeadAndLeavesAWatchOnItsGuardrail, TestAMergeThatTheRuleRefusesNeverReachesTheForge, TestTheNextStageWaitsForTheBakeAndTheWayBackDoesNot, TestOnlyAChainThatHoldsShipHasMergeMR, TestAStageMustSayWhoItReachesHowItIsGuardedAndHowItIsTakenBack, TestTheNextStageWaitsForThePreviousOneToBake, TestABreachOrAGuardrailThatCannotBeReadStopsTheStage, TestAWayBackMergesWhileTheStageBakesAndItsGuardrailIsBreached, TestGitHubMergesAPullRequestAtTheHeadAskedForAfterMakingADraftReady, TestGitLabMergesAMergeRequestAtTheHeadAskedForAfterMakingADraftReady
