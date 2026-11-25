# 0031. An objective is a set of key results the owner approves, and the agent that takes a request decides whether it needs one

Status: proposed, 2026-11-02; a key result refines into key results of its own (ADR-0115); its waits on the owner, but a secret act, are an agent's (ADR-0087). Source: the owner's design research (`docs/archive/council/02-design.md` §2.2 to 2.6 and §6,
`docs/archive/ai-run-company-blueprint.md` §3.3 and §3.6, `docs/archive/brainstorm-ai-autonomous-system.md`) and the owner's words of 2026-11-02: a big request
arrives half-formed and an agent helps him analyse and design the OKR for him to approve; a small request ("change this text colour") is just
done; the agent that takes the request classifies it; the owner answers for the company, an agent for each objective, a Chair for each key result.
Builds on ADR-0013 (a requirement carries its check), ADR-0016 (the customer opens the chain), ADR-0027 (a refusal names its kind), ADR-0029 (a
change the owner applies), ADR-0032 (the employee: Chair and judge are hired) and ADR-0033 (memory).

## Context

The research says the owner states one objective and agents do the rest: a contract (who the customer is, what they get, the outcome,
north-star, guardrails, surrogates, horizon, what is unknown), key results each with a check a machine can run, a deterministic checker whose
findings carry a reason, an owner queue for what the author cannot settle in one answer, and nothing that blocks the work.

Part of that exists. ADR-0013 has each requirement of a chain's goal carry how it is checked, the goal judge review the check before work
starts, a verdict of `met`, `not_met` or `unverified` per requirement with evidence, and a chain whose window ends with no reading finish as
unverified. ADR-0016 opens a chain with the customer. `intake.go` has an agent read the owner's words and choose the chain's playbook and
title. What does not exist: a requirement is a sentence inside `Chain.Goal`, so nothing can count it or name it in a finding; the contract's
other parts have no home; no object outlives a chain, so a big objective that many runs serve has nowhere to live; a finding has no reason
code that code can group by; and the owner has no step where an agent turns a vague big request into an OKR for him to approve.

A `KeyResult` entity beside the goal's requirements would put one fact in two owners.

## Options

1. Improve briefs only and keep requirements as prose. Cheapest, but no figure (how many key results cannot be verified, how often a draft is
   sent back) can be computed, and a finding cannot name what it is about.
2. Objective and key-result entities beside `Chain.Goal` and its prose requirements. Two owners of one fact.
3. The objective is an entity of its own (as Decision is); its key results replace the prose requirements; a chain serves a key result; the
   agent that takes a request classifies it, and a big one is shaped into a draft the owner approves. **Chosen.**

## Decision

- **Classification is the intake agent's.** `intake.go` already reads the owner's words and chooses the playbook; it also chooses `direct`
  (do it) or `shaping` (an objective is needed first), and Code settles what it chose, as it does the playbook (`chooseCmd`). The owner
  overrides in words ("make an OKR for this", "just do it"); no setting and no form. A playbook the owner named stays and the request is
  `direct`; `shaping` chooses the built-in playbook `objective`, which lists the capability `objectives`. A chain that skips intake (an outside
  event, `propose_chain`, a routine) is `direct`. (My call.)
- **Direct requests stay direct.** A direct chain is one run with one key result, the owner's words, checked as ADR-0013 says. It gets a
  *task objective*: an objective with no scope, hidden from the objective list, so runtime code reads one shape.
- **Shaping is a chain.** A chain whose playbook lists the capability `objectives` (`internal/orch/capabilities.go`) analyses the request and
  drafts an objective with the Chair's tool `draft_objective`. It drafts first and does not interview: each unknown goes in the objective's
  `unknown` list with the question that would settle it, and the chain asks the owner (the existing `question` inbox kind) only when an
  unknown blocks a key result's check. (My call; the owner has not chosen between asking first and drafting first.)
- **The draft is a change the owner applies.** It reaches the owner as a change item of a new kind, `objective`, quoting the owner message it
  came from. He corrects it by talking to the agent (each correction a new version of the draft); Apply is the owner's own answer in his inbox
  (`answerChange`, ADR-0029): the Assistant drafts and tells him which item waits, and never applies. Apply records who acted and makes the
  objective active. The item also carries the employees the draft proposes (a Chair and a judge for each key result, ADR-0032), and Apply hires
  them in the same write. A draft has no chains.
- **Objective.** Versioned: each write is a new version made over the revision it read (as `Chain.Rev`). Fields: `scope` (the owner's name for
  it, "game factory", empty for a task objective), `customer`, `outcome`, `northStar`, `guardrails`, `surrogates` (ids of outcome KPIs,
  `internal/outcomes`, each with an owner, the one kind of change it reads and an expiry), `horizon` (with an expiry: a stale objective shows
  as stale), `unknown`, `owner` (the accountable agent, below), `status` (`draft`, `active`, `closed`), `keyResults`. It is stored in its own table, carried in `ChainView`, pushed by
  its own notification and defined in `web/src/rpc/define.ts`. There is no product entity: a scoped objective is what the owner calls a
  product or a project. A tier above it is a parent pointer added the day a second tier is needed.
