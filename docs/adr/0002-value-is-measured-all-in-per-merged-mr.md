# 0002. Value is measured as all-in cost per merged merge request

Status: superseded by 0010, 2026-10-29. Accepted 2026-10-27. Source: review Appendix 8 section 6.

## Context

The outcome numbers count chains and decisions, not what a merged change cost. A chain that spends for hours and is
abandoned, a stalled chain and a merge request waiting for the owner are all missing from the per-merge figure. Spend
defaults were set without data.

## Decision

The figure that steers the system is **all-in dollars per merged merge request**: the spend of every chain, including those
that ended abandoned, stalled or failed, divided by the merge requests merged in the same period. Next to it, always:

- the share of merge requests with no decision yet, and the age of the oldest;
- the spend of chains that ended without a merge.

Budget and daily-ceiling defaults are set from the observed spend of this figure, not guessed. Changing a default stays the
owner's act, never an agent's.

## Defaults, 2026-10-27

Chosen from the only measured spend there is: the architecture review's read-only query of the live database (review O4 and
Appendix 8): $429 spent on 2026-10-26 and $479 on 2026-10-27, $1,126.07 over 3.5 days, about 114 chains, 8 merged MRs
($141 all-in per merged MR), and $8.25 own cost per merged chain. The margin rule is about 1.5 times what is observed
(review P1-3), because the consequence of the rail tripping (work pauses until the owner resumes it) is cheap next to the
consequence of it not tripping (a runaway loop spending a month's budget in a day).

| Flag | Was | Now | Why |
|---|---|---|---|
| `-daily-budget` | $10,000 | $700 | 1.63 times the one whole day measured ($429), 1.46 times the $479 read for 2026-10-27. Past it nothing starts and a chain at work pauses at its next spend. |
| `-budget` (per chain) | $1,000 | $100 | About 10 times the mean chain ($1,126 / 114, about $9.90) and 12 times the mean merged chain's own cost ($8.25). An autonomous chain wraps up at it and stops at 1.25 times it, $125: one runaway chain takes at most 18% of a day. |
| `-max-waiting-mrs` | none | 5 | ADR-0001. |
| `-webhook-chains-per-day` | none | 3 | One delivery's most (`chainsPerDelivery`); one webhook in use, so its worst day is 3 times $125, about half the daily budget. |

The $479 of 2026-10-27 was read from a backup taken at 12:12 that day, so it is likely half a day: at the same rate the
whole day would have been about $940, and $700 would have paused work that afternoon. That is accepted: 83% of the spend
measured ($933) was work Remit gave itself (Scout runs and the chains they proposed), which ADR-0001 now holds while
merge requests wait (52 waited when it was measured), so the spend a day should fall well under the measured days; a
pause that does happen costs the owner a resume the next day. If the rail trips on days he calls normal, raise it to 1.5
times the highest day the ledger then shows.

What is not known: the spread of cost per chain (only the mean was measured), so the per-chain cap may cut a legitimate
long chain; it wraps up honestly when it does, and the owner sets a playbook's own cap for work he knows is larger. The two
days measured were dogfooding while the product was built. Revisit both numbers once the all-in figure has four weeks of
data: the daily budget at 1.5 times the highest day seen, the per-chain cap at 1.5 times the costliest chain that merged.
The analyst playbook keeps its own cap ($10,000, `internal/orch/playbooks.go`), since its one chain lives for weeks; the
daily budget now bounds what it spends in a day.

## Consequences

- The figure is derived on the server from stored cost and the merge record, once, in the outcomes package.
- A decision type that would hide cost (a stall that is not counted) is a defect.

## Revisit when

A different cost basis is needed (subscription versus metered pricing).
