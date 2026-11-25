# 0109. A chain starts at once, and its goal judge rules on its check

Status: accepted, 2026-11-18. Supersedes the intake of ADR-0031, ADR-0035 and ADR-0108 (the intake card, its pool, its
`sub_hire_intake` subscription, `Chain.chosen`), and the check review of ADR-0031 and ADR-0068 (the check judge, its
`sub_hire_check_reviewer` subscription, `VerdictRole::CheckReview`, `Signs::Check`). Source: the owner, 2026-11-18,
"remove the intake concept" and "why does the judge run before the chain is done".

## Context

An owner's chain did not start when he made it. It was queued unchosen, and an intake card (a judge of its own, in a pool
of its own, hired by the `chain.queued` fact) read the goal, named the chain, chose its playbook and project and decided
between a direct request and a shaping one; only then did the chain take a Chair. Every chain paid a second session and a
wait before any work, and what the intake chose the Chair re-read anyway.

Once started, the Chair states its check with `state_check` before its first hire. That wrote `fact:check.stated`, whose
subscription hired a check judge that reviewed the check while the work had not begun. On the chain page this read as a
judge running before the chain was done (kiot chain `ch_3d3b8109bb4d`, "Check judge · review 1" at 21:50). Its finding
reached the Chair as a message that blocked nothing, and a second unresolved finding raised an `exception` for the owner.

## Options

1. Keep both, rename them. The owner asked for the concept to go; renaming leaves the same wait and the same early judge.
2. **The Chair takes the intake's jobs in its first turn; the goal judge rules on the check when it rules on the result.**
3. Keep the intake as a tool the Assistant calls. Nothing needs a separate classifier: the Chair already reads the goal.

## Decision

Option 2.

- An owner's chain is queued and admitted like any other, titled by the first line of his words. Its Chair titles it in its
  first turn with `set_title` (a `title` decision) before any hire, and sets its project with `set_project` when he named
  none. A shaping request is one the owner or the Assistant starts from the `objective` playbook.
- The intake card, its pool (`-max-intakes`), `Chain.chosen`, the `choose` decision and the `sub_hire_intake` subscription
  are gone.
- The Chair still states its check with `state_check`, and `fact:check.stated` is still recorded. No judge is hired for it.
  The goal judge's brief carries THE CHECK ITSELF when the chain serves a key result: a check that would pass work missing
  the key result is `not_met` with `unverifiable_key_result`, and the Chair states it again.
- Migration 0116 deletes the intake and check judges' cards with what was recorded for them, the check judge's verdicts and
  their subscriptions (firings kept, pending ones cancelled), removes `chosen` from chains and reads `choose` decisions as
  `title`.

## Consequences

- A chain's first session is its Chair's; nothing waits between the owner's words and the work.
- A weak check is found at the finish, after the work, not before it. The Chair's brief tells it to make the check one that
  fails now for the right reason, since the goal judge sends a hollow one back.
- The owner sees fewer judges and no check exception.

## Revisit when

- A finish is sent back for a weak check often enough (counted by `verdict_rates`, ADR-0103) that an early review would save
  more work than it costs: the seam is `fact:check.stated`, to which a hire subscription can be added again.
