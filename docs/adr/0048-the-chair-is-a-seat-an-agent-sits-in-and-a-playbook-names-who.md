# 0048. The Chair is a seat an agent sits in, and a playbook names who

Status: accepted, 2026-11-03; the template match of the seat superseded by 0062; the playbook naming who sits superseded by ADR-0113: the seat is chosen as for a playbook that names none. Builds on ADR-0032 (an agent is hired from a template and stays until retired), ADR-0033 (an employee
keeps a memory) and ADR-0045 (the OKR playbooks). Source: the owner's words of 2026-11-03: the Chair is a role, so an OKR expert, an
agent with its own tools, experience and memory, or an engineer can sit in it.

## Context

ADR-0032 decided that the Chair is no reserved id: an employee with the title `chair`, hired from a template. The build kept one
reserved template. `chairEmployeeOf` read the template `chair` for every chain, except one that serves a key result a standing Chair
owns. A Chair's session also never received its employee's loadout: `library.Access` applied a loadout to workers only, so a Chair
template's instructions, skills and tools were stored and never read. A chain of the OKR playbooks, whose work is judging measures,
therefore had the same Chair as a bug hunt, with nothing it had learned and no craft beyond its brief.

## Options

1. A second kind of Chair beside the first (an "OKR chair" origin). Two concepts for one seat, and each later difference (tools, memory,
   model) is a new branch on the kind.
2. The playbook's `chair_brief` carries the expertise. It is the status quo: the brief is text for one chain, so nothing accumulates, and
   an engineer cannot sit.
3. **Chosen.** The Chair stays one seat. A playbook names the agent that sits in it (`Playbook.Chair`, an agent id). The seat gives the
   duties (`chairPrompt`) and the tools (origin `chair`); the agent gives instructions, library skills and the tools it narrows, which the
   Chair's session now holds as a worker's does.

## Decision

- **`Playbook.Chair`** is an agent id, empty for the shipped `chair` (`defaultChair`). A playbook is refused when its Chair is no agent or
  cannot sit (`templateFits`: titled `worker` or `judge`, or signing a gate). A chain of a playbook whose Chair was deleted since is refused
  at creation, naming the playbook and the agent; the Chair is never replaced silently by another.
- **The seat.** `chairEmployeeOf` picks, in order: the standing Chair that owns the key result the chain serves (ADR-0032); else the oldest
  active standing Chair hired from the agent the playbook names (`standingChair`); else a new employee with the title `chair` and tenure
  `chain`, hired from that agent. An agent need not be titled `chair`: any template that fits can sit, an engineer included.
- **A Chair holds a loadout.** The Chair's card takes a copy of its employee's loadout, `library.Access` applies it as it does a worker's,
  and a Chair with none holds a plain agent's (every tool). The card's name is the employee's label.
- **Experience.** What a Chair learns outlasts its chain when the owner keeps it standing (`draft_hire` of a chair from the agent); the
  playbook then reuses it, and its memory (ADR-0033) carries over. A playbook that wants a Chair to keep a memory lists the capability
  `memory`.
- **Shipped.** The agent `okr-chair` (an OKR expert: it loads the `okr` skill, hires the OKR Analyst and Auditor, accepts no measure on
  a report) is the Chair of `objective`, `okr-review`, `okr-checkin` and `okr-instrument`.

## Consequences

- The Playbook author sets a Chair by talking to an agent (`chair` in `create_playbook` and `update_playbook`); the Playbooks page shows
  it, read only.
- Every Chair's session now starts with its template's instructions appended: the shipped `chair` template's one paragraph repeats what
  `chairPrompt` says and costs nothing.
- Two chains of one standing Chair run their turns one after the other (the employee's lease queues the later message, `lease.go`); a
  chain does not wait for a Chair it cannot reach, because the lease queues instead of refusing.
- Hires are unchanged: a Chair hires workers only, and an agent whose title is `chair` is not offered to hire.

## Revisit when

- The model of a Chair should follow its agent's (`Agent.Model` is not read for a Chair; `Config.ChairModel` is).
- Several standing Chairs of one agent exist and a chain should pick by scope or load, not by age.
- A standing Chair of an OKR playbook becomes a bottleneck through the lease, which is the day it needs a pool or a second seat.
- A chain should name its Chair directly (a request that says "have the engineer lead this"), not only through its playbook.

Enforced by: TestAPlaybookThatNamesNoChairSeatsTheShippedChair, TestAccessGivesTheChairWhatTheAgentInItsSeatHolds
