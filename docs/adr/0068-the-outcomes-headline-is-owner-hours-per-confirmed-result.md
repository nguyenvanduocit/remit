# 0068. The Outcomes headline is owner hours per confirmed result

Status: superseded by ADR-0114 (no MR waits for the owner; the headline is landings taken back per confirmed result), 2026-11-19; its check rail's merge door (`CheckDoor::AtMerge`, "a merge always ships") removed by ADR-0116; accepted, 2026-11-07. Source: the owner's words of 2026-11-07 ("the Outcomes headline becomes the loss function: owner hours
per confirmed result, with results confirmed as the constraint shown beside it"), `docs/design.md` (Outcomes), ADR-0002, ADR-0010,
ADR-0016, ADR-0067.

## Context

The Outcomes page shows eleven KPIs in three groups of equal weight (quality, owner, reliability; `internal/outcomes/kpi.go`).
None of them says what the system is for in one figure, so neither the owner nor the analyst agent has a single number to steer
by. ADR-0002 once named one (dollars per merged MR); ADR-0010 took cost out of Remit, and nothing replaced it.

What Remit is for: the owner states intent and agents deliver a result the customer gets (AGENTS.md *Agent-centric*,
ADR-0016). Two things are recorded that measure that:

- **A confirmed result.** `Period.GoalConfirmed` counts the chains whose latest goal check approved with every key result met
  (`model.Approval.Confirms`, the rule behind a finished chain's Done, ADR-0067). It is the only record that the customer's result
  was shown to hold.
- **The owner's time on it.** Remit records no measure of his reading or review time. What it records is how long each MR he
  decided waited for him: from the submission of the version he decided to his merge or close (`waitOf`,
  `internal/outcomes/outcomes.go`, summed into `Period.WaitedHours` in the period of `MRRef.DecidedAt`).

## Options

1. **Keep the groups as they are.** No figure to steer by; the analyst picks "the biggest move", which is whatever moved most.
2. **MRs merged per owner hour.** Counts merges, not results: a merged MR the goal judge never confirmed counts the same, and a
   result with no MR (research, data) counts nothing.
3. **Average wait per MR (`owner-wait`, the existing figure) as headline.** Falls when MRs are small and many, whatever they deliver.
4. **Owner hours per confirmed result, with the share of results confirmed beside it.** The numerator is the owner's cost, the
   denominator the outcome. Read alone it can be lowered falsely (confirm nothing and it has no figure; confirm only trivial goals and
   it falls), so the constraint is shown with it, not in another group.

## Decision

Option 4. A new first group, `headline` ("Loss"), holds two KPIs in this order:

- `owner-hours-per-result`, "Owner hours per confirmed result": `Period.WaitedHours / Period.GoalConfirmed` over a period,
  `Of = GoalConfirmed`, no figure when no result was confirmed; kind hours, better down.
- `goal-confirmed`, "Results confirmed", moved from the quality group (one KPI, one group): the constraint the loss is read against.

It has every other KPI's shape: a figure per period of the span, the last against the one before, all time, and the server's reading
of the move. The KPI carries `over` ("confirmed result"; `owner-wait` says "MR"), so the page and `Describe` name what an average is
over instead of assuming MRs. No table or migration: both counts were already recorded.

Assumption: the owner's hours are the hours decided MRs waited for him, because no record holds his active time. The hint says so in
plain words.

## Consequences

- The page and the analyst read the loss first (`OutcomesView.vue` renders the groups in the server's order; `Describe` tags it
  `[headline; better when lower]`).
- The numerator and the denominator are counted in the same period but not over the same chains: hours of MRs decided in the period,
  results confirmed in it. A week of merges whose results are confirmed the week after reads high, then low. Over all time they meet.
- Wait is not effort: an MR that waited overnight counts twelve hours although he read it in five minutes. The figure is an upper
  bound on his time and moves with his response time as much as with the agents' work.
- It steers the owner and the analyst; it scores no agent. Outside the page and `outcome_metrics`, the only reader of KPI keys is the
  objective checker, which accepts any of them as an objective's surrogate (`checkedObjective`, `internal/orch/objective_tools.go`;
  `CheckObjective`, `internal/orch/objective.go`), so an objective may name `owner-hours-per-result` as its surrogate from now on.
- No cost enters it (ADR-0010): hours are the owner's, never spend.

## Revisit when

- **A record of the owner's active time exists** (the time he had a chain or MR open, or his review session's span): it replaces
  `WaitedHours` in the numerator. The seam is the one `of` function of `owner-hours-per-result` in `kpi.go`; no caller changes.
- **Results without an MR become common** (research, data chains confirm with no owner wait): the loss then reads low for work that
  took his reading. The same record above fixes it.
- **Confirmed results can be trivial by construction** (a Chair writing goals the judge cannot fail): the constraint then needs the
  goal's weight, not its count.
