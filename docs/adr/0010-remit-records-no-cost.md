# 0010. Remit records no cost and holds no cap on it

Status: accepted, 2026-10-29; the counts it keeps for scheduling (`-max-waiting-mrs`, `-max-concurrent`) superseded by ADR-0115, which keeps the entrance counts. Supersedes 0002.

## Context

ADR-0002 made all-in dollars per merged merge request the figure that steers the system, and set a per-chain cap
(`-budget`), a daily ceiling (`-daily-budget`), a wrap-up and hard stop for a chain at its cap, and a usage ledger behind
all of it. The owner has since said that quality is the only target and that cost does not matter to this app
(`AGENTS.md`, *Where we are*). Each of those rails then costs something and protects nothing the owner wants protected: a
chain wrapped up or paused for money strands finished work, a daily ceiling pauses work he asked for, and the ledger, the
cost columns and the cost KPIs carry a figure nobody steers by.

## Options

1. Keep the caps as an owner rail and stop steering by the figure. Keeps a rail whose only effect is to interrupt good work.
2. Remove the caps and keep the ledger and displays for information. Keeps a write on every agent report and a second source
   of numbers that nothing decides by.
3. Remove the caps, the ledger and every cost display. Nothing remains that can disagree with the owner's target.

## Decision

Option 3. Remit records no cost and holds no cap on it:

- No `-budget`, `-daily-budget`, per-playbook cap, client-key cost limit, wrap-up or cost pause. Nothing is held for money.
- No usage ledger. Migration 42 drops the `usage` table, the cap fields of chains, playbooks and client keys, and the
  `budget` decisions. The Claude `total_cost_usd` is not read.
- No cost in the contract or the screens, and no cost KPIs in Outcomes. The KPI of merge requests without a decision stays,
  in the owner group, with the rest of what measures his review.

What stays are limits on counts that protect the owner's attention and the host, not money: `-max-waiting-mrs` (ADR-0001),
`-webhook-chains-per-day`, `-max-concurrent`, `-max-dispatchers`, `-max-intakes`, and a client key's chains a day and acts an
hour (ADR-0009).

## Consequences

- A runaway loop is bounded by the owner's pause and stop, by the slots and by the liveness rails, not by a ceiling.
- Past spend is gone from the database: the usage rows are deleted by the migration, and a backup taken before it holds them.
- The doctor (the outside evaluator) no longer measures dollars; it measures outcomes and behaviour.

## Revisit when

The app runs on metered pricing the owner has to pay for, or runs for someone other than its owner.
