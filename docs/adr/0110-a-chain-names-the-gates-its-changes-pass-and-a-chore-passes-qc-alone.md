# 0110. A chain names the gates its changes pass, and a chore passes QC alone

Status: accepted, 2026-11-18; the landing reads its gates where `submit_mr` and `merge_mr` did (ADR-0114); amended by ADR-0113: the gates are read from the chain's kind (`ChainKind::gates()` returning `model::ChangeGates`, `qc` for `chore`), `Chain.gates` is not stored, and ADR-0113 builds the chore kind; where this record says `Chain.gates` is stored or set, read the kind's gates.
Amends ADR-0087's change gates (review and QC on every change). Source: the owner, 2026-11-18: "make a chore playbook:
agents always call in the heavy machinery, but some things are very simple", and his choice the same day that the chore
setting is a chain kind, since a playbook is reference only (a name, a brief, suggested agents).

## Context

Every change a chain publishes passes a reviewer and a QC (`GATES` in `chains::gate`, checked by `submit_mr`, `merge_mr`
and the merge queue), and the Chair's rules say so. A playbook may drop research and design, but it cannot drop a gate: a
one-line fix still costs an engineer, a reviewer, a QC and the goal judge. The Chair, told review and QC are mandatory,
plans every task as a project.

The owner's standing rule (2026-11-07) is that gates are telemetry and reality, not prediction: review is the gate for
rails, and a change otherwise proves itself by running. QC runs the change; a reviewer reads it.

## Options

1. **A chore playbook with a brief only.** Leaves the four-agent floor in place: the weight the owner named.
2. **A playbook field naming its gates.** Playbooks are reference only and carry no rule.
3. **The chain names its gates, set by its kind; the chore kind names QC alone.** The gate stays deterministic code;
   which gates apply is a property of the kind of work, fixed when the chain is made.
4. **The Chair decides per change which gates apply.** The agent being judged would choose its own judges.

## Decision

Option 3.

- `model::ChangeGates` (`review-and-qc`, the default, or `qc`) is a field of `Chain`. The chore chain kind sets `qc`; every
  other chain keeps the default. No agent tool changes it.
- `required_gates` starts from the chain's gates and adds `security` when the change touches a path `.remit/rails` names,
  whatever the chain's kind. `submit_mr` records them on the MR as before; `merge_mr` and the merge queue read them, and the
  chain's verification view shows them.
- The Chair's rules name the gates its chain's changes pass (`gates_rule`). A chore chain hires one QC and no reviewer.
- The built-in `chore` playbook suggests one engineer and one QC, and tells the Chair to move a task that turns out bigger
  to a heavier chain rather than grow the chore.
- Stored chains carry no field and read as `review-and-qc`; nothing migrates.

## Consequences

- A chore costs an engineer, a QC and the goal judge. The goal judge stays: it checks the result against the goal.
- Whoever starts a chain as a chore chooses fewer gates. The goal judge still rules on the result, the check still has to
  fail before the work, and a rail still needs security.

## Revisit when

- `verdict_rates` (ADR-0103) shows chore chains sent back by the goal judge or reverted after merge more often than others:
  the seam is `ChangeGates`, where the kind could name `review-and-qc` again or a judge be added.
- A kind of work wants a reviewer alone, with no QC: a third `ChangeGates` value.
