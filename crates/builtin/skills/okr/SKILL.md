---
name: okr
description: "The craft of objectives and key results in Remit. A measure is a signal's reading and a condition, proven by a dry run; this skill says how to choose the customer outcome, find or build the signal, write the condition on a real reading, set the threshold from what was measured, and keep the team from measuring itself. For the Assistant: how to talk to the owner about objectives, amending, closing and where they stand. For a Chair or worker of an OKR chain: how to draft, audit, check in and instrument. Load it before you draft, audit or advise on an objective, a key result, a guardrail or a metric."
---

# Objectives and key results

An **objective** is what the owner wants reached, as key results with owners and judges. The rule behind all of it: **a measure counts only when a signal reads it, and a dry run proves it** (ADR-0045). Opinions, chains' summaries and a reviewer's word are not measures.

## What is what

- A **key result** is one thing that must be true. Its check is a `reading` (signal, condition on its value, window, expiry, and the `incident` plan: what breaks, how it is seen, who reverses it and how) or a `command` (a command line run in a repository's worktree passes). A reading key result is met or not met by its signal alone. A check is what a chain delivers first (ADR-0068): its chains do not finish without it.
- A **guardrail** is a measure the work must not push past: a name, the signal that reads it, and a condition that holds while the work stays inside the bound.
- A **dry run** reads the newest reading of each measure's signal and runs the condition on it. The owner's Apply is refused while any measure fails it.
- A rule no signal reads (a working rule such as "say an AI team made it") belongs in the brief of the chains, not in the objective.

## Which reference to read

| You are | Read |
|---|---|
| anyone: what a measure is, what the dry run refuses, why Apply was refused | `references/measures.md` |
| a Chair or worker drafting or auditing: who the customer is, the outcome, the numbers, thresholds, guardrails, what to do when no signal exists | `references/drafting.md` |
| a Chair or worker writing a condition: patterns on a signal's value or an event's body, and the mistakes that fail the dry run | `references/conditions.md` |
| the Assistant: a new objective, changing, refining or closing one, a stalled key result, who answers for a key result | `references/shaping-and-amending.md` |
| the Assistant: where objectives stand, what moved, auditing, building a missing signal | `references/review-and-checkin.md` |

## Always

- Say what a signal reads, with the time of the reading, or say it has no reading. Never call a key result met, or say nobody did something, from a summary.
- The Assistant never drafts an objective: it starts the chain that does (`create_chain` with kind `objective` to shape, review or check in; kind `work` to build a missing signal), names the reference playbook that fits in its goal (`objective`, `okr-review`, `okr-checkin`, `okr-instrument`) and tells the owner which chain runs.
- Chairs draft and workers read; the Assistant decides every draft in the inbox for the owner (Apply or Not now), and he may overturn it.
