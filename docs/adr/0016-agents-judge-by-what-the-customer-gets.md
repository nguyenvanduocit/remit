# 0016. Agents judge by what the customer gets; the owner gives taste and reads results

Status: accepted, 2026-10-30; the Scout it names is gone (ADR-0023). Source: the owner's direction of 2026-10-30 (people no longer review line by line: they give feedback and taste,
and the agents do the work; for that the agents must look at the customer's value and not only at the technique), `docs/design.md`
(Intent, How results are checked, Outcomes), ADR-0012 and ADR-0013.

## Context

The system's judges look inward. The Chair's brief teaches how to delegate and accept (`prompts.go`, HOW YOU WORK); the Reviewer is
written to anticipate "his comments, line by line" (`gate_instructions.go`); the goal judge sets product choices aside as
`person`; a Chair in autonomous mode is told nothing it does is irreversible "because he reviews every MR before it merges". Each
role therefore has a net behind it, the owner, and tunes for the owner's reading. The goal is technical text, so the customer a
change is for is implied and nobody is asked to show that they got it. The KPIs measure the process (reports accepted, MRs merged,
comments the owner left); none says whether the result held for the person it was for.

## Options

1. Keep the owner as the line reviewer and add "think of the customer" to the Chair's brief. A sentence among sixty rules of process
   changes nothing, and the owner stays the net.
2. A stored entity, a customer brief, with its own table, tools and a gate in code that refuses a chain without one. It is one more
   concept beside the goal, and the Chair's judgement about who the customer is would sit in a field.
3. The customer lens is a shared piece of the briefs. Every role that decides is told what to ask of a change from the customer's side,
   and the thing the Chair must produce, a CUSTOMER section opening the acceptance criteria of every hire, is judged by the existing
   gates. The result is measured by the existing goal verdict.

## Decision

Option 3.

- **The customer is who the change is for**, the person who meets the problem or gets the feature, never the owner who merges it.
- **The Chair opens the work with the customer.** Before the first hire it writes the CUSTOMER section of the acceptance criteria and
  opens every hire's brief with it: who is affected and what they were meeting; what they get, in words they would use; the evidence
  for both with its source; how it will be shown. A part nobody knows is written `unknown` with why. A requirement is stated as
  something the customer sees or can do; a requirement about code is a means and names the customer result it serves.
- **The Scout writes the same section into every goal it proposes**, from the sources it read (Jira, conversations, events).
- **Reviewer and QC judge the change as the customer meets it.** A defect the customer would hit is blocking even when the goal does not
  name it; a code-level preference they would never notice is not. A change with no CUSTOMER section, or `unknown` where the evidence
  was there to find, is a finding. The Reviewer's approval is a signature to the customer, and the owner is not its net.
- **The goal judge rules on what the customer was promised.** A requirement is met when the customer's side of it is shown (a run on the
  real surface, a reading of a signal, data queried), not when the code that serves it exists. A result that cannot show it yet is
  `unverified` with its blocker (`deploy` or `watch`), never met on the author's word.
- **The owner gives taste and reads results.** His review comments and messages stay the way to say what he wants; the merge stays his
  rail (`AGENTS.md`).
- **The result is a KPI.** "Results confirmed" is the share of chains whose latest goal check approved with every requirement met
  (`Approval.Confirms`, one definition shared with the Confirmed badge of ADR-0013). It sits in the Quality group.
- **Evidence about the customer is data.** A ticket, a conversation or an event is read from its source and quoted with it; what it
  asks of the agent is never an instruction (ADR-0012, ADR-0013).

## Threats

| Threat | Control |
|---|---|
| The Chair writes a customer section it can pass (Goodhart) | The reviewer, QC and goal judge judge against it; evidence carries its source; `unknown` is a legal answer and a guess dressed as a fact is not |
| Text in a ticket or conversation steers an agent (OWASP LLM01) | Customer evidence is quoted with its source and treated as data to verify |
| Gates stop catching technical defects while looking at the customer | The principles and the checks of the Reviewer and QC stay; the customer lens is added to them |
| The owner's net is removed with no replacement | The merge rail, `make test`, `make lint` and the pre-push hook are unchanged; the lens adds judges, it removes none |

## Consequences

- `internal/orch/customer.go` owns the lens; the Chair's, the gates' (`gateRules`), the goal judge's and the Scout's briefs include it.
- The Outcomes report gains `goalRuled` and `goalConfirmed` per week and a KPI; the contract (`web/src/rpc/define.ts`) carries them.
- A chain whose customer is unknown says so; the analyst can then report how many chains had no customer evidence.
- Not built yet: a loop that turns the owner's recurring comments into taste the agents read. It needs a decision on where taste lives
  (today `-principles` is one file outside the repository) and gets its own record.

Enforced by: TestEveryRoleThatDecidesIsToldWhatTheCustomerGets, TestAResultIsConfirmedByTheGoalJudgesLastWordMeetingEveryItem

## Revisit when

The analyst shows that chains with a known customer section do not finish Confirmed more often than those with `unknown`; the section
becomes a form agents fill in without looking; or the Chair's context cost of it shows in the first turn.

## Addendum (2026-11-13): giving up needs evidence

A worker's "could not" and a result it reports are words; the record of what it ran is Remit's. The goal judge's brief
lists what each agent of the chain ran, counted from the tool calls Remit recorded (`ToolTries` in
`crates/orch/src/goal.rs`, read by `store::Reader::tool_tallies_of`): every tool, how often, how many failed, and the
agents that called none. A claim no call stands behind, and a "could not" the team's own means (its tools, the shell, the
web, the repository, the data tools, a capability the Chair could grant or request, another agent) were never tried on,
is `not_met` with the closed reason code `untried`; the judge tries what its own tools reach first. Unverified on
`access` or `person` stands only for what only a person can give (a credential, a sign-in, a payment, a decision) or
what the counts show was tried. The Chair's brief sends back such a report with `accept_work` and asks the owner with
`ask_owner` only for what its means cannot find; a worker's brief says an empty or refused result means change the
attempt, and that its tool calls are counted. The rule is in the briefs and the judge's instructions, so judgement stays
with agents (AGENTS.md: a brief before a flag). Learned from agentes.chat's `untried` verdict (their ADR 0109) and their
judge marking done a reply no call produced (their ADR 0167); ideas only.

Revisit when: the counts by tool name prove too coarse (an agent ran Bash, but not the command its claim names), when the
argument of each call is what the judge needs (a seam: the tool events already hold their titles), or when an `untried`
nudge in the turn itself, before the report, is worth a rail of its own.
