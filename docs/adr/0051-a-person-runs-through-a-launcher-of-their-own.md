# 0051. A person runs through a launcher of their own; only their reply is the report

Status: superseded by 0062, 2026-11-04 (accepted 2026-11-03). Source: the owner's approval of the human-employees design
(`docs/superpowers/specs/2026-11-03-human-employees-design.md`, decisions 2, 3 and 9): engineers work in Remit as employees,
exactly as AI employees do, through their own tools. Extended by ADR-0059: a person's work card runs a hosted session, whose
turn end is no report; this launcher stays a person's question's.

## Context

Remit starts every agent through `agent.Launcher` (`internal/agent/agent.go`), and the path from a card's queue to its Chair
runs through the session it returns: `take` opens a turn on the next message, `Prompt` waits for the turn's end, the turn's message
text is the report (`appendText`, `stream.go`), a report with plan steps open gets one more turn to update the plan (`endTurn`,
`session.go`), and `accept_work` answers it. Rules beside that path assume a process: the session runs on an account and moves to
another when the provider fails (`bindAccount`, `failover.go`); a silent turn is cut; an employee holds one live turn; a restart
marks running cards stopped; an open turn holds the chain's slot.

A person (an `Employee` of kind `person`, ADR-0032) works with their own tools, Claude Code among them, and answers in their own
time. Their own AI ends many turns before the person is done, and none of those ends may reach the Chair. ADR-0052 lets a Chair put
a person to work; their card needs a run.

## Options

1. A card kind beside runs: a person's card has no session, and the queue, turn, report, rework and `accept_work` get a second
   implementation for it. Every later change to that path would be made twice.
2. A second path in the orchestrator, branching on the card at each step. The branches would spread through every rule that reads
   a run.
3. A launcher of its own, `agent.PersonLauncher`, behind the `agent.Session` every card already has. **Chosen.** The path from
   queue to `accept_work` is the one every card takes, and each rule that assumes a machine names its exemption in one place.

## Decision

Option 3.

- **The session.** `Start` returns at once and the person's device binds to the card when it connects; `Prompt` opens a turn and
  returns only on the person's `Reply`, with no deadline; `Steer` says whether a turn is open; `Close` ends the session and its
  turn. The launcher carries no message text: the device reads a card's messages off its events.
- **Whose card.** A card is a person's when its employee is (`isPerson`, `internal/orch/person.go`), read once when its run is made
  (`open`, `outbox.go`) and never copied onto the card.
- **Only the reply is the report.** `Reply` hands the handler the turn's message, then ends the turn, so the report the Chair reads
  (`report`, `session.go`) is the person's words. A person is asked for one reply: no plan-sync turn follows it, and the report
  carries the plan as it stands (`planText`).
- **Delivery and availability.** How far a message to a person got is kept on its own `prompt` event, in its data: when it first
  reached one of their devices (`reachedAt`) and when they first saw it (`seenAt`), both absent until a device tells of it
  (`model.Delivery`, `DeliveryOf`). `MarkDelivery` (`internal/orch/person.go`) keeps each step's earliest time, so a device that
  tells a step again changes nothing; a seen message reached a device then, if no earlier word says so. It refuses an event
  that is not a message to the card named. Whether a message waits in the queue is the event's status alone: a message sent
  again (a restart, Resume) waits there again with its delivery, since the person has it already. The requeue takes that
  delivery from the event as stored in its own transaction (`queueMessage`, `internal/orch/outbox.go`), so a mark stored after
  the line read the message is never written over. What a person says of when they can work is a field of their employee
  (`Employee.Availability`, a note and when they said it). Each is one record pushed as its owner is (`EventUpdated`,
  `EmployeeUpdated`), so no table of deliveries or availabilities stands beside them.
- **Presence.** Whether a person is connected is held in memory only, as a restart closes every socket: `DeviceOnline`
  (`internal/orch/person.go`) counts the sockets each device key holds, and the `offline` it returns counts once however often
  it is called, so a device that reconnects before its old socket closes stays connected. A Chair reads presence, availability,
  each open card's messages by how far they got (queued, delivered, seen) and the last verdicts in `list_agents`' standing
  (`standingSummaryOf`, `internal/orch/employee_standing.go`), and presence and availability in `hire_agent`'s description.
