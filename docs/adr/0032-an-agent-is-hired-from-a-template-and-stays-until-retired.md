# 0032. An agent is hired from a template and stays until it is retired

Status: proposed, 2026-11-02; its one live turn per employee (the lease) superseded by 0073; its waits on the owner, but a secret act, are an agent's (ADR-0087); the judge template's model is the catalog's strongest, as every judge's (ADR-0102). Source: the owner's words of 2026-11-02: the Chair should be a long-lived agent the owner hires from a template (the
Agents tab), as an instance with its own memory; the agents it hires are the same kind of thing, and a Chair may keep some full time and let
others go; a manager is not bound to one chain and can call its staff back when it takes other work; every agent is an agent and differs by
title and capability; the judge is no longer made by the system but hired, a peer of the Chair under the same superior, and may open a chain of
its own; the owner answers for the company, an agent for each objective, a Chair for each key result. Builds on ADR-0031 (objective, key result),
ADR-0013 (a chain owns its outcome), ADR-0018 (grants), ADR-0027 (the effect scale), ADR-0012 (a change that reaches later chains goes through the
owner).

## Context

`Agent` is a definition (`internal/model/library.go:50`): a model, a loadout of skills and tools, a prompt and the gate it signs. A hire copies
its loadout into a `Card`, one agent at work in one chain (`model.go:313`); the Chair's `hire` tool does this (`tools.go:116`). Everything a
card is dies with its chain: nothing keeps what an agent learned, who it answered to, or that it was good at a part of the product. The Chair is
the reserved id `chair` and the goal judge the reserved id `judge` (`library.go:20`), both made by code for each chain. `Chain` is "one case
worked by one Chair and the agents it hires" (`model.go:31`). So no one can be accountable for an objective that outlives a chain, and a judge
that code makes for the team it judges is independent only because code says so.

## Options

1. Keep the card per chain and add long memory to the template. Every chain's Chair shares one memory and one identity; two chains write the
   same memory at once, and nothing says who is accountable.
2. A new kind of thing, "senior agent", beside the card, with its own staff and tools. Two concepts for an agent.
3. One concept: the template is the pattern and a hired **employee** is the instance, with a title, a place in the reporting tree and its own
   memory. A card is the employee at work in a chain. A Chair, a judge and a worker are employees with different titles. **Chosen.**

## Decision

- **Template and employee.** `Agent` stays the template, in the Agents tab. An `Employee` has `id`, `label` (the owner's name for it, "login
  chair"), `template` and `templateRev` (copied at hire: a later change to the template does not reach it; the owner moves it onto a newer
  template through a change item), a copy of the template's `Loadout`, `title`, `scope`, `reportsTo`, `tenure`, `status` and a memory
  (ADR-0033). It is stored in its own table, carried in the contract (`web/src/rpc/define.ts`), and shown on the Crew tab as the reporting
  tree.
- **Title decides the rules.** `Title` is a closed set: `chair`, `judge`, `worker`. A new title is a change to the rules and their tests. Past
  that, every employee is the same thing. The Assistant, `intake` and `probe` are system roles, not employees.
- **Scope.** `scope` is the area the employee answers for (the owner's words, "user login"), a `ceiling` and the key results it owns. A chain it
  works narrows the ceiling to the task; nothing widens it. A `Ceiling` is three parts: the strongest `Effect` (ADR-0027), a set of grants
  (ADR-0018) and a set of Claude Code tools. One ceiling is not above another when its effect is no stronger and its grants and tools are subsets.
  It is enforced at the places that already enforce each part: the grant check, the session's tool list and the jail. A ceiling without write
  tools mounts the worktree read-only, so a judge at `read` cannot edit what the team made.
- **Titles of the templates in use.** The shipped `reviewer` and `qc` templates (`Signs` review and qc) are workers. Only a template meant for a
  judge may sign the goal (`SignsGoal`, `library.go:16`): that validation moves from "no agent definition" to "only a definition whose title is
  `judge`", and no other title can take it.
- **An agent or a person.** An employee has a `kind`: `agent`, which Remit runs from its template's loadout, or `person`, an engineer who
  works through their own tools (`docs/superpowers/specs/2026-11-03-human-employees-design.md`, decision 2). A person is hired from no template:
  no `template`, no loadout, title `worker`, tenure `standing`, reporting to the owner or to a chair, at `reversible` unless the owner names a
  ceiling. Only the owner hires a person; no agent makes one.
- **Hiring.** The owner hires chairs, judges and people: the Assistant drafts the hire on his words as a change item of kind `employee`, and he
  applies it himself (a hire creates a principal with a ceiling, so it is a rail as ADR-0029 defines it); a hire of a chair or a judge may also
  ride on an objective's change item (ADR-0031). A Chair hires agent
  `worker`s with `hire`: `tenure` `chain` (retired when the chain finishes, as a card is today) or `standing` (kept). Rules, each a pure check:
  the new employee's ceiling is not above the hirer's; headcount and depth are capped, as the other caps are (a cap is a rail: no agent raises
  it); a Chair hires no chair and no judge.
- **Retiring is not deleting.** A Chair retires an employee it hired; the owner retires any. `retired` keeps the record and the memory, and the
  same employee can be hired again by the owner or its former manager. Nothing deletes a memory except the owner, who applies a forget the Assistant drafted.
- **One live turn per employee.** An employee has one card for each chain it works (`Card` stays the session of one seat in one chain). A
  lease is held for a turn, not for a chain: two turns of one employee never run together, so its memory has one writer at a time. A chain whose
  Chair waits for its workers, or is `waiting` (`model.go:16`), holds no lease, so a Chair with several chains works the one that woke and the
  others queue. Finish checks of one hired judge queue the same way (short turns); more parallel work is more employees hired from the same
  template. (My call; the other choice is several sessions with conflict handling on the memory.)
- **The judge is an employee.** Its title is `judge`. Rules: it reports to the superior of the Chair it judges and never inside that Chair's tree;
  its ceiling is `read`, so it cannot edit what the team made, and a chain it opens inherits that ceiling; the Chair neither hires, retires nor
  instructs it; its verdicts are records no one but the judge writes and they reach the owner as they are. It may open a verification chain when it
  needs one: that chain has no judge of its own and the depth is capped at one, so judging does not recurse. By default a judge's template names a
  model other than the Chair's, because judges of one provider err together (`docs/archive/council/02-design.md` §2.5). Who judges a judge (calibration,
  seeded defects) is decided elsewhere.
