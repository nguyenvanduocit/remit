# 0093. A person teaches from a reply, the owner's overturns are counted per kind, and each employee has a face

Status: accepted, 2026-11-13. Extends ADR-0088 (who learns from what), ADR-0063 (the coach measures, never writes, a
figure) and ADR-0061 (an employee's name). Ideas 15, 20 (its first half) and 21 (its first half) of
`docs/research-agentes-chat/report.md`, read from agentes.chat for their ideas only (its code is AGPL; nothing is copied).

## Context

- **Teaching needs a message.** A person teaches an agent today only by writing to it (`teach.steer`) or by overturning
  the Assistant (`teach.overturn`). agentes.chat puts Helpful, Not helpful and Correct reply on every agent message: the
  cheapest teaching there is, and it names the exact reply.
- **The owner's overturns teach the Assistant one at a time.** Each overturn retracts the turn and teaches his word
  (`learn_overturn`), but nothing says the Assistant keeps deciding one kind of item against him: the coach counts
  repeated corrections of employees (`repeat_corrections`), and the Assistant is not an employee.
- **Every employee looks the same.** An employee is a line of text (`Mai · E-42 (engineer)`) in every chain, the tree and
  the inbox; siblings of one lineage (the template it was hired from, ADR-0069) do not read as a family.

## Options

**Teach from a reply.** (1) A reaction stored on the event: a second record of a teaching beside the facts, which the
memory backend and the coach do not read. (2) **A teaching fact, `fact:teach.reply`, recorded by a browser call
(chosen):** the same path as every other teaching, so memory learns it through a shipped subscription and the coach reads
it with the others. Marking a reply is feedback to an agent, a browser verb like a message; it edits no domain record.

**Overturns as a signal.** (1) **A figure the coach reads, `overturn_rates` (chosen):** per kind of item, the
Assistant's answers among its newest 500 and how many the owner overturned, with his newest words; computed by code from
decisions, never written by the coach. (2) A brake that narrows what the Assistant decides alone for a kind he keeps
overturning. ADR-0087 forbids routing it to him, and no route to a mentor or a second judge exists for the Assistant's
decisions: `bring_mentor` is a Chair's for a card of its team, and no judge reviews an `answer_inbox`. A brake now would
park the decision; it waits for that route (*Revisit when*).

**A face.** (1) An image per employee, made at hire and stored: a stored record for what ids already decide. (2) **A
recipe derived on read (chosen):** `wire::app::Face` (shape, hue, accessory), from SHA-256 of the lineage (shape, hue)
and of the employee id (accessory). The ids never change, so the face is the same on every read and in every channel
without a column; the browser only draws it.

## Decision

- **`turn_teach(event_id, mark, words)`** (`wire::app`, a change): the owner, or a mentor on a card one of their own helps
  is on once they read it (`Orchestrator::mentor_teach_turn` through `help_on_card` and `mentor_reads`, ADR-0089's
  deny by default), marks an agent's own reply `helpful`, `not_helpful` (words: why, may be empty) or `correct` (words:
  the correct reply, required). An agent's own reply is a `message` event its session wrote, which names no writer: a
  message Remit posted on the card (the digest on the Assistant's) is refused, so nothing corrects an employee for
  words it did not write. A reply its agent is still writing is refused too: the stream writes over the event.
- **The mark lives on the reply.** One person's mark on one reply is one record (`ReplyMark`: the person as their mentor
  id, `m_owner` for the owner, who acted, the mark, the words, when), kept in the reply's event and carried to every
  channel as its `marked` activity, so a reload or a second tab shows it; the person's new mark replaces it, and the
  same mark with the same words again changes nothing (`remarked`, `teach.rs`, the pattern of a block's pick, ADR-0091).
- **Each change is a teaching.** The change that rewrites the reply's marks stores, in the same transaction,
  `fact:teach.reply` with the chain, card, employee, event, mentor, the mark (`mark`, a `TurnMark`) and the words in
  `why`, once per revision of the reply. The facts are the history the learners read; the record of what a person
  thinks of the reply now is the mark on it.
- **Memory learns it.** The shipped subscription `sub_teach_reply` runs `teach`; `learn_reply` lets the backend `learn`,
  for the card's project, the reply quoted (8 KB at most) with the mark and the words, taught by the person; a mentor's
  own memory learns their words (ADR-0088). A lesson a newer mark contradicts is the coach's to forget (`draft_forget`).
- **The coach counts it.** A reply whose person's newest mark is `not_helpful` or `correct` is a correction in
  `repeat_corrections`; a helpful one is not, and a mark the person replaced counts for nothing. The coach's brief reads
  `fact:teach.reply` with the other teachings.
- **`overturn_rates`** (a coach read tool, `coach.rs`): per kind of item (`question`, `permission`, a change item as
  `change <record>`), the Assistant's answers, the owner's overturns, the rate and his newest three words, the most
  overturned first. `teach.overturn` now names the item. The brief notes each kind's figures and tells the owner in one
  line when a kind's overturns rise. It narrows nothing the Assistant decides.
- **`Employee.face`** (`wire::app::Face { shape: FaceShape, hue, accessory: FaceAccessory }`), derived by
  `employee_face::face_of` for every employee the server sends (`employee_read_of`): six shapes, a hue 0 to 359, six
  accessories. The `employee_updated` push now sends the employee as a channel reads it (`employee_read_of`), shown, tag
  and face included, as the snapshot does.

## Consequences

- The web app can put Helpful, Not helpful and Correct reply on any agent message and draw a face wherever an employee
  shows (the Assistant is no employee and has none). Both are the next web wave.
- Every mark spends some of the memory backend's Claude quota, as a steer does.
- The coach sees which kinds of decision the Assistant gets wrong and says so; the Assistant keeps deciding them.

## Revisit when

- A route exists to put an Assistant decision to a mentor or a second judge before it stands: then a kind the owner
  keeps overturning goes there first (the brake of option 2), never to him.
- The owner wants a face he chose: a stored face overrides the derived one.
