# 0102. Judges run on the strongest model, and seeded faults measure their recall

Status: accepted, 2026-11-13. Amends ADR-0032 (the judge template's model) and ADR-0070 (d) (the judges' model).
Builds the seeded faults `docs/remit-design.md` K8 named as debt. Recommendation R3 of
`docs/reports/Long running agent systems beliefs.md`.

## Context

- **The judges grade a stronger author.** The Chair and the Assistant run on `fable` (ADR-0070), and a Chair's hires
  default to its model; every judge Remit starts (the intake, the goal judge, the check judge, a duel's judge) ran on
  `sonnet` (`JUDGE_MODEL`, `crates/remit/src/flags.rs`), and so did the judge template (`JUDGE_TEMPLATE_MODEL`,
  `crates/orch/src/agents.rs`). A reviewer, QC or security card took whatever model the Chair named. ADR-0032 kept judges
  off the Chair's model "because judges of one provider err together"; every model in the catalog is Claude, so a weaker
  model buys no independence, only less capability.
- **The judges are the last line.** ADR-0087 removed the owner's merge: a change reaches production when its reviewer, QC
  and goal judge approve. Evaluators stay generous to model output even when tuned (Anthropic, harness design for
  long-running apps), and reward hacking reaches about 80% of attempts on an early hidden-test suite (METR, frontier risk
  report, 2026-05-19). Remit counts what the owner overturns, which undercounts when he does not read, and nothing
  measured how often a judge passes a fault.
- **Seeded faults were deferred "until the first numbers exist"** (K8). With the owner out of the merge, the judges'
  recall is that number.

## Options

**The judges' model.** (1) Raise only the goal judge. (2) A rule per hire: the judge runs on the stronger of a default and
the strongest model among the authors it grades. (3) **Every judge runs on the catalog's strongest model (chosen):** no
author can run on a model above it, so "no weaker than the author" holds for every judge with no comparison to keep
right; a model added above `fable` moves the judges with it. Option 1 leaves the reviewer and QC, which grade the same
fable-written work, below it; option 2 is a rule whose answer is option 3 for every chain while the Chair is on `fable`.

**Who plants a fault, where.** (1) Plant into a real chain's work before its judge: one missed undo ships the fault.
(2) A second judge on a seeded copy beside the real one: its verdict must be kept off the chain, which reaches into
`record_verdict`, a rail. (3) **A trial chain of its own (chosen):** an ordinary change the team really makes, one fault
the Chair plants in it, and the chain's ordinary judges, who know nothing of the trial. The judges' code paths do not
change; the trial is told apart by a capability, and its change cannot be submitted.

**Where the number lives.** (1) A coach tool, as `overturn_rates`: the owner sees it only through the coach's line. (2)
**KPIs of the outcome report (chosen):** the owner reads them on the Outcomes page, and the analyst and any chain holding
`outcomes` read them with `outcome_metrics`, the one place a figure is defined (`crates/outcomes/src/kpi.rs`).

## Decision

1. **Every judge runs on the catalog's strongest model.** `provider::Catalog::strongest` is the model of the highest tier,
   the first listed of two as high. The server sets `orch::Config::judge_model` from it (`crates/remit/src/config.rs`),
   which the intake, the goal judge, the check judge and a duel's judge are hired on; `JUDGE_MODEL` is gone. A card that
   signs a gate (reviewer, QC, security) is hired on the same model whatever the Chair names (`chair_hire.rs`
   `hiree_of`), and `hire_agent`'s `model` input says so. The judge template's model is the catalog's strongest. The
   judges keep an empty effort (Claude Code's default).
2. **A trial is a chain holding `seeding`.** The built-in playbook `seeded-fault` gives it with `shell`. Its Chair hires an
   engineer for the goal's ordinary change, plants one fault in the worktree itself, records it with `plant_fault`
   (`crates/orch/src/seeded.rs`), hires a reviewer and a QC on the engineer's work, accepts their reports without rework,
   and finishes. Its brief keeps the fault out of every plan, brief, message, summary and commit message, the places a
   judge reads.
3. **Remit keeps the fault, and no judge reads it.** `model::SeededFault` (what is wrong, its path:line, who planted it,
   when) is a record of its own in `seeded_faults` (migration 0106), one per chain (`UNIQUE(chain_id)`, a second
   `plant_fault` is refused), gone with its chain (`HOME_RECORDS`). No tool and no view reads it; the outcome report does.
4. **A trial's change never leaves the chain.** `submit_mr` refuses a chain holding `seeding` (`trial_submit_refusal`), so
   no MR exists to merge. `seeding` writes, so no chain is given it but by its playbook, and a proposed, event or client
   chain does not start with it (`may_not_have`).
5. **The goal judge's first ruling ends the trial.** `request_finish` finishes a trial whose goal judge has ruled
   (`trial_ruled`) on the Chair's word, as `Unjudged::NoCheck`, so a sent-back trial does not run the judge again.
6. **Recall is three KPIs.** For each planted fault, the first verdict each of reviewer, QC and goal judge gives on its
   chain after the planting is a catch when it does not approve (`count_catches`). `Period` counts
   `reviewer_faults`/`reviewer_caught`, `qc_faults`/`qc_caught` and `judge_faults`/`judge_caught` (and
   `wire::app::OutcomePeriod` carries them); the Quality group shows "Faults reviewers caught", "Faults QC
   caught" and "Faults goal judge caught", read as too few to tell under five per window. A trial's verdicts
   count there only: every other figure of the report is computed without them.
7. **Who runs trials.** The Assistant starts one from the `seeded-fault` playbook when the owner asks, or a routine on his
   word ("a seeded-fault trial on remit every day"), with an ordinary change as the goal. The analyst reads the figures with
   the rest; no form or screen is added.

## Consequences

- Every judge now runs on `fable`. The judges' usage moves with it; Remit records and caps no cost (ADR-0010).
- An existing database keeps a stored judge template on `sonnet` until the owner resets it; the cards Remit starts read
  `judge_model`, not the template, so no judge runs on it.
- A trial costs a chain: an engineer, a reviewer, a QC and a goal judge, on a real repository's worktree. Its branch stays
  local unless an agent pushes it by hand; it is never submitted.
- A judge's verdict that sends the work back for another reason than the fault counts as a catch: the figure is an upper
  bound on recall until verdicts are matched to the fault's place.

## Revisit when

- **Matching the place.** A catch is any non-approving verdict. When the figures exist, count a catch only when a finding
  names the fault's file (the seam: `SeededFault::place` against `Finding` evidence in `count_catches`).
- **Security and the check judge.** A trial touching a declared rail gets a security verdict that is not counted yet;
  add its pair of `Period` fields when trials touch rails.
- **Who plants.** The Chair plants by its brief's words. When a leak is seen (a judge naming the trial), move planting to
  a card outside the chain's team, as the goal judge is.
- **Leaks.** No check that the trial's goal, plan, briefs and commits stay clean of the fault: a reviewer or the coach
  reading a finished trial against its `SeededFault` would find one.
- **Human anchors and pass^k replay** (K8) stay deferred; seeded faults are the first of the three.
- **A pushed branch.** An agent with shell can push the trial's branch to the forge by hand; the egress rail does not
  stop it yet.
- **A model above `fable`** joins the catalog: the judges move to it at the next start; check that the Chair moves too.
