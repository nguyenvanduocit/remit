# ADR-0132 · A chain keeps its goal judge across checks

Status: accepted. Changes the goal judge of `docs/design.md` (*How results are checked*), ADR-0035 and ADR-0039.

## Context

Each finish the Chair asked for was checked by a new goal judge: a fresh agent that read the record of the earlier
checks (their findings, the HEAD each judged) and started its investigation from nothing. After a refusal, the second
judge redid what the first had already established, could word the same items differently, and had no memory of why the
first ruled as it did. The owner asked that the judge that gave a chain's check be the one that judges it again.

## Options

- **A new judge per check (as before).** Fresh eyes on every finish; each check pays the full investigation again, and
  the list of what is missing can move between judges.
- **Keep the chain's judge, resume its session.** The judge that refused knows what it asked for and checks whether it
  was done; the records still go into its brief, so its memory is a guide and the records stay the authority.
- **Keep the judge, start a new session with its card.** Same card, no memory: the cost of a new judge with none of the
  benefit.

## Decision

The judge of a chain's last goal check checks its next finish, and the second look after a merge or a live mark, in its
own session (`judge_to_ask_again`, `crates/shipping/src/goal.rs`). It is asked again while it can resume and is still
the judge the check needs: it holds a session, did not fail, is the employee the key result names (none for the built-in
judge) and runs on the model judges run on. Otherwise a new judge is hired, as for the chain's first check.

Asked again, the card is renamed to the new check (`Goal judge · check N`) and the brief is sent to it as before; its
brief opens with `ASKED_AGAIN`: what it remembers says where to look, what it found then is a claim to check again
against the work as it is now. The check still ends on the judge's turn, its verdict claimed on the pending finish, so
a verdict given outside a check counts for nothing, as before.

A judge the owner asks about its verdict may now be the one a new check is sent to, so a turn's end says what it was on
(`JudgeEnd`): `Turn` on the check's own message, `Question` on the owner's (or the Assistant's for him), `Stopped` when
it stopped, failed or ran out of time. A `Question` turn that ended without a verdict leaves the check pending: the
judge's next turn is the check's brief, waiting in its queue.

## Consequences

- A chain has one goal judge card while it can be kept; the tree and the Verification tab show its rounds under it.
- A refusal and its re-check come from the same mind: the list of what is missing holds between checks, and the second
  check reads what changed rather than everything.
- The judge's context grows with each check; Claude Code compacts it, and a judge that failed is replaced.
- A judge that was wrong once may anchor on its own reading. The brief names its earlier findings claims to check
  again, the chain stops at `MAX_GOAL_CHECKS` refusals, and the owner can overturn as before.

## Revisit when

- Checks show a judge holding to an item it should have dropped (an item `not_met` across checks while the chain's
  work on it changed): `judge_to_ask_again` is the one place that decides, and a context-fill or age limit goes there.
- The owner wants a fresh look on demand: a field on the finish request naming a new judge is the seam.