- **What assumes a machine leaves a person alone:** account binding, waiting for an account and provider failover (`launch`,
  `session.go`; `unreachable`, `failover.go`); the silent-turn cut and the stall rule (`owesWord`, `schedule.go`: a person's
  open turn is never cut and is the chain's progress); the employee's one live turn (`takeLease`, `lease.go`: a person takes
  no lease and holds open turns in several chains, their memory being entries the store appends); the restart rule that stops
  running cards (`recoverCards`, `recover.go`: a person's card in a chain not finished is left as it is, and their open turn
  opens again on the message it was on, `ResumeInterrupted` sending that message again, with no restart message to answer);
  and the slot and wait rules (`othersBusy`, `wait.go`: a person's open turn
  and a message queued for them neither keep the Chair from subscribing nor its wait from starting; a chain whose only open
  turns are people's waits on them when its Chair's turn ends, with no check-back (`turnEndedCmd.people`, `step.go`), and the
  owner's decisions record it as a check-back's start is, "Wait on Lan, Minh."; a message that ends it woke it, said plainly
  ("A message woke the wait: Lan.") since no check-back was held for it to come early to; its
  sleep closes every session but a person's (`sleepSessions`, `apply.go`) and frees its slot; its Chair's line names them,
  "Waiting on Lan, Minh" (`peopleAtWork`, `agentstanding.go`); their reply, a report to the Chair, wakes it, and so does their stop: a person stopped in an open turn will not reply, so `stop` (`lifecycle.go`, `owedReply`) tells the Chair in the same change, and a paused chain stays paused). Pause, stop and the chain's finish close a person's line, the rails staying deterministic (*A person's line*).

## A person's line

A person's open turn is a line to them that only their reply ends, and it must outlast what cuts the turn short: a restart, a
pause, a stop. Their device reads each close and reopen off the card, so what the card says, whether a reply is owed and on which
message, and what Resume, a restart or a stop does, must never disagree.

Options weighed:

1. Derive the line from what is already stored: the card's status, the newest message the card took (`store.LastTaken`) and the
   newest `turn_closed` or `turn_reopened` event, with the turn ends after it. Three sources that disagree while the line's message
   waits in the queue again (a restart or Resume sends it on): a stop then named an earlier, answered message, or found no line and
   told no one; a reply in flight as the pause stored its close needed a rule over turn ends (`after`) to be told from an earlier
   turn's end. Three review rounds each found a new state where it broke.
2. One derivation from the prompt rows alone. A prompt row says queued, delivered or steered, never that a pause closed the line
   or that a stop ended it for good: the pause would still need an event read beside it.
3. **Chosen.** One field of the card, `model.Line` (`internal/model/line.go`): the message the person owes their reply to and its
   state, `open`, `paused` or `stopped` (closed for good), the zero line owing nothing. It is card state like status and plan, so it
   is written as they are, on the card's latest copy under its run's lock, in the transaction of each move.

The moves, each in its own change:

- `take` (`outbox.go`) opens it on the message a person's card takes, and takes nothing while the chain is paused, decided on
  the chain as stored in the take's own transaction (`openToTake`).
- The reply clears it when it answers the line's message (`endTurn`, `session.go`, `model.Line.Answered`); a reply to an earlier
  message leaves a newer line owed. The answer is stored with the report, in one change, so a restart before the report sends
  the message on again. From the turn's end to that store the card's run holds it (`run.replied`), under the lock every move of
  the line takes: a pause or a Resume stored in between moves the line without a word (`lineMoveOf`), so the device is never told
  that a turn closed or opened again on a message the person answered, and the message is not sent again.
- The pause closes each open line and puts the card idle, "paused" (`pausedLine`, `person.go`); the pause's change cuts the
  turn on a line it closed, under the chain's lock, before a Resume can open the line again (`cutPausedLine`, from `endSessions`,
  `apply.go`), and that turn sends no report.
- Resume opens each paused line again (`reopenedLine`).
- A stop closes an owed line for good (`stop`, `lifecycle.go`); when the line owed a reply, or a message waited for them, the Chair
  is told in the same change (`owedReply`), in words for the case: their open turn ended with no reply, or the message waiting
  for them will not be read. The stop closes the card's session and stores its change under the card's lock (`lockSession`,
  `session_lock.go`), which a dispatch (`dispatch`, `outbox.go`) holds from reading the card's queue to storing the card
  starting, and a dispatch opens nothing for a card with nothing queued. So a session a dispatch opens (`redispatch` as the
  chain goes to wait) comes before the stop, which closes it, or after it, which reads the card stopped with its queue dropped
  and opens none: no `starting` is written over a stop or from a copy read before it, for every card. The chain's finish closes every owed line for good and ends the work still open (`finishedWork`).
- Pause, Resume and the finish move the lines in their own change, from `applyLocked` under the chain's lock (`linesMovedBy`):
  they edit every person's card of the chain and decide which line moves on its latest copy under its run's lock, never on a
  copy read before. A take stores its open line under the same run lock and refuses a paused chain, so a line opens either
  before the pause, which closes it, or after the resume.
- A restart keeps it: `Recover` notes an open line (`notePersonLine`) and `ResumeInterrupted` sends its message on again; a paused
  line waits for Resume.

What a move says and does beside the card is read off the move itself as `commit` stores the card (`lineRecords`, `lineMoveOf`,
`person.go`): open to paused adds `turn_closed` (data `{"why": "paused", "prompt": <the message>}`), paused to open adds
`turn_reopened` (`{"prompt": …}`) and sends the message again, an owed line stopped adds `turn_closed` (`{"why": "stopped", …}`),
and a line that owes no turn any more takes its message's copy back out of the queue (`store.WithdrawPrompt`), ahead of any
drop in the same change, so the message stays read whether a stop or the finish closes the line. So a reply is never asked
for again, whichever of it and a pause or a Resume stores first; once it is in, no pause or Resume tells the device its turn
closed or opened again; and a stop names the message the line is on. Migration 74 gave the cards the build before left at work or paused their line.

Consequences: what reads a person's line reads the card, with no window over events and no rule over turn ends. The line is as
safe as any other field of the card: each move is made on the card's latest copy under its run's lock, and a session opened for
the card reads the card under the card's lock, after any stop of it.

## The protocol

How a person's device binds to a card, reads its messages and sends its acts up. The device holds a code the owner had made for the person (ADR-0060) and
connects to the `/worker` namespace with its key (`internal/workerrpc`, contract `web/src/rpc/worker/define.ts`).

- **The feed** is the person's cards' `prompt` and `system` events after a cursor, read off the events the cards already hold
  (`WorkerFeed`, `workerEventOf`, `worker_view.go`), never a second log. The device names its cursor in its handshake (`after`);
  with none it starts at the head (`me().head`). Each event is pushed in order and answered before the next; one left unanswered
  ends the connection, and the device resumes from its cursor. A change to a person's card rings that person's connections
  (`Tap`, `Hub`); a card the channel has not seen since it started rings every connection.
- **An act** is a call that acts as `ByWorker(card)` on the person's own card, once per request id namespaced by the key
  (`worker:<key>:<id>`, `workerAct`): accept with a due time, decline with a reason, answer a decision card, ask the lead, mark
  messages seen. A replay answers as the first call did; the same id asked for another act is refused.
- **What the person sees** is `personTask` (`worker_view.go`): their brief, the deliverable, the inputs handed to them, the repos,
  and where the task stands. They hear the Chair as *the lead*: nothing names a chain, an agent or the owner.
- **The report.** A decision card's `answer` and a decline are the turn's report through one path (`personReport`): the words
  end the person's turn as a reply does. An accept and a question end nothing; a question reaches the Chair as a message headed
  `[from <name> · card <id>]` and stays on the card as `asked`. A work card's acts are its session's tools (`hand_in`, `ask_lead`), as
  the hosted session decides them (ADR-0059).
- **Delivery** is on the message's own event (`MarkDelivery`): reached once a device answered its push, seen when the person opens
  it (`seen`).
- **Presence** is in memory: each app on the person's device reports itself (`presence`: a phone or a desktop, whether it is in
  view, when it was last used), and a key's apps go when its last socket closes. `Presence.Words` is the one owner of how it
  reads: *in the app now · phone*, *active 2h ago*, *not signed in yet*. What the person says of when they can work is stored on
  them (`availability`, `Employee.Availability`).

