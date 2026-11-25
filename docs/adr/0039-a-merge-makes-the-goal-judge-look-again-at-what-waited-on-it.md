# 0039. A merge makes the goal judge look again at what waited on it

Status: accepted, 2026-11-02; the merge it answers is a landing, `fact:chain.landed` (ADR-0114). Source: the owner's report of 2026-11-02 that finished chains read "Closed unverified · deploy" after
their change was live, on this instance and on others; `docs/design.md`, ADR-0013, ADR-0035. Completes ADR-0013's "after the merge the
chain waits" for a chain that finished before its merge.

## Context

The goal judge rules when the Chair asks to finish, which is before the owner reviews the MR. A key result that shows only after the
merge or the deploy is then `unverified` with blocker `write` or `deploy` (the judge's brief says so), and the chain closes, its
badge read from that last verdict (`closing`, `standing.go`). ADR-0013 meant the chain to wait through the merge and judge again, but
that wait is a watch the Chair must set before it finishes. Chain `ch_32bdd16fb993` had none (no subscription of its own): the judge
ruled at 17:40, the MR merged at 17:44, the site went live, and nothing judged again, so the chain stayed "Closed unverified · deploy"
while `https://aiocean.io/nope-xyz123` returned 404 as the goal asked.

## Options

1. A finished chain stays `waiting` until its MR is merged and judged. It holds a chain open for as long as the owner takes to review.
2. **A merge is a fact, and a subscription hires the judge again for a finished chain whose last verdict waits on a merge or a deploy.**
   **Chosen.** It needs no watch from the Chair and no state beyond the check itself.
3. The owner reopens the chain by hand. This is what happens today, and it is the defect.

## Decision

- **A merge is a fact.** `fireRecordMR`, the one place that records an MR's state (from what the forge said, ADR-0049), also records `fact:chain.merged` (`{chain}`) in the same
  step when the MR is merged, and the next tick judges the subscriptions to it (ADR-0035). The record cannot judge them itself: a firing runs it under `tickMu`.
- **A shipped subscription answers it:** `sub_hire_goal_recheck` hires the role `goal-recheck`. The owner sees it on the Subscriptions tab
  and may pause it, as the other shipped ones.
- **It hires only when there is something to settle.** The chain is finished, has no check pending, goal checks are on, and the goal
  judge's last verdict has a finding that is not `met` with blocker `write` or `deploy` (`awaitsMerge`). Otherwise the subscription moves
  on with no firing.
- **The second look is a goal check of a finished chain** (`Finishing.Recheck`): one judge, the same brief plus a note that the change is
  merged and that a deploy takes minutes, so it tries the live system for a few minutes before it calls a result unverified. Its deadline
  is the goal check's. It finishes nothing: the chain stays `done`, its verdict is the chain's new last word, and the badge follows it
  (`Confirmed` when every key result is met). A judge that gives no verdict leaves the earlier verdict standing, and a decision says so.
- **Rails:** it merges, deploys, reopens and changes nothing. A "not met" verdict is recorded and shown; it does not reopen the chain.

## Consequences

- A chain whose effect shows after the merge closes Confirmed without the Chair having set a watch, and without the owner reopening it.
- A merge of a chain whose verdict waited on nothing a merge settles (a person's choice, an access) hires nobody.
- One more judge per merged chain at most, and only where the last verdict named a blocker of that kind.
- A second look that finds a key result not met does not send the chain back to work. The owner reads it in the decisions.
- The deploy may land after the judge's last attempt; the chain then stays unverified, as an unmerged one would. The next merge, or the
  owner telling the Assistant the change is live, are the other ways to ask again.

## Revisit when

A not-met second look should reopen the chain's Chair (ADR-0013 `reopen`); deploys are slow enough that the judge's wait is not
enough (a `chain.live` fact, `MarkLive`, hires the judge as well); a revision of the owner's merge happens with no MR (a direct push).
