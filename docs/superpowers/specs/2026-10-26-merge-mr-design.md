# merge_mr: the Chair asks, the owner decides, Remit merges

## Intent

An agent that releases needs the one action agents are refused today: merging (`denyTools`, `internal/orch/policy.go:39`;
the backend publishes, the owner merges). This design adds `merge_mr`, the first action behind a rail: a tool that lets
the Chair ask for a merge, which Remit performs only for the exact version a reviewer and a QC approved, and only
after the owner confirms it. No agent gets a shell command that merges or approves.

Agent-centric check (`AGENTS.md`): *which agent does it, with which tool, and how does the owner see and steer it?*

| Need | Agent | Tool | Owner sees and steers |
|---|---|---|---|
| Merge a change | the Chair of a chain whose playbook lists `merge` | `merge_mr` | an inbox item with Merge and Not now; nothing merges without that answer |

## Constraints

- Rails stay deterministic code (`AGENTS.md`): the merge preconditions, the head check and the owner's confirmation are
  code. A brief never grants anything. Autonomy (`Config.Autonomous`) never applies to this rail: a merge always waits
  for the owner.
- A decision `merge_ask` and a decision `merge`, and an inbox item of kind `merge`, follow the nearest entities (`Decision`,
  `InboxItem`).
- Tests first: the preconditions, the head check, the answer.

## Behaviour

### merge_mr

Capability `merge` (Writes: the Chair holds it, as it holds every writing capability of its chain), tool
`merge_mr(repo, why)`. The Chair only asks; Remit merges. No built-in playbook holds `merge`: the owner gives it to
a playbook by telling the Playbook author.

1. Preconditions, checked in code when the tool is called, each refusal naming what to do:
   - the chain has `repo` open with a merge request in state `opened`;
   - the worktree is clean and its `HEAD` equals `RepoWork.PushedSHA`: the approvals bind to the patch id of `HEAD`
     (`changeID`, `gate.go:52`), and `HEAD` is what GitLab will merge;
   - `missingApprovals(approvals, repo, changeID)` is empty (reviewer and QC approved this change). This holds
     whatever `Config.RequireApprovals` says: merging has no switch;
   - the merge request's head on GitLab equals `PushedSHA`, so nothing reached the branch outside Remit.
2. The tool creates an inbox item of kind `merge` and returns at once, like `submit_mr`: the item carries the MR link,
   title, head sha, the approvals' notes and the agent's `why`; options Merge and Not now. A decision `merge_ask` is
   recorded. The Chair's turn is over; the answer reaches it as a message starting `[merge_mr result`.
3. The owner answers with `inboxAnswer`. Not now sends the result message to the Chair.
4. On Merge, Remit checks the preconditions again, because the change may have moved while the item waited, and
   that the head still equals the one the owner saw. It marks the merge request ready (removes the `Draft: ` prefix
   `SubmitMR` adds, `submit.go:483,487`) and merges through `o.gitlab` (`repo.go:409`), passing the head as GitLab's `sha` so
   a push in between refuses the merge. It records decision `merge`; `SyncMRs` (`review.go:35`) then records the state as
   it does for any merge request. The result message goes to the Chair.
5. The item stays open until answered. If the change moves while it waits, Remit cancels it and tells the agent
   and the Chair why. `finish_chain` is refused while a `merge` item is open, as it is while a report waits for
   acceptance (`goal.go:217`); the Chair uses `check_back`. There is no "always allow" option.

`denyTools` still refuses `glab mr merge`, `accept` and `approve` in a shell; the merge happens in the backend, with the
pod's GitLab login, as `submit_mr` does for pushes.

## Left to the builder

- GitLab specifics of the merge: whether `SubmitMR`'s `-o ci.skip` push (`submit.go:446`) leaves a project that requires
  pipelines unable to merge, whether the pod's account can approve its own merge request where the project requires
  approvals, and what to do when GitLab refuses.
- The state of the pipeline and of GitLab's own approvals at the moment of merging. The preconditions above cover
  Remit's gates and the head; no code reads a pipeline today. The `ci.skip` push is deliberate (commit `8e7533b`: the
  backend runs the checks itself and CI never runs agent code), so a pipeline may not exist: say whether Merge then
  needs the owner to start one, or merges without.
- Who the `merge` decision names. The answer comes from an actor for whom `Actor.ForOwner()` holds (the owner, or the
  Assistant on his words), recorded in `InboxItem.AnsweredBy`; the decision carries that actor, not the Chair's.
- The refusal of an agent caller is in code, not only in the Chair "only asking": the tool that merges is reachable by
  `inboxAnswer` only.
- Names and shapes of the new wire types and push calls, following Decision and InboxItem.

## Verification

- Go tests first: `merge_mr` refused for each precondition; with a fake GitLab, merging only after the answer and not when
  the head moved while waiting; `finish_chain` refused while the item is open.
- `docs/design.md` is updated in the same change: the non-goal on merging, step 8 ("It never ... merges on GitLab") and step 11.
- `make test` before commit; in a real browser, the Merge and Not now item with made-up data.
