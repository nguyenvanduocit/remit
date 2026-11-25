# 0062. Every seat is an AI employee, and people mentor them

Status: accepted, 2026-11-04; where an agent's words to its mentor go amended by 0100; built by `docs/superpowers/plans/2026-11-04-ai-employees-human-mentors.md`. Source: the owner's
interview of 2026-11-04 ("in real life I have many positions, PO, Marketing, Design, Engineer, Director: how does that work with
Remit, or does Remit no longer need hard concepts?"; "a human employee is really an AI employee with a human behind it
supporting it"; "even the owner should be one of these: it decides on my behalf, and comes to me only when it is stuck").
Supersedes ADR-0051; supersedes the card shapes, `question` and hand-over to a person of ADR-0052 and ADR-0059; supersedes
the template match of the Chair seat in ADR-0048 and ADR-0061 and of the reuse rule in ADR-0061; amends ADR-0056 and ADR-0060
(the Work plugin and its codes serve a mentor). Builds on ADR-0032 (titles), ADR-0033 and ADR-0061 (memory and record),
ADR-0026 (the owner key), ADR-0027 (effects ranked by how far they can be undone).

## Context

A company has positions: PO, Marketing, Design, Engineer, Director. Remit has three titles, `chair`, `judge` and `worker`
(`internal/model/employee.go:9-15`), which say what an employee may do, not what it is good at. ADR-0051 to ADR-0060 made a
person an `Employee` of kind `person` who holds cards of their own, always as a `worker`
(`internal/orch/employee_change.go:170-190`), so a PO cannot judge a key result and a Director cannot sit in the Chair seat.
Adding a title per position would make every new position a code change.

Where Remit picks who works, a template stands in for what an employee can do: the Chair seat goes to the oldest active Chair
hired from the playbook's `chair` template (`lifecycle.go:627-652`, `employee_rules.go:124-133`), and a new hire is refused
beside an idle worker of the same template (`employee_rules.go:102`). The Chair reads each kept employee's scope, open cards,
last judged cards and memory gist (`employee_standing.go:105-116`), but a judged card says only its outcome and how long it
took (`EndedCard`, `employee_standing.go:134-139`), not what the work was, so a record cannot show a strength.

The owner answers every `ask_owner` and applies every change item himself (`requireOwner`, `employee_change.go:359`; 26 owner
checks in 19 files of `internal/orch`). He wants an agent that decides for him and reaches him only when it is stuck.

## Options

1. **A title per position** (`po`, `designer`, `director`…). Every reorganisation is a migration; a position says little about
   what one person can do.
2. **People stay employees, with more titles open to them** (a person as `judge`, a person as `chair`). Two kinds of employee for
   every rule, and a person in a seat stalls the chain whenever they are away.
3. **Every seat is an AI employee; people are a pool of mentors behind them, the owner among them; who works is chosen by
   strengths and record.** **Chosen.**

## Decision

- **No positions.** A position is words: an employee's or a mentor's strengths, in their own words, checked by their record.
  Titles stay the closed set of what an employee may do.
- **Every employee is an AI employee.** It holds the title, ceiling, loadout, memory and record, and does the work on Remit's
  runtime. `EmployeeKind` and the person employee are removed.
- **People are mentors**, an entity of their own (`Mentor`: name, strengths, availability, status), not employees. A mentor
  works from the codes the owner has made for them (ADR-0060) through the Work plugin (ADR-0056).
- **Strengths are self-declared and the record checks them.** An AI employee states its strengths with a tool of its own; a
  mentor states theirs from their plugin. A judged card names the work it was (`EndedCard.Work`), and a mentor's record lists
  the help they gave and how each card ended.
- **A mentor helps one card.** When an AI employee is stuck or faces a hard call, the Chair reassigns the work, or the AI asks
  the Chair (`ask_chair`) and the Chair brings a mentor chosen by strengths and record onto that card. The help is an entity of
  its own (`Help`: card, mentor, the problem, offered, accepted, declined, ended), carried in `ChainView` as Decision is. It
  ends with the card.
- **A mentor's message is a direct steer** of the card's own session: the AI does what the mentor says, within its own ceiling
  and rails, and the message is recorded as the mentor's. The card, its report and its record stay the AI employee's.
- **The Chair seat is matched by who answers, not by template.** The Chair that owns the key result the chain serves sits;
  else the Chair employee the playbook names; else the kept default Chair. A template is how an AI employee was built, never
  who is chosen. The seat holds its craft: what a Chair of an OKR chain must know is the playbook's brief, so it reaches whichever
  Chair sits, and no agent exists to be an OKR Chair.
- **The reuse rule is template-free.** A new hire while any kept worker is idle needs a `why` naming one idle worker and why its
  strengths do not fit.
- **The owner is a mentor, behind the Assistant.** The Assistant is the owner's AI employee: it acts on his words when he is
  there and decides for him when he is not. A Chair's `ask_owner` and every change item reach it first; it answers or applies,
  or, stuck, asks the owner as its mentor. The owner talks to it as any mentor talks to the card they help, through one
  mechanism: the Assistant's panel in the web app is one client of it, Paseo or any other client another. The panel keeps its
  look.
- **The Assistant may do everything the owner may, except secrets.** It applies change items, pulls and lifts pauses and stops,
  and moves ceilings and rails, each recorded as the Assistant's with its reason. What stays the owner's alone: typing a
  credential (the write-only inbox field), the owner key, and reading a key's or a code's value. One function decides what the
  Assistant may do for him, so narrowing it later is one change.
- **The owner sees and overturns.** Every decision the Assistant makes for him shows in its feed, where he looks for it, with
  its reason; he can overturn it, and an overturn is recorded and becomes a lesson in its memory. The Assistant learns his taste
  from principles he states and from his record: his answers, Applies, Not nows and overturns.

## Consequences

- PO, Marketing, Design, Engineer and Director are mentors described by their strengths. A PO's or a Director's judgement reaches
  a chain through the AI judge or Chair they mentor; no title opens to a person.
- Every time a person steps in is recorded on the AI employee's card, which is the data that improves the AI employees.
- The person-as-employee path goes in the same change: the person launcher, the `decision` and `session` card shapes, offers on
  a person's card, `question`, hand-over to a person, `personBrief`, `ask_lead`, and a person's hire through `draft_hire`.
  Existing person employees become mentors in one migration, keeping their id, codes and availability.
- **Risk the owner chose:** the Assistant reads text from outside in what Chairs ask (MR comments, posts, a source's content). An
  injected line that reaches it can lift a stop the owner pulled or raise a ceiling. A stop that only the owner can lift is no
  longer guaranteed. The bound is the record and the overturn, not a rail.
- A code the owner makes for himself, to talk to the Assistant from Paseo, opens everything the Assistant may do for him: a
  stolen one acts as the owner in all but secrets. The owner key stays the only way in to the web app (ADR-0026).
- A Chair's question waits on the Assistant, not on the owner: chains block on the owner only when the Assistant is stuck.

## Revisit when

- An injection through a Chair's question moves a rail, or the owner asks for hardening: narrow the one delegation function
  (lifting a stop the owner pulled, raising a ceiling) and run `aio-threat-models` on the Assistant's inputs.
- A mentor is needed across many cards of one AI employee: a standing pair of mentor and employee.
- Strengths stated by AI employees drift from their record: the platform derives strengths from the record and shows both.
- A mentor's steer conflicts with the AI's ceiling often enough to slow work: a ceiling the mentor brings, granted by the owner.
- Non-code work (posts, designs, campaigns) cannot pass the submit path a chain uses for code: a deliverable kind of its own.
- The owner's inbox fills with items the Assistant then answers: an item is his from the start and the first answer closes it
  (`fireAssist`); hide it from him while the Assistant decides, with a time after which it shows.
- An answer stands but its feed line is missing: the Assistant's decision (`recordAssist`) and an overturn are written after the
  answer, in a step of their own; write them in the answer's transaction.
- Undoing on an overturn goes wrong: the asker is told to undo what followed from the Assistant's answer, by its own judgement;
  an Apply the owner overturns is reverted by Remit (the change's before is stored with it).

Enforced by: TestTheChairReadsWhatEachJudgedCardWas, TestAnEmployeeStatesItsStrengthsAndTheChairReadsThem,
TestANewHireBesideAnyIdleWorkerNeedsAWhy, TestANewHireBesideAnyIdleWorkerNamesThatWorker,
TestAPlaybookNamesTheChairEmployeeThatSits, TestAPlaybookThatNamesNoChairSeatsTheOldestKeptChair,
TestAPlaybookCannotNameAChairThatIsNotAtWork, TestAChainOfAPlaybookWhoseChairRetiredFailsToStartSayingWhy,
TestTheOKRCraftReachesWhicheverChairSits, TestThePlaybookAuthorListsTheChairsAPlaybookCanName, TestMigration86NamesTheChairEmployee,
TestTheOwnerAddsAMentorThroughTheAssistant, TestAStuckWorkerGetsAMentorOnItsCard, TestAMentorsMessageSteersTheCardsSession,
TestAMentorReachesOnlyTheCardTheyHelp, TestAnUnansweredHelpEndsAtItsDeadlineAndTellsTheChair,
TestTheChairOfAWaitingChainNamesTheMentorsHelpingIt, TestMigration87MovesPeopleIntoMentors,
TestTheAssistantMayDoForTheOwnerEverythingButSecrets, TestOnlyTheOwnerOrTheAssistantForHimLetsAChangeThrough,
TestAChairsQuestionReachesTheAssistantFirst, TestAStuckAssistantLeavesItToTheOwner, TestTheOwnerOverturnsAnAssistantsDecision,
TestTheOwnerTalksToTheAssistantAsAMentorFromAnyClient
