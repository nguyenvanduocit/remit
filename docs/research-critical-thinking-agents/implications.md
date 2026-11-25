# What the research means for the owner's workflow and for Remit

Written 2026-11-04, after `report.md`. Evidence is cited as `file:line` as it stood that day; re-check before acting on it.

The lesson that carries over: **a check is worth only as much as the evidence it can actually see.**

## 1. The owner's Claude Code setup

### The Stop hook judges text, not evidence (HIGH)
- `~/.claude/settings.json` Stop hook: a command `bun ~/.claude/hooks/open-items-gate/cap.ts` (lets the stop through after
  3 blocks in a row, `OPEN_ITEMS_GATE_CAP`) and a prompt hook whose judge is told *"Judge only its last message"*.
- Observed 2026-11-04: it blocked three times in a row and each time asked for `wc -w` and `rg` to be re-run, although
  they had been run and quoted (5969 words, 33 label lines). The cap ended the loop.
- Consequences: it rewards a message that *sounds* verified (Goodhart); a fabricated "I ran X, output Y" passes; real work
  with evidence in tool calls gets blocked; each block makes the final message longer (orchestration tax on the owner).
- Proposed fix (MEDIUM, untested): the hook input carries `transcript_path`. Have a deterministic script read this turn's
  tool calls and their results and accept a "done/verified" claim only when a matching tool result exists. A script fits
  better than an LLM judge, because "did this command run this turn" is a fact and LLM judges share the generator's errors
  (~60%, `notes/verification_green_checks.md`).

### Summary of a summary
The first summary of this research rounded a figure to "about 10–30%"; the notes give 9 of 33 and 4 of 14 commits. Numbers
that matter in a final message are quoted from the notes, not restated from memory.

### What already matches in CLAUDE.md
Confidence labels tied to evidence, verify on the surface the user looks at, test behaviour not implementation, pre-mortem.
Risk: labels and explanations raise acceptance whether or not they are right (Bansal, `notes/counterarguments_interventions.md`).
A HIGH label is only worth something beside a command and its output.

### The owner's own calibration (MEDIUM)
METR: developers believed AI made them faster while it made them slower. Judge the setup by numbers (the KPI rows in
`internal/outcomes/kpi.go`, e.g. reopened after merge), not by feel.

## 2. Remit: where it already follows the research

| Research | Remit | Evidence |
|---|---|---|
| Orchestration tax | At 5 open MRs waiting for the owner, autonomous starts wait; no agent raises it | `docs/adr/0001-admission-by-review-capacity.md:20-28` |
| Define "verified" upfront, by someone other than the author | "The check is judged before the work starts, by someone other than its author" | `docs/adr/0013-…`, Decision |
| A guardrail that cannot fail (Goodhart) | Condition run on a real reading before merge; no reading → refused | `docs/adr/0050-…`, Threats |
| Evidence before summary | Findings carry evidence; `evidence_contradicts` needs platform refs (source_call, reading, event) | `internal/orch/gate.go:115-138` |
| Bound risk by exposure, not by reading every line | Canary stages, dark first, rollback written before merge | ADR-0050 |
| Crumple zone | "The owner is not their net": the owner owns the rules, not each diff | `AGENTS.md`, Agent-centric |

## 3. Remit: gaps the research ranks highest
1. **Judge independence (MEDIUM-HIGH).** Multi-provider judges are a Proposal (`docs/remit-design.md:233`). Reviewer, QC and
   goal judge run in the same harness and, probably, the same model family as the Chair (LOW: model selection not traced).
   The design already says "a panel not measured as independent counts as one vote"; it is not built.
2. **Seeded faults are deferred "until the first numbers exist"** (`docs/remit-design.md:247`, K8). Seeded faults are how the
   first number on judge vigilance is obtained (Parasuraman: failures are missed more when automation is usually right).
3. **Who writes the tests QC runs (LOW, unchecked).** If QC runs the implementer's tests, the check shares the code's
   assumptions. Stronger: tests from a separate context written from the spec, property tests, implementer unable to edit the
   checker. Not yet checked in `internal/orch/verification.go` and neighbours.

What stays the owner's (MEDIUM-HIGH): what to build and for whom, where risk tiers and canary thresholds sit, and whether the
oversight itself works (gaps 1 and 2 measure it).

## 4. Next steps offered, not started
- Rewrite the Stop hook to read tool results from `transcript_path` (owner's global config; owner to approve).
- An ADR (architect agent) moving the independent judge panel and seeded faults to next, against the "hardening when asked"
  stance: the owner's decision.
