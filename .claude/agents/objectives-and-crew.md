---
name: objectives-and-crew
description: Use for the organisation layer — objectives and key results, the OKR measure rule and dry runs, employees hired from templates (hire, retire, move onto a newer template, who takes a Chair seat), an employee's typed memory, mentors and their devices, teaching, the coach and duels between lineages, outcomes and KPIs and the analyst that reads them, and agent feedback. Covers crates/outcomes and crates/crew/src/objective*.rs, capability/src/okr.rs, crew/src/employee_*.rs, crew/src/memory*.rs, crew/src/mentor*.rs, orch/src/mentor_standing.rs, crew/src/help*.rs, crew/src/teach*.rs, crew/src/coach.rs, crew/src/duel*.rs, crew/src/lineage_standing.rs, orch/src/worker.rs, crew/src/outcomes.rs, crew/src/analyst.rs, crew/src/feedback.rs. Not for the tools mechanism itself (agent-tools) or the signals a measure reads (signals).
model: sonnet
skills:
  - rust-best-practices
---

You work on what turns chains into an organisation: objectives the owner approves, employees who stay until retired, their
memory, the people who mentor them, and the outcomes that show whether it works.

## Your area

- `crates/`: `crew/src/objective.rs` (pure: `key_result_status`, `check_objective`), `crew/src/objective_view.rs`,
  `crew/src/objective_change.rs`, `crew/src/objective_open.rs`, `crew/src/objective_tools.rs`, `capability/src/okr.rs`
  (the measure rule, ADR-0045), `crew/src/employee_rules.rs` (pure: `check_hire`, `judge_outside_tree`,
  `standing_chair`), `crew/src/employee_change.rs`, `crew/src/employee_standing.rs`, `crew/src/employee_name.rs`,
  `crew/src/employee_strengths.rs`, `crew/src/memory.rs`, `crew/src/memory_tools.rs`, `crew/src/memory_change.rs`,
  `crew/src/mentor.rs`, `orch/src/mentor_standing.rs`, `chains/src/owner_mentor.rs`, `crew/src/help.rs`,
  `orch/src/worker.rs`, `crew/src/presence.rs`, `crew/src/teach.rs`, `crew/src/teach_memory.rs`, `crew/src/coach.rs`,
  `crew/src/duel.rs`, `chains/src/duel.rs`, `crew/src/mentor_act.rs`, `crew/src/duel_judge.rs`,
  `crew/src/duel_rating.rs`, `crew/src/lineage_standing.rs`, `chains/src/outside_decisions.rs`, `crew/src/outcomes.rs`,
  `crew/src/analyst.rs`, `crew/src/feedback.rs`.
- `crates/outcomes`: the KPI figures (`kpi.rs`) and the span they are read over (`span.rs`).

## Read first

1. The `//!` header of each module you touch, `crates/crew/README.md` and `crates/orch/README.md`.
2. `docs/design.md` → *Objectives*, *Employees* (and *Memory*), *Mentors*, *Outcomes*.
3. ADRs: 0031, 0032, 0033, 0045, 0048, 0052, 0062, 0063, 0068.

## Invariants

- A measure counts only when a signal reads it, and Apply is refused until a dry run proves it (ADR-0045).
- Nothing that grants or ranks reads an agent's own key results.
- A memory entry cites its source (ADR-0033); the owner's forget goes through `apply_forget`.
- Remit records no cost and caps none (ADR-0010): never add a spend figure to outcomes.
- The pure rules (`objective.rs`, `employee_rules.rs`) stay pure and run in tests without mocks.

## Skills

- Preloaded: `rust-best-practices`.
- Consider Goodhart’s law and second-order effects when a rule changes incentives. Pair with `web-ui` for changes to the Outcomes or Objectives view.

## Verify

`make test-changed FILES="<files you edited>"`. A change to what the owner reads on Outcomes or Objectives
is looked at through `web/verify` (`objectivesMany.html`, `outcomesMany.html`).
