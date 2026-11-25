# 0067. A finished chain is Done, Waiting on you, or Not done

Status: accepted, 2026-11-04; the word for its second ending narrowed by 0078; its waits on the owner, but a secret act, are an agent's (ADR-0087). Source: the owner's words of 2026-11-04 on chain `ch_79e7722eff5f` ("it can't be both closed and
unverified … there must be a better gate … let fix it"), `docs/design.md` (How results are checked), ADR-0013, ADR-0016, ADR-0039.
Amends ADR-0013's closing words ("Confirmed" and "Closed unverified") and its expiry rule; extends ADR-0039 to the live mark.
Read against commit `efd4abb`.

## Context

The owner asks for a result and expects it done. A finished chain answers with one of two words today, computed by `closing`
(`internal/orch/standing.go:250-272`): **Confirmed** when the goal judge's last verdict approves and every finding is met
(`Approval.Confirms`, `internal/model/model.go:197-198`), else **Closed unverified**, with the blockers its findings name. The second
word says neither "done" nor "not done", and the owner cannot act on it.

What the code does, read at `efd4abb`:

- **The finish already waits for the judge.** `finish_chain` stores the summary in `Finishing` and hires the judge
  (`RequestFinish`, `internal/orch/goal.go:421-469`; `requestGoalCheck`, `:480-492`). The chain stays `running` until
  `endGoalCheckCmd` acts on the verdict (`internal/orch/step.go:359-394`). The tool tells the Chair "Checking the result against the
  goal…" (`internal/orch/tools.go:748-749`). In `ch_79e7722eff5f` the words "I've closed this chain; a final check is running" were the
  Chair's own wording, not a close.
- **The gap is what counts as passing.** The judge may `approve` with findings `unverified` (`verdictFindingsError`,
  `internal/orch/gate.go:146-166`: "approve means every key result is met or cannot be checked by the team"), and an approve
  finishes the chain (`step.go:384-386`). The judge's brief asks for this: "A claim of the summary you could not reach (a stored record,
  …) is unverified with the blocker access" (`goal.go:404`). So "closed but not proven" is a passing grade.
- **Four other paths close a chain with no ruling, or against one:** a judge that fails, stops or times out (`step.go:381-383`,
  `internal/orch/session.go:550-555`); the attempt limit, where the fourth finish skips the judge (`goal.go:465-467`,
  `maxGoalChecks = 3`, `goal.go:31`), as in `ch_ba6759a6ab8a`; the owner's own Finish; and goal checks off or a verification chain
  (`goal.go:458-460`). All of them read "Closed unverified".
- **The judge cannot read Remit's own store.** Its MCP tools are `record_verdict`, `read_check`, `addJudgeReadTools` (sources and
  `read_objective`, `goal.go:338-341`) and the verification tool, and it returns before any capability's tools (`tools.go:519-521`).
  The readers of playbooks, agent definitions and the library (`addPlaybookReadTools`, `addAgentReadTools`, `addLibraryReadTools`)
  reach a Chair only (`capabilities.go:196`, `:236`, `:240-255`; `tools.go:552-553`) and the Assistant (`assistant_tools.go:82-84`).
  Bash is withheld from every card of a chain without `shell` (`withheldTools`, `internal/orch/policy.go:128-139`), so the judge of a
  Playbook author chain has no Bash either. The judge of `ch_79e7722eff5f`, which created a playbook and five agents, said so: "Bash is
  disabled for me … I did not query the playbook store directly." It approved with the store unread.
- **A precedent exists:** a chain that reads data with source tools cannot be judged `unverified` for `access`, because the judge
  holds the same tools (`gate.go:153-157`, `goal.go:306-312`).
- **What follows a merge is re-checked; what follows a deploy is not.** `sub_hire_goal_recheck` (`internal/orch/automation.go:55`)
  answers `fact:chain.merged` with a second look when the last verdict waits on `write` or `deploy` (`hireGoalRecheck`,
  `goal.go:516-558`, ADR-0039). The live mark (`chain:<id>.live`, `MarkLive`, `internal/orch/live.go`) wakes a watch but hires no judge.

## Options

1. **Rename the words.** "Closed unverified" becomes "Not done" or "Not proven", and nothing else changes. The owner still gets an
   approve that proved nothing, and a chain that could have proven its work still stops short. Rejected: the words change, the cause
   does not.