## Time and accountability

What happens when an offer is not taken up, a person passes the time they gave, the lead ends their work, or they dispute a
send-back. Builds on ADR-0032 (time waits), ADR-0035 (subscriptions), ADR-0048 and ADR-0049 (the Chair's seat and its waits).

A person's card answers to one time at a time (`timeOf`, `deadline.go`): an offer to its deadline (`hire_agent`'s `deadline`,
24 hours unless the Chair gives one, bounded to 7 days), accepted work to the ETA the person gave (bounded the same way,
`withinWindow`) until their first reply (`Offer.Replied`, so a send-back asks no time again), and cancelled work to the end of its
window to hand in. The rule reads the offer alone, never the card's status: a restart or a chain's sleep stops a card's process,
not the work. Why the work ended is a fact of its own (`Offer.Ended`: accepted, stopped, handed over, its window ran out, or its
chain finished), set by the act that ends it, and the one record of it: the person's session, the task their device shows and the
timer all read it. An ended card answers to no time, and a refusal of the person's act on it names how it ended (`endedWords`).
Every end the person did not cause (a withdrawal, the window's end, a stop, a hand-over, the chain's finish) tells their device in
the change that makes it, by one note (`work_ended` from `workEnding`, shown as `task.ended`; the line it closes adds none); the
lead's accept of their hand-in tells them by its verdict. The first end wins and is told once: `commit` reads each person's card
as stored inside its transaction (`settleEnds`), drops a write made on a copy that missed an end stored since, and the notes of
ends told already. A hand-in the window's end leaves waiting still reads handed in, and its note says it waits for the lead. The
person's device shows the time the card answers to (`timeOf`), as the owner's header does. Every act of the person on ended work
is refused the same way (`HandIn`, the session, Accept, Decline, the answer), and `accept_work` refuses work they never took on.
The lead's next message to an ended card (the Chair's, decided on the card's latest copy) takes the work up again and clears it,
and a time that passed meanwhile (an offer's deadline, an ETA) counts as told (`Offer.Told`), so taking work up again tells the
Chair of no old time. The offer is
written only through `commit`, so the timer has one path. The owner's card header shows the time (`AgentStanding.due`) and, once
the Chair was told it passed, says overdue (`Due.overdue`, from `Offer.Told`) until the person replies or the work ends.

Options for the timer:

- **The Chair subscribes to each deadline itself.** It depends on the Chair remembering, and a `subscribe` replaces its time
  wait.
- **Each act writes its own timer** (hire, accept, cancel). Every new act is one more writer, and they drift from the card.
- **One timer per card, derived in `commit` from the card as stored (chosen).** `putDeadlines` writes, moves or deletes the
  card's subscription (`sub_deadline_<card>`, an automation owned by the chain, action `deadline`) in the same transaction as the
  card, so no act can leave the two apart. The timer waits for `dueOf`: `timeOf` less a time already told. A moved timer keeps
  what it judged and fired; a new one starts at the log's newest place.

When the time passes, `fireDeadline` tells the Chair once, in one `send` that wakes it: *Offer not taken up*, *Overdue* or *The
window to hand in ended*, with the person's stated availability and who to escalate to. The same send marks the time told on
the offer (`Offer.Told`) and records the firing and a `deadline` decision, its one decision; at the window's end it also stops
the card (`Offer.Ended` window) and tells the person (`work_ended`) in that change, so a restart cannot part the end from its telling and the Chair hears of it once. A second firing is `errFiredAlready` and tells
nothing. It decides under the lock that orders the person's acts, so an accept or a hand-in lands before or after it.

- **Cancelling** a person's work is `cancel_work`, never `stop_agent`: an offer not taken up, and a question (its answer is the
  work, and a message would ask them for another reply), are withdrawn at once, in one change with one decision and the note that
  tells the person (`work_ended`, which their device shows as `task.ended`); other accepted work gets a 2-hour window to hand in
  what is done (`Offer.Cancel`), which the person's session and plugin hear as `task.cancelled`; the plugin's popup for it, once
  closed, stays closed for that cancellation alone (`at`), so the lead's next one pops up again. A hand-in or answer after the window is refused (`handInRefused`), and the window's end stops the card and
  tells the Chair. Times a person reads in the server's words are in UTC and say so (`personClock`). The person's device shows
  until when they may hand in only while the window is theirs (the server sends `graceUntil` while the task is working, sent back
  or handed in).
