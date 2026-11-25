# Seeing where an objective stands

## The chains

Start each with `create_chain`, of the kind the table names, its goal naming the reference playbook its Chair reads.

| He wants | Start | What it does |
|---|---|---|
| to know where his objectives stand and what moved | kind `objective`, playbook `okr-checkin` | A Chair has the OKR Analyst read every measure on its signal and reports standing, the newest value with its time, the move, what is stale or unreadable, and which guardrail is close to its bound. It amends nothing. A routine can start it on a rhythm. |
| his objectives checked against the measure rule | kind `objective`, playbook `okr-review` | A Chair has the OKR Auditor run each condition on its real reading, then amends what the existing signals can settle; the amendment waits in his inbox for the Assistant to decide. |
| a measure that has no signal made readable | kind `work`, playbook `okr-instrument` | A Chair builds the instrumentation (an event signal it opens with a standing sensor that waits for an Apply, a plugin or script) with a test, and reports the fields of the first real reading so a condition can be written on them. It drafts no objective. |
| a new objective | kind `objective`, playbook `objective` | See `references/shaping-and-amending.md`. |

## Reading standing yourself

The Objectives tab (under Work) shows each key result as met, not met, unverified or unserved. A reading key result is met or not met by its signal; `unverified` means the signal has no reading or the condition cannot be read on it. `read_objective` (its id) gives one objective as stored, with its key results. Use `read_signals` for the signal behind a measure. Say the value and when it was read.

## The OKR agents

- **OKR Analyst**: finds the signals, reads their values, writes each condition and baseline, names what has no signal. Reads, drafts nothing.
- **OKR Auditor**: audits an objective or draft against the rule on the real readings; reads, edits nothing.

A Chair of an OKR chain hires them; the owner does not talk to them directly. The Chair is the one that drafts, and the Assistant decides every draft for him in the inbox.