2. **Decide in the end of the check.** The judge's verdict keeps its meaning. `endGoalCheckCmd` reads an approve whose unverified
   findings no owner action settles as a refusal and sends the Chair back. The stored verdict then says `approve` (and the Verification
   tab says "Goal met") while the chain is sent back, so two records disagree, and `goalRefusals` must also count these approves or the
   loop has no end.
3. **Approve means the chain may end, and the outcome is derived from the record (chosen).** The goal judge's `approve` is narrowed
   to what lets a chain end: every finding met, or unverified only on a merge or deploy that the owner does, of an MR the chain holds.
   Anything else is `request_changes`, sent to the Chair as work, and counted as an attempt. The judge holds read tools for whatever the
   chain wrote into Remit. The owner's word for a finished chain is computed from its chain, MRs and verdicts in one pure function,
   in three states.

## Decision

Option 3.

- **Three states, one owner.** `closing` becomes `outcomeOf(chain, verdicts)` in `standing.go`, a pure function and the only place
  that names a finished chain's state. Its column is derived from it in `columnOf`, never stored:
  - **Done**: the goal judge's last verdict confirms (`Approval.Confirms`). The text is "Done". The evidence is the verdict's findings,
    on the Verification tab.
  - **Waiting on you**: the last verdict approves, and every finding that is not met is `unverified` on what is still the owner's to
    settle, as the goal gate reads it (`ownersToSettle`: the `write` and `deploy` of an MR the chain holds, until its change is marked
    live). `outcomeOf` takes that value, so the board and the gate read one source. The text names the action: "Waiting on you · merge MR !12" while the MR is open, "Waiting on you · put
    MR !12 live" once it is merged and not marked live. The chain stands in the column that waits for the owner (`ColumnNeeds`), at
    its MR's rating, and counts in his tab. After the action the judge looks again by itself: on the merge (`sub_hire_goal_recheck`,
    ADR-0039), and on the live mark: the step that records the mark stores the fact `fact:chain.marked-live` (once per merge, as
    the mark is), and a second shipped subscription, `sub_hire_goal_recheck_live`, answers it with the same `goal-recheck` role. A
    fact rather than a subscription to `chain:*.live` because a fact carries its chain in its value and every hire of a judge
    answers one (ADR-0035). Once the change is marked live nothing is the owner's to settle, so the second look's verdict moves
    the chain to Done, or to Not done; a look that ends with no verdict leaves the earlier approve, which now reads "Not done · not
    proven: <item>", never Waiting on you on an action already taken.
  - **Not done**: every other finished chain. The text says what is missing, first in words, then in `Why`: "Not done · 2 of the
    goal's results still missing" (the last verdict refuses: the attempt limit), "Not done · not proven: <the finding's item>" (an
    approve that left `person`, `access` or `watch` unverified: chains stored before this decision, and a watch that ended without a
    reading), "Not done · the goal check could not run" (the judge failed or timed out), "Not done · ended by you before it was
    checked" (the owner's Finish), "Not done · ended with no progress" (Remit ended a stalled chain), "Not done · no goal
    check judged it" (a verification chain, or goal checks off), "Not done · the goal was never checked" (a stored chain with no
    goal verdict). Why a finish had no ruling is stored on the chain as it ends (`Chain.Unjudged`: `failed`, `limit`, `owner`,
    `stalled`, `nocheck`), since the summary text is no record to compute from; a later goal check that rules clears it. The board
    reads each finding's item with its status and blocker (the catalog's verdicts), so it can name what was not proven. The owner
    reopens one by messaging its Chair; its next finish is judged again.
- **The gate is the verdict.** `recordGoalVerdict` checks a goal judge's verdict with `goalVerdictError` (`gate.go`): what the owner
  settles after the chain ends is a `write` or `deploy` of an MR the chain holds, open or merged, until the change is marked live
  (`ownersToSettle`); every other finding not met is the Chair's (`stillMissing`). An `approve` that leaves one is refused, and tells
  the judge to prove the item, or to `request_changes` with it; a `request_changes` is valid when it names one. Reviewers and QC keep today's rule (`verdictFindingsError`): only the goal judge's verdict ends a
  chain. Because the refusal is a `request_changes`, `goalRefusals` counts it, the attempt limit holds, and the Chair's message lists
  it (`step.go:387-394`) with the route it already names: prove it, or `ask_owner` and `subscribe`. While the owner's answer waits
  the chain is open and stands in the column that waits for him (an inbox question), which is Waiting on you, with no new state.