- **Who answers.** `Objective.owner`, `KeyResult.owner` and `KeyResult.judge` are required, each an `Accountable` (ADR-0032): the owner or an
  employee, a Chair for an owner field and a judge for the judge field. The default is the owner, and the owner is the answer when a draft
  proposes no one. The draft proposes employees and the owner
  approves them with the draft. The company is the owner's: legal and boundary responsibility stays human. A task objective's owner and judge are
  the owner.
- **Key result replaces requirement everywhere.** `KeyResult{ID, Text, Owner, Judge, Check}`. `Check` is a closed set, each with its fields: `submit{how}`
  (a document or test the judge verifies at submit, ADR-0013), `reading{signal, condition, window, expiry}` (the parameters `subscribe`
  takes today, ADR-0013 and ADR-0024; the Chair still calls `subscribe`, and the check records the subscription it made), `command{repo,
  name}` (a check the repository declares, ADR-0025), or `unverifiable{reason}`. A key result may be a plain pass-or-fail condition. An
  `unverifiable` one still runs; the share of them is a figure the owner reads. (My call: the fields.)
- **The check of a task objective's key result** starts as `submit` with `how` empty; the Chair states how it will be checked in its first
  turn (the one write a Chair makes to a task objective, a new version), and the goal judge reviews that check before work starts, as ADR-0013
  says. Today the goal judge runs only at finish (`goal.go:407`); the pre-work review is part of this change. It blocks nothing and adds no chain
  state: a finding is recorded, the Chair revises the check once, and a finding still open becomes an exception. (My call.) When a key result's
  judge is the owner, Remit starts the built-in goal judge for the chain as it does today; a hired judge runs the same check, so the check
  runs for every chain either way.
- **A chain serves one key result.** `Chain.Serves` is an objective id and one key-result id; a key result may have many chains. The Chair's
  goal for the chain is that key result plus what the owner said. A key result's status is derived by a pure function and never stored: from
  the latest goal-judge verdict of each chain that serves it, the last word wins (as `standing.go` already rules): any `Confirms` makes it
  `met`; otherwise a `not_met` finding makes it `not_met`; otherwise a chain with an open watch or an unverified finding makes it
  `unverified`; with no serving chain it is `unserved`; a chain with no goal check yields no verdict and counts as `unverified`.
