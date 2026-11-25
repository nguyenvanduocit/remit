# 0001. Work is admitted by the owner's capacity to review it

Status: superseded by ADR-0115 (capacity is measured and shared by key result; no count of waiting changes paces work), 2026-11-19; accepted, 2026-10-27; the merge requests it counts are 0078's waiting list; its waits on the owner, but a secret act, are an agent's (ADR-0087). Source: the architecture review of 2026-10-27 (commit 3b30ede on the local branch docs/architecture-review-2026-10-27), Appendix 8 section 6.

## Context

Everything Remit starts on its own (queued starts, the Scout, routines, webhook deliveries) ends as a merge request
only the owner may merge. Spend caps limit what agents cost; nothing limits how much waits for review. Past the owner's
capacity the backlog grows without bound, the owner's attention becomes the bottleneck, and the work already paid for goes
stale.

## Options

1. Keep as is: caps on spend only.
2. A hard cap N on merge requests waiting for the owner. At N, new autonomous starts wait.
3. An adaptive cap that follows how fast the owner reviews.

## Decision

Option 2, with N = 5 as the default and a server flag to change it.

- The count is merge requests that are open and waiting for the owner, over every chain.
- At the cap, queued starts, the Scout, routines and webhook deliveries stay queued. They start as the count falls.
- Never held: the owner's own actions (a prompt, an answer, a start the owner asks for), pause, stop, and a chain that is
  already running.
- The cap is one rule, in the pure admission function (`internal/orch/admit.go`), tested without mocks. What is held shows in
  the UI as a waiting standing with the reason, so no agent changes anything the owner cannot see.
- It is a rail: no agent raises it.

## Consequences

- The backlog is bounded, so the owner's review time sets the pace of the whole system.
- A burst of webhooks queues instead of fanning out into agent spend.
- Option 3 stays open: the rule reads one number, so changing how it is chosen touches one function.

## Revisit when

The owner says the cap holds back work they would have reviewed, or the median time a merge request waits is under a day for a
month (the cap is then too tight).
