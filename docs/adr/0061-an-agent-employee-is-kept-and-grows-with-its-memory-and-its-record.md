# 0061. An agent employee is kept, and grows with its memory and its own record

Status: accepted, 2026-11-04; the template match of the Chair seat and the reuse rule superseded by 0062; its memory superseded by 0063; its waits on the owner, but a secret act, are an agent's (ADR-0087). Source: the owner's words of 2026-11-04: too many temporary employees are hired and retired;
employees are to be reused so they develop over time; today they have no memory and cannot reach what they did, their
transcripts or their decisions. The owner chose that the Chair decides whom to reuse and justifies a new hire. Supersedes the
`tenure` of ADR-0032 and the `memory` capability of ADR-0033; builds on ADR-0048 (the Chair seat) and ADR-0052 (a Chair puts a
standing employee to work).

## Context

ADR-0032 made an employee outlive its chains, but gave a Chair's hire the tenure `chain` by default: it is retired when its
chain finishes (`retireChainEmployees`, `lifecycle.go`). A Chair made for a chain (`chairEmployeeOf`) is chain-tenure too. On the
dev instance on 2026-11-04 that left 101 employees retired, 17 active and no standing agent: nobody stays, so ADR-0052's reuse
has no one to reuse. ADR-0033's memory sits behind the capability `memory`, a writer only the owner gives and no playbook lists:
`memory_entries` holds no row. And an employee has no door to its own past cards: `read_chain` reads chains, behind `history`.

## Options

1. Keep `tenure`, default it to `standing`. Two kinds of employee stay, and the one that throws work away is still a field away.
2. Drop `tenure`: every employee stays until it is retired, by the owner's word. The Chair seat is taken by a standing employee
   of the seat's agent, made once. Memory is part of being an employee, and the employee reads its own record. **Chosen.**
3. Option 2, with the platform picking the employee to reuse by memory match. Hidden behaviour at the call site; the owner chose
   that the Chair decides.

## Decision

- **No tenure.** `Employee.Tenure` and the type `Tenure` are removed; every employee is kept until retired. A finished chain retires
  no one. Migration 84 strips `$.tenure` from every employee; active ones stay active, retired ones stay retired (a rehire
  brings one back, ADR-0032).
- **Each employee has a number, and may name itself.** Every employee gets a sequential number at hire (`E-42`), unique among
  all employees, retired ones included; migration 84 numbers those on record by when they were hired. An agent employee with no
  name has the tool `name_yourself` and its brief offers it: a short name, unique among all employees, fixed once chosen;
  nothing waits on it. An employee is shown as `Mai · E-42 (engineer)` when named and `E-42 (engineer)` when not; the card's
  name stays the Chair's label for the work. A person's name is the one the owner gave.
- **The Chair seat is kept.** A chain with no key-result Chair is seated by the active Chair hired from the seat's agent (the
  playbook's `chair`, or the shipped chair) whose ceiling fits; only when none exists is one hired, once, by the owner, and kept.
  One Chair sits in several chains at once, a card in each, as a key-result Chair already does.
- **The Chair decides whom to reuse, and says why when it hires anew.** `hire_agent` lists the active workers it may put to work
  first, each with its template, scope, open cards, last judged cards and the gist of its memory. A new worker of an agent of
  which a worker the Chair could put to work exists is refused unless `why` names that worker and why it is not the one. The
  headcount cap (`chairHireCaps`) now counts kept workers, so it bounds the staff a Chair grows.
- **Memory is Claude Code's own, one directory per employee.** Every session of a card with an employee runs with
  `autoMemoryEnabled` and `autoMemoryDirectory` set to that employee's directory under the data directory, so Claude Code
  writes and loads the employee's memory itself (`MEMORY.md` and one file per memory), across every chain and worktree; a
  card with no employee runs with it off. Checked with Claude Code 2.1.312: a session started from another directory answers
  from what an earlier one saved. Remit's typed memory (ADR-0033: `remember`, `recall`, `memory_entries`) is removed. The
  owner reads an employee's memory files in the Crew tab and through the Assistant, and only he removes one, by applying a
  forget the Assistant drafts.
- **An employee reads its own record.** `my_work` lists the cards it held, newest first (chain, title, deliverable, outcome);
  `read_my_card` gives one of them: brief, report, verdicts, the decisions on it and a page of its session's transcript. Reads are
  by the card's employee, set by the platform; no argument names another's.

## Consequences

- The Crew tab shows one kept employee per role and their growing record, not a column of retired chain copies.
- Staff grows only when a Chair justifies it; retiring is the owner's (`draft_retire`).
- The brief of every employee's card grows by the `my_work` hint, and, on a card with no Write tool, word that its memory is
  read-only there.

## Revisit when

- Kept workers sit idle in numbers: an idle-retirement rule the owner applies.
- One Chair holds too many chains at once for its context: a seat limit per Chair.
- The Chair should read a worker's memory in full before choosing (ADR-0033's team layer).
- Memory or a transcript is a channel into later sessions an outsider reaches: the threat model of ADR-0033 runs again, and
  source citation, stale and untrusted marks come back as checks Remit runs over the memory files.
- A card that holds no Write or Edit (a reviewer, a judge) reads its employee's memory but cannot add to it (checked with
  2.1.312); when such employees should learn too, give memory a door of its own rather than widening their tools.
- One session can read and write another employee's memory directory: the jail hides by a deny-list
  (`protectedFromAgents`). When memory must be private, the jail hides every employee's directory but the session's own.
- A person works in several chains at once (they hold no lease, ADR-0051): two of their sessions may write their memory
  directory, and its `MEMORY.md`, at the same time. When that loses a line, their sessions take turns on it.
- Claude Code changes how auto memory is configured or stored: the one function that builds the session's settings moves with it.

Enforced by: TestAFinishedChainRetiresNoOne, TestTheChairSeatIsKeptAcrossChains,
TestAnEmployeesSessionKeepsItsMemoryInItsOwnDirectory, TestOnlyTheOwnersForgetDeletesMemoryFiles, TestAnEmployeeReadsOnlyItsOwnCards, TestEveryEmployeeHasAUniqueNumber,
TestAnEmployeeMayNameItselfOnce