- **Accountability.** The owner answers for the company. `Objective.owner`, `KeyResult.owner` and `KeyResult.judge` (ADR-0031) are required and
  name an `Accountable`: the owner or an employee. An owner field names an employee with title `chair` (the same one for a small objective), a
  judge field one with title `judge`. The default is the owner, who is then the one answering; a draft proposes employees and the owner approves
  them with it. A tier above the Chair ("head") is a title added the day one objective's key results sit with different Chairs and need someone
  to settle overlaps; it is not built now.
- **A card refers to an employee.** `Card` gains `Employee`, the id of the employee at work in that card; `Card.Agent` stays the template or role
  id (`chair`, `reviewer`, `intake`, `assistant`). `Employee` is empty for the cards Remit starts for itself: the Assistant, `intake`, `probe`,
  the Dispatcher and the built-in goal judge. `Chain.ChairID` names the Chair employee's card of that chain. `Chain.ProposedBy` names the employee
  that proposed it. A direct request has no standing Chair: its task objective is owned by the owner, and its chain is worked by a Chair of tenure
  `chain` as today.
- **Migration 58** makes each existing Chair, hired and seed card an employee with tenure `chain` (`Employee` = `e_<card id>`, `Card.Employee`
  set), retired when its chain is finished or archived. Cards Remit starts for itself get none. Library agents stay templates.

## Consequences

- The Chair and the judge stop being reserved ids made by code; the reserved set is `assistant`, `intake` and `probe`.
- `Chain.Serves` (ADR-0031) is worked by the key result's owner when it is an employee; a task is a chain that owner proposes.
- The Crew tab shows who answers to whom, which employees stand and which retired.
- Threat model (a new way to create principals; STRIDE):

| Threat | Control |
|---|---|
| Hiring as privilege escalation: a worker hired with more than its hirer holds | The ceiling check on every hire, a pure function with a test over the whole grid of titles and ceilings |
| A Chair removes or steers the judge that checks it | The judge is outside its tree; hire, retire and instruct are refused for that title; verdicts are append-only records |
| Runaway hiring: agents hire agents | Headcount and depth caps are rails; a hire past the cap is refused with its kind (ADR-0027) |
| A retired employee's memory is lost | Retire keeps it; only a forget the owner applies deletes |
| A worker acts as an employee it is not | Every action carries the employee id of the session that made it, set by the platform, never by the agent |
| A judge captured by the Chair it knows | A different default model, the report line outside the tree, verdicts as the judge's own records; calibration against sealed anchors is a later ADR |

## Not decided here

The memory itself (ADR-0033), the title "head", who judges a judge, pay or cost per employee (ADR-0010: nothing is recorded), and moving an
employee between managers. Four limits of the first build, each with its seam:

- An employee has one live turn at a time (`internal/orch/lease.go`). A turn an agent starts itself after background news is not leased, so two
  turns of one employee can overlap there; the lease is taken in `take`, where a second entry point would add it.
- A hired judge's card runs on `Config.JudgeModel` (`goal.go`), so the `model` of the shipped judge template is not read for it.
- The owner cannot create a Chair or judge template: `AgentInput` has no `title`, so a new template is a worker. A contract change adds it.
- An employee hired for a chain retires in a write of its own after the chain finishes, not inside the finishing change; a crash between the two
  (or a failed write, which is logged) leaves it active until the owner retires it with the Assistant; no start-up sweep exists yet.

## Revisit when

Chairs queue so long that parallel work is the owner's complaint; two turns of one employee are seen to overlap; the closed set of titles blocks a role the owner wants; judges and Chairs of
the same template are found to agree too often; or hiring depth beyond two levels is asked for.

Enforced by: (to be written first in each slice) TestAnEmployeeIsHiredFromATemplateAndKeepsItsRevision, TestAHireNeverHoldsMoreThanItsHirer,
TestAChairHiresNeitherAChairNorAJudge, TestTheJudgeIsOutsideTheTreeOfTheChairItJudges, TestAJudgesChainInheritsItsCeilingAndHasNoJudge,
TestAnEmployeeHasOneLiveTurnAtATime, TestRetiringKeepsTheRecordAndTheMemory, TestAnObjectiveAndItsKeyResultsAlwaysNameWhoAnswers,
TestMigration58MakesEachCardAnEmployee,
TestTheOwnerHiresAPersonThroughTheAssistantAsAStandingWorker