- **The finish waits** while a person holds accepted work (`holdsWork`: accepted and not ended, unless their report already waits
  for the lead): `finish_chain` names them, and `cancel_work` ends it.
- **A dispute** of the Chair's send-back or a reviewer's verdict on the person's own work is the person's AI's `dispute` tool,
  called on the person's word. A verdict reaches the person by id when the Chair routes the reviewer's work to them, and a verdict
  on other work is refused (`onWorkOf`: on a repository whose worktree holds the card's directory). It is an inbox item for the
  owner (`InboxDispute`) that holds what is disputed (`InboxItem.Dispute`); until he rules, `accept_work`, `route` and `cancel_work`
  on the card are refused, the check and the dispute ordered by one lock. His ruling reaches the Chair as a message and the person
  on their card.
- **Decisions a hand-in carries** (`hand_in`'s `decided`) become chain decisions of kind `outside`, and the brief of every later
  hire in the chain lists them by id (`decidedBrief`).

Consequences: a deadline outlasts a restart (the timer is a stored clock sensor, and the rule reads no card status); finishing a
chain ends the work still open on its people's cards (`Offer.Ended` finished) and deletes its timers, and a card's work comes back
with the lead's next message to it; the Subscriptions tab lists every person's deadline. Migration 81 moved the cards stored as
closed before `Offer.Ended` onto it, with the first report's time as their reply; migration 82 ended as finished the people's cards
of chains that finished before the finish set an end.

Revisit when:

- **A Chair reports to an employee**: a dispute is refused today; it would go to a chain in that employee's seat, as
  `wakeKeyResultOwners` opens one.
- **An ETA beyond 7 days** is needed.
- **A send-back asks for a new ETA**: today the first reply answers the ETA, and a send-back asks no time again.
- **Hardening**: a cap on disputes per card.
- **A stale copy of a card written over a newer one** matters beyond ends: `PutCard` is an upsert with no revision check, so a card
  with no run, read outside every lock, can be written over a change stored since. Only ends are settled inside the transaction
  today (`settleEnds`); a revision check in `PutCard` that refuses the stale write (`store.ErrConflict`, the caller reading again)
  is the store-wide seam.

Not enforced yet: the tests that will hold it are named in `docs/superpowers/plans/2026-11-03-human-employees/06-accountability.md`,
Tasks 1-12, and are written when this slice is hardened; this line then becomes their `Enforced by:`.

## Consequences

- A person's card is a card with a run: queue, turn, report, rework, `accept_work` and the tools scoped by the run's token work for
  it unchanged.
- A person's card has no account, no model and no Claude Code session; its session id is `p_<card>`.
- What the Chair reads of a person is only what the person replied, so a report the person's AI drafted reaches the Chair only when
  the person sends it.

## Revisit when

- **A person should sit in the Chair seat** (ADR-0048): the Chair's turn end settles its waits and moves the chain, which assumes the
  Chair answers every message it is sent.
- **A team memory** every employee reads (decision 13 of the design) is decided: what a person's reply records there.

Enforced by: nothing now; superseded by ADR-0062, whose tests hold a mentor's help on an AI employee's card.
