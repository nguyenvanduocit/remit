# 0045. A measure counts only when a signal reads it, and a dry run proves it

Status: accepted, 2026-11-03; its waits on the owner, but a secret act, are an agent's (ADR-0087). Builds on ADR-0014 (signal is the backbone), ADR-0031 (an objective is a set of key results the owner
approves) and ADR-0044 (a plugin script reads what no source gives). Source: the owner's words of 2026-11-03: every metric is a number
measured in a signal, counts as true only when the signal carries it, and is accepted only when a dry run of it runs.

## Context

The first dogfood objective, "one real person outside our team plays Lights Daily and tells us" (ob_b6745bdae1ed), had four key
results whose check was `submit`. Its guardrail on the build objective forbade any runtime network request, so the game could not
report a play. Nobody outside the team could be seen playing it; a friend of the owner did, and the system never knew. What the system
believed ("nobody has played it") was a claim about its own blindness.

The causes, each in the code:

- `Guardrail` was `{kpi, bound}`, both words. Nothing tied it to a signal (`model/objective.go`).
- A key result could be `submit` (a person reads a merge request) or `unverifiable`; the checker found a fault and blocked nothing
  (`CheckObjective`, "Neither blocks anything", ADR-0031).
- A `reading` check named a signal and a condition, and the checker asked only whether the signal had a reading. The condition was
  never run. A condition that fails to run is false in `Evaluate` (`signal/condition.go`), so a mistyped field read as "not met".
- A reading key result's standing came from a judge's words, not from the reading.

The patterns that large systems use for the same need:

| System | Pattern | Taken here |
|---|---|---|
| Google SRE (SLI, SLO, error budget) | A measure is a ratio of events over a window, binary per event, directly measurable; the target is set from what was measured; an objective with no consequence is decorative | A measure is a signal, a condition and a window; the threshold is written after reading the signal |
| Fitness functions (Ford, Parsons, Kua) | An executable, objective pass or fail that runs on every change, runs locally, and fails with a message that says what to fix | The dry run is one |
| Amazon canary comparison | Judge the new thing on the live metric, never on the team's word | A reading key result is met by its signal, not a judge |
| Azure Policy | The same rule in audit mode while drafting, then deny mode at the gate | The checker answers the Chair as it drafts; Apply refuses |
| Prometheus `promtool check rules` and `test rules` | A rule must compile and is run on a series | The condition must run on a real reading (the synthetic series is under *Revisit when*) |
| Experimentation platforms | A goal or guardrail metric needs an event already logged and carrying data before launch | A measure needs a signal with a reading before it is drafted |

## Options

1. Prompt the Chair harder to write measurable key results. Cheap, and the failure above happened with a careful Chair: no brief
   makes a signal exist.
2. Keep the checker advisory and add a measure type. Two owners of "what a measure is", and the owner can still apply the unmeasured one.
3. A measure is a signal and a condition, the checker dry-runs each on a real reading, and Apply refuses what it still faults.
   **Chosen.**

## Decision

- **A measure is `{signal, condition}`.** A `Guardrail` is `{kpi, signal, condition}`: the condition, in the language of a
  subscription's condition, holds while the work stays inside the bound. A `reading` check is the measure of a key result, with its
  window and expiry. The window says what the signal aggregates; the condition reads the signal's newest reading.
- **Only machine checks are key-result checks of an objective.** `reading` and `command` are accepted. `submit` and `unverifiable`
  remain for a task objective (a direct request, ADR-0031) and are a finding on an objective the owner approves.
- **The dry run is `signal.Probe`.** It runs a condition on a real reading and returns why it cannot: the condition is present, it
  compiles, it gives true or false, it reads `value` or `body` (a constant or a clock-only condition holds whatever the signal says),
  and every field it names is in the reading. A missing field is an error, never false.
- **`CheckObjective` is the one checker.** It dry-runs every guardrail and every live reading key result on the newest reading of its
  signal. Findings: a signal with no reading or no signal named (`unknown_metric`), a condition that cannot be read, a `submit` or
  `unverifiable` check (`unverifiable_key_result`), a guardrail the reading already fails or a key result that cannot be met inside a
  guardrail (`guardrail_breach`). It stays pure: the shell hands it the newest reading of each signal the draft names.
- **Apply is the deny gate.** `applyObjective` runs the checker again on the objective it is about to store, reading the signals now,
  and refuses while it finds anything. A close is not gated. The Chair is told the findings as it drafts, as before, and no override
  exists: the owner who wants a measure to count builds the signal.
- **A signal comes before the measure.** A measure whose signal does not exist is not drafted. The objective's first key result is the
  work of building the signal (a `command` check: the instrumentation's own tests), through an event signal or a plugin, and the
  measure is drafted when the signal reads.
- **A reading key result's standing is `ReadingStatus`.** `met` when the newest reading meets the condition, `not_met` when it does not,
  `unverified` while the signal has no reading or the condition cannot be read on it. `objectiveViews` derives it on every read. No
  chain's verdict decides it.
- **Stored guardrails** keep their words as the `condition` and have no signal (migration 65). They run as before and fail the
  checker on the owner's next amendment.

## Consequences

- An objective's numbers are the ones its signals carry, and its standing is computed. The Chair's brief, the draft tool's description
  and its schema say so.
- Drafting an outcome measure takes a prior step when the signal does not exist yet. That is the point: the system cannot report what
  it cannot read.
- The checker is a fitness function with a reason in every finding, so the Chair can fix a draft from what it says.
- The craft of a good measure (outcome over activity, a threshold from the baseline, who could inflate it, a condition on a real reading) is the
  built-in skill `okr`. The Assistant holds it with its manual; a chain's agents hold it when the chain lists the capability `okr`, which
  the objective and okr-* playbooks do. The capability gives no tool, so it never stops a proposed chain from starting.

## Revisit when

- A condition that can never fail (`value.x >= 0`) passes the dry run. The seam is a synthetic-series test as `promtool test rules`
  has: the condition must be shown true on one series and false on another.
- The north star is still words. It becomes a measure (`{signal, value}`), so the number the objective exists for is read like the rest.
- A threshold should follow a baseline (canary comparison: not worse than the value before the change) instead of a constant.
- A reading's age matters: the dry run reports when the reading was made and accepts any; a staleness bound per signal is the seam.
- A `command` check is verified to exist (the repository and its named command) and not run; running it in the dry run is the seam.
- Window semantics: today the signal carries the aggregate; a window the condition reads over several readings is the seam.