- **Who starts the chains that serve key results.** After Apply, the shaping chain finishes: its product is the objective. The chains for
  its key results are opened by their owners with the existing `propose_chain` (ADR-0001's caps apply), each with `Serves` set, and start without
  a second word from the owner (my call; the owner did not answer). After Apply the platform wakes each key-result owner for a turn whose one job
  is to open those chains; that is the owner's own turn, so the rule that a proposed chain cannot propose chains (`capabilities.go`) does not
  apply. A key result whose owner is the owner himself starts no chain until he tells the Assistant to open one, which uses the Assistant's
  existing chain start with `Serves` set.
- **Findings.** `model.Finding` has `KeyResult` (the id the finding rolls up to; for a direct chain, its only one) and `Item` (what the judge
  checked, free text, the sub-requirement or defect class as today), so a judge's finer lines stay and no judge writes to an objective.
  Findings also carry `Code`, a closed `ReasonCode` (`uncovered_key_result`, `unverifiable_key_result`, `unknown_metric`, `overlap`,
  `guardrail_breach`, `evidence_contradicts`, `process_constraint`, `duplicate`, `other` with text), and `Refs`, a list of
  `EvidenceRef{kind, id}` whose id is a platform record (a `source_calls` row, a signal reading, an event), never text an agent wrote. `Evidence`
  stays the judge's text proof (`model.go:183`, ADR-0013); `Refs` corroborate it, and a finding coded `evidence_contradicts` carries at least
  one.
  `FindingBlocker` stays: it says what stops a key result from being verified now, `Code` why a reviewer objected.
- **The draft is checked twice, neither by its author.** A pure function `Check(objective, now) []Finding` computes what code can:
  `unverifiable_key_result`, `duplicate`, `unknown_metric` (a surrogate id not in `internal/outcomes`, or a `reading` signal with no
  subscription), and a guardrail breach for guardrails stated as `{kpi, bound}`. The hired judge of the key result (ADR-0032), outside the
  shaping chain, rules on what needs reading: `uncovered_key_result` (the outcome is not covered),
  `evidence_contradicts`, `process_constraint`. The owner cannot apply a draft the pure checker still faults (ADR-0045). `overlap` belongs to claims, decided elsewhere.
- **One answer, then the owner.** The author answers a finding once, in the shaping chain, recorded on the finding; a finding still open after
  that becomes an inbox item of kind `exception`, the one place anything waits for the owner. A change touching a number a customer relies on
  still reaches production only through the owner's merge.
- **North-star, guardrail and surrogate are read, not rewarded.** North-star is read quarterly; a guardrail raises an alarm (a notification);
  nothing that grants, ranks or pays reads an agent's own key results, and a test fails when such a package reads them.
- **Life of an objective.** `draft` becomes `active` by Apply and `closed` when the owner applies a close, a change item of the same kind
  that the Assistant drafts on his words. A change to an active objective is a new draft, applied the same way; a key result it removes is `retired`:
  the chains serving it finish, none is stopped, and its Chair is woken. A passed horizon marks the objective stale on the board and in the
  Assistant's briefing; it closes nothing by itself. (My call.)
- **Where the owner sees it.** The Work area (ADR-0019) gets an Objectives tab: a read-only tree table, Objective → Key Result → Chain, one row
  each, with columns for owner, derived status (never stored), horizon and scope label. Drafts are marked as drafts and also wait in the inbox as
  the change item; filters and the chosen view live in the URL. The Assistant's floating panel, on every page, is the only way to create or change
  anything. There are no weights, no roadmap and no chart. Task objectives are not listed. The chain's own page shows the key result it serves.
  (My call; needs a `web/verify` page and a look in a browser, with keyboard and contrast checks, before it is done.)
- **Migration 57** gives each existing chain a task objective (owner and judge: the owner) with one key result, its goal text, check `submit`; every stored finding
  keeps its requirement text as `Item`, names that key result, and the contract field `requirement` becomes `item` and `keyResult`. No chain
  gets many key results, so no near-duplicate is created.

## Consequences

- The judge's brief, the Chair's brief and the chain's page speak of key results; the word requirement is gone from them, from ADR-0013's
  live text, from the contract and from the tests, in the same change.
- Figures the board can show, none of them a target: key results by status, the `unverifiable` share, drafts sent back, open exceptions,
  objectives past their horizon.
- A chain started from an outside event (a Dispatcher, a client key) gets a task objective like any direct chain; its customer field names
  the sender's customer, not the sender, and what the sender said stays an unverified hypothesis (ADR-0016).
- The owner defines an objective only by talking to an agent. Nothing edits one in a form.
- Threat model (new agent tools and an owner-facing queue; STRIDE and OWASP LLM01):

| Threat | Control |
|---|---|
| A draft sets key results the chain can pass (Goodhart) | The checker flags `unverifiable_key_result` and `uncovered_key_result`; the judge reviews each check before work starts (ADR-0013); the owner reads the draft before it is active |
| Text in an outside event steers the objective | The draft quotes the owner's message; an event's claims are unverified hypotheses; evidence references are platform-issued |
| An agent activates its own draft | Apply is the owner's own answer in his inbox, recorded with who acted; neither an agent nor the Assistant (which reads outside content) applies |
| The queue floods the owner | One answer per finding and one exception per finding; the cap on what waits for the owner (ADR-0001) holds |
| A guardrail becomes a target | An alarm is a notification with no reader that grants or ranks; a test greps for it |

## Not decided here

Role-level key results that roles propose for themselves and the calibration figures over them, claims with overlap reports, typed shared
context and long-term memory (§2.4), the auditor panel and sealed anchors (§2.5), surrogate evidence and calibration beyond the field, and
the owner's legal register. Each is its own ADR. Three limits of the first build, each with its seam:

- A retirement tell to the owners of a key result is recorded as a `DecisionEvent`; a decision kind of its own would let the chain view group it.
- The `guardrail_breach` reason code follows the grammar of `objective.go` as built; the exact intent of §2.2 is checked against the first guardrail
  an owner writes.
- The objective view runs one `Approvals` query per serving chain on every snapshot; a store query that counts for all chains at once replaces
  it when a snapshot is slow.

## Revisit when

The intake agent classifies badly (big requests treated as direct, or small ones sent to shaping) often enough that the owner overrides it
more than he lets it decide; drafts reach the owner mostly as `unverifiable` key results; the exception queue fills faster than he reads it;
a second tier above the scope is asked for; or the objective turns out to be one more thing agents fill in by rote and a brief would have done.

Enforced by: (to be written first in each slice) TestAnObjectiveIsVersionedAndWrittenOverTheRevisionItRead, TestIntakeChoosesDirectOrShapingAndCodeSettlesIt,
TestADirectChainGetsATaskObjectiveWithOneKeyResult, TestADraftHasNoChainsUntilTheOwnerApplies, TestAKeyResultsStatusIsDerivedFromItsChainsVerdicts,
TestAFindingNamesAKeyResultAndAClosedReason, TestTheCheckerIsPureAndBlocksNothing, TestAFindingOpenAfterOneAnswerBecomesAnExceptionInTheInbox,
TestNothingThatGrantsOrRanksReadsAnAgentsOwnKeyResults, TestMigration57GivesEachChainATaskObjectiveAndRenamesTheField
