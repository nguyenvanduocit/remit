# 0088. Chairs, the Assistant and mentors learn, and the owner reads each one's memory where he looks for it

Status: accepted, 2026-11-12; its backend superseded by ADR-0117. Source: the owner, 2026-11-12 ("why don't the Chairs show up in memory?"; "implement memory for
the Chair, and for the Assistant too, and for the mentors"; a mentor's memory is "what they teach"). Extends ADR-0063 (an
employee learns only what people and verdicts teach it) and ADR-0062 (the owner is a mentor, behind the Assistant).

## Context

ADR-0063 lets the memory backend learn only from teachings the core records as facts (`fact:teach.*`). Every teaching named
a worker's card: the Chair's verdict (`accept_work`) judges a team card, a merged edit names the MR's authors, a mentor's
steer is a message on the card they help. Nothing judged a Chair, so on the dev instance 41 Chair employees held no teaching
and no memory while 86 workers held 146 teachings. The Assistant learned only from overturns: the owner's own messages to it
are recorded as the owner's (`ActorKind::Owner`), not a mentor's, so `steers` skipped them. A mentor had no memory at all, and
the owner could read memory only on the Crew page, which lists employees: neither the Assistant nor a mentor is one.

The backend keeps how far it has read each session per role (`admit:<role>:<session>`), not per project: admitting one
session for two projects teaches the second nothing.

## Options

1. **Turn Claude Code's auto memory back on for Chairs and the Assistant.** Ungated memory written by any turn, the reason
   ADR-0063 replaced it.
2. **Teach each through a judge of its own work, inside the gate.** The goal judge's verdict is the verdict on the Chair's
   work; the coach's retrospective is a teaching outside a transcript; the owner's message is a mentor's steer, since he is
   the mentor `m_owner`; a mentor's own memory learns the words they taught with `learn`, not a second admit. **Chosen.**
3. **A mentor's memory as notes about the person** (strengths, how their help went). A new writer of judgements about people;
   their record already says how each help ended.

## Decision

- **The goal judge teaches the Chair.** `record_goal_verdict` stores `teach.verdict` for the chain's Chair card with the
  approval: `approve` is `accept` (the Chair's session admitted as accepted), anything else `rework` (only the judge's words).
  A check review records none.
- **The coach's retrospective teaches the Chair.** `write_retrospective` stores `teach.retrospective` (the chain, its Chair's
  card and employee, the retrospective) in the transaction that stores it, once per retrospective; the shipped subscription
  `sub_teach_retrospective` lets the backend `learn` what went wrong and the brief that would have served, taught by the coach.
  It is not a correction in `repeat_corrections`: the coach does not count its own teaching.
- **The owner's messages are a mentor's steer.** `steers` takes a prompt by a mentor or by the owner, on any card: a Chair,
  a worker, the Assistant. Each `teach.steer` names its mentor (`m_owner` for the owner, the help's mentor for a mentor), and
  `teach.overturn` names `m_owner`.
- **A mentor's memory is what they teach.** A teaching that names a mentor also `learn`s, for the project named by the mentor's
  id, the words they taught: the message's text, the overturn's reason with the decision it overturns.
- **Who taught it is named.** A sent-back card's teacher is the one who judged it (the Chair, the goal judge), by its card's
  name; a mentor by their name; the owner as "the owner".
- **The owner reads each memory where he looks for its owner.** An agent's drawer has a Memory panel when the agent has a memory
  (`AgentStanding::remembers`: an employee's card or the Assistant's), read with `card_memory`; a mentor's page shows what
  they taught (`mentor_memory`). The Assistant reads a mentor's with `read_mentor_memory`, and `list_mentors` gives a Chair the
  gist of what each mentor taught beside their record.

## Consequences

- Every Chair that finishes a chain is taught: by the goal judge on its finish, and by the coach once the chain is judged.
- Every message the owner types to an agent teaches it and his own memory, as a mentor's always did. Extraction spends more of
  the agents' Claude quota.
- An accepted chain admits the Chair's whole session, a long one: what it learns is lessons of running a chain.

## Revisit when

- The owner wants to forget a memory of the Assistant or a mentor: a forget names an employee today (`draft_forget`), and the
  forget item names whose memory it is.
- A mentor's words should reach the employees they mentor beyond the card they helped: the coach promotes a mentor's lesson.
- The backend keys its read position per project: a mentor's memory may then admit the session as the card's does.
