# A measure is a signal and a condition

## The rule

A guardrail or a key result counts only when a signal reads it. There is no opinion, no "a reviewer says so", no number a chain reports about itself. If the owner wants something measured and no signal carries it, the first piece of work is building the signal.

## What a measure holds

| | holds |
|---|---|
| guardrail | `kpi` (its name), `signal` (the key that reads it), `condition` |
| reading key result | `signal`, `condition`, `window` (what the signal aggregates, such as `7d`), `expiry` (the day its reading stops counting), `incident` (what breaks, how it is seen, who reverses it and how: a reading reads production) |
| command key result | `repo`, `name` (the command line run with sh in that repository's worktree, read-only and offline), and when its chains land a staged change `telemetry` (the signal the result is seen through) and `incident` |

The condition is written in the language of a subscription's condition, on the reading's `value` (an event signal's body is `body`): `value.solves >= 10`, `value.errors / value.requests < 0.02`, `body.plays >= 3`. Write it on fields seen in a real reading: `read_signals` shows the signals, and the chains' agents read their readings.

## The dry run

For every guardrail and reading key result, Remit reads the signal's newest reading and runs the condition on it. It fails, with the reason in the finding, when:

- the measure names no signal, or the signal has no reading yet;
- the condition is empty, does not compile, does not give true or false, or reads nothing from the signal (a constant, or only the clock);
- the condition names a field the reading does not have (a typo is an error, never "not met");
- a guardrail is already failed by the reading, or a key result can only be met by breaking a guardrail.

The owner's Apply is refused while the dry run finds anything, and it runs again at Apply because the signal may have changed. A `submit` check (a person reads what landed) and an `unverifiable` check are no measure for an objective.

## What to tell him

- Refused Apply: read him the findings in plain words (which measure, which signal, what was missing) and offer the chain that fixes it: an `objective` chain on the `okr-review` playbook to rewrite what the signals can already read, a `work` chain on the `okr-instrument` playbook to build a missing signal.
- A number he asks for with no signal: say there is no reading, never an estimate.
- Who could inflate a measure: ask it. A count the team itself can raise (its own plays, an agent's own writes) measures the team.
