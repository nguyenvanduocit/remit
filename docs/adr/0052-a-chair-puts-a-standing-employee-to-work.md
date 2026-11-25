# 0052. A Chair puts a standing employee to work, in the role it gives

Status: accepted, 2026-11-03; a person's `question` and card superseded by 0062. Source: the owner's approval of the human-employees design
(`docs/superpowers/specs/2026-11-03-human-employees-design.md`, decision 4): engineers work in Remit as employees, and a Chair
puts any standing employee to work.

## Context

`Hire` always made a new employee: a Chair's `hire_agent` built a worker from an agent definition (`workerFor`, `lifecycle.go`) and
stored it with its card (`records.employees`, `tools.go`). An employee the owner keeps standing (ADR-0032) could sit in the Chair seat
(ADR-0048) but could not be given a card by a Chair. A person (an engineer, `Kind person`) is hired only by the owner, through the
Assistant, as a standing worker. A Chair could not give them work, because it could only make new agents. The gate an employee signs
came only from its agent definition (`Agent.Signs`), and a person has no agent definition.

## Options

1. `hire_agent` takes `employee`, the id of a standing employee, in place of `agent`, and `role`, the gate it signs. **Chosen.** The
   card, brief, inputs, handoffs, gate and report path are the ones every hire uses, and nothing else changes.
2. A separate tool (`assign_employee`). A second door to the same card and the same rules: the two would drift, and a Chair would
   have to learn which to use.
3. Copying the standing employee into a new chain-tenure employee for each hire. Its memory, accountability and record would split
   across copies, and a person would be several employees.

## Decision

Option 1.

- **`employee` names who works**, an active standing worker (`standingFor`, `lifecycle.go`; the rule `standingRefusal`,
  `employee_rules.go`). `agent` and `tenure` are refused beside it: the employee's record already decides both. No employee is
  stored. The card's `Employee` is theirs.
- **`role` gives the gate**: worker (the default), reviewer or qc (`signsOfRole`, `chair_hire.go`). It is taken only with `employee`, because
  a new hire's role is its agent's. A reviewer or QC lists the cards it judges in `inputs`, as any does.
- **The ceiling holds.** The employee's ceiling must be within the Chair's (`ceilingExcess`), the rule every hire meets (ADR-0032).
- **An agent's card copies the employee's loadout**, with each list the Chair gives in its place, on its template's model unless the
  Chair names one.
- **A person's card holds no loadout and no model**: `Agent` is `person`, the `Cwd` is the chain's directory, and `cwd`, `tools`,
  `skills` and `model` are refused, because the person works with their own tools. The decision reads `Put Lan to work (person,
  worker).`

## Consequences

- One employee holds cards in several chains. Their memory and record stay in one place.
- A card's gate comes from its agent for a new hire and from the role for an employee put to work. `gateClosedBy` reads the card's
  `Signs` either way.
- A Chair whose ceiling is unset (a chain-tenure Chair of a direct request, `chairEmployeeOf`) holds less than `reversible`, so it
  cannot put a person, hired at `reversible`, to work. A chain seated by a standing Chair (ADR-0048), which is hired at `reversible`,
  can.
- A person's card is a card with a run, through a launcher of its own (`agent.PersonLauncher`, ADR-0051), which decides which
  machine rules leave it alone.

## Revisit when

- **A Chair should see an employee's memory before choosing one:** `list_agents`' standing lists show presence, load and outcomes, but
  not what each one has learned.
- **An owner-started Chair should put people to work:** give a direct request's Chair the chair ceiling (`chairCeiling`) when its
  employee is made.

Enforced by: TestAChairPutsAKeptEmployeeToWorkWithoutHiringAnother, TestANewHireBesideAnyIdleWorkerNeedsAWhy