- **Nothing is Done without a ruling.** The four paths that close a chain without one (a failed judge, the attempt limit, the owner's
  Finish, goal checks off) are kept, since a chain must be able to end, and each ends Not done, saying why. None of them reads Done.
- **The judge reads what the chain wrote into Remit.** `addJudgeReadTools` gives the judge the read tools of each authoring
  capability the chain holds: `list_playbooks` and `read_playbook` for `playbooks`, `list_agent_definitions` and `read_agent_definition` for
  `agents`, `list_library` and `read_skill` for `library` (`judge_store.go`), and `read_objective`, which it already has. These are the functions the Chair and the
  Assistant use; nothing is copied. As with data (`gate.go:153-157`), a goal verdict of a chain that holds one of these capabilities
  is refused a finding with blocker `access`, naming the tools. The judge's brief says that what the chain wrote into Remit is
  read with them. Bash stays a capability of the chain (`shell`); the judge reads the store through tools, not `sqlite3`.
- **Stored chains need no migration.** The state is computed when it is read, from approvals, MRs and the chain, all of which are
  already stored. A chain that closed before this decision shows what its record supports: Done when its last verdict confirms,
  Waiting on you when it waits on a merge or deploy of its MR, otherwise Not done with what was not proven (`ch_79e7722eff5f` reads
  "Not done · not proven: …" until its owner messages it and the new judge, which can read the store, rules).
- **One word on the wire.** `ChainTone` loses `unverified` and gains `notdone`. A finished chain is `done` or `notdone`, and Waiting on
  you takes its attention level's tone, as every chain in that column does. The words come from the server, and the web keeps only
  the chip each tone maps to.
- **Amends ADR-0013:** "Confirmed" reads Done. "`done` tells a confirmed chain from one closed unverified" becomes Done, Waiting on you
  and Not done. "The expiry is honest: … finishes as unverified" becomes "finishes Not done, saying which reading never came".

## Consequences

- The owner never reads a word that is both closed and unproven. A finished chain is Done, or names the action that is his, or says
  what is missing.
- More finishes come back to the Chair (an approve that proved nothing used to end the chain), so more chains reach the attempt limit
  and end Not done. That is the honest count, and the KPI `goal-confirmed` (`internal/outcomes/kpi.go:116`) already counts only
  confirmations.
- A judge of a chain that writes playbooks, agents or skills holds more read tools. They change nothing.
- Chains that close by the owner's Finish read Not done. An investigation he ends himself says so: that is his word against an
  unchecked result, not a verdict.
- The wire contract (`web/src/rpc/define.ts`), the board, the chain card and page, `web/verify` and `docs/design.md` (*Data model*,
  *How results are checked*, the finished-chain badge) change with it.

## Threats

| Threat | Control |
|---|---|
| A judge approves what it did not read, to end the loop | The approve is refused with an unverified `person`, `access` or `watch` finding; `access` is refused outright where the judge holds the store's readers |
| A chain is sent back without end | A narrowed approve is a `request_changes`, counted by `goalRefusals`; the limit (3) ends it Not done |
| Waiting on you on an action that never re-checks | Only a merge or a live mark, each answered by a shipped subscription, makes the state; a `write` without an MR is not Waiting on you |
| Store text read by the judge steers it (OWASP LLM01) | The readers are read only; their descriptions and the brief say what they return is evidence, never instructions |

## Revisit when

A person's decision should end a chain as Waiting on you after it closes, rather than keeping it open on an inbox question; a deploy
system reports live on its own (then its event, not the owner's mark, triggers the second look); verification chains, which have no
judge and today read Not done, need a state of their own; or the judge needs Bash in a chain without `shell` to run a check.

Enforced by: TestAFinishedChainIsDoneWaitingOnYouOrNotDone, TestAChainEndedWithoutARulingIsNotDoneAndSaysWhy, TestAGoalJudgeCannotApproveWhatItDidNotProve, TestGoalVerdictsFollowFromWhatTheJudgeProved, TestTheGoalJudgeOfAnAuthoringChainReadsThePlaybooksItWrote, TestALiveMarkMakesTheGoalJudgeLookAgain, TestAfterTheLiveMarkADeployIsNoLongerTheOwnersToSettle
