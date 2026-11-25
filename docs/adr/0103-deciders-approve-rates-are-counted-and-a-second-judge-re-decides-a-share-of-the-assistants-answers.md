# 0103. Each decider's approve and decline rates are counted, and a second judge re-decides a share of the Assistant's answers

Status: accepted, 2026-11-13. Extends ADR-0093 (the coach reads `overturn_rates`), ADR-0087 (no agent waits on the owner)
and ADR-0063 (the coach measures, never writes, a figure). Row R2 of `docs/reports/Long running agent systems beliefs.md`.

## Context

- **A lenient decider looks fine from the overturns.** Anthropic's Project Vend found its CEO agent approved about eight
  requests for every one it turned down. Remit's only figure on the Assistant is how often the owner overturns it
  (`overturn_rates`, ADR-0093), and the Assistant decides for him so that he does not have to read: an answer he never
  reads is never overturned, so the overturns undercount exactly when the Assistant is relied on most.
- **Nothing says how often a Chair or a judge says yes.** Chairs accept or send back their workers' reports
  (`accept`, `rework` decisions); reviewers, QC, the goal judge and the check judge approve or ask for changes
  (approvals). A gate that approves everything catches nothing, and nobody counts it.
- ADR-0093 left a brake on the Assistant waiting for "a route to put an Assistant decision to a second judge". This is
  that route as a measure; the brake still waits.

## Options

**Where the approve and decline rates live.** (1) A new report or screen of its own: a second measure beside the
overturns, read in another place. (2) **The coach's figures (chosen):** `overturn_rates` gains, per kind of item, the
Assistant's approvals, declines and second-judge dissents; a sibling read tool, `verdict_rates`, gives the Chairs (per
playbook) and each kind of judge. The owner already hears these figures from the coach in one line when they move.

**What an answer approves.** (1) Read from the item's answer text: each kind words it differently (`Allow`, `not now`,
`retried`, `Sent back …`), and a held item's answer is overwritten when its hold lifts. (2) **A `stance` on the assist
decision (chosen):** `approve`, `decline` or neither, computed when the Assistant answers from the option it chose
(`assist::stance_of`), stored with the decision, read by the coach. One rule, in one place.

**How the second opinion is judged.** (1) Remit compares option ids: a question answered in words has none, and two
permission options can decide the same thing. (2) **The judge decides first, blind, then compares (chosen):** it
records its own answer with `second_opinion` before it can see the Assistant's, and only then is shown the Assistant's
answer and reason and says with `compare_answers` whether the two decide the same in substance. Its own answer is fixed
before it reads the other, so the comparison cannot bend the decision.

**Where its record lives.** (1) A decision only: a decision has no field for "agrees", and a second opinion still waiting
has no decision yet. (2) **An entity of its own, `SecondOpinion` (chosen):** one row per re-decided answer in table
`second_opinions` (migration 0107), the pattern of a duel (ADR-0063): the judge's card, its answer, whether the two agree
and why it ended. A difference is also a `dissent` decision, so it reaches the owner as decisions do.

**Which answers.** A fixed function of the answer's decision id (`second_opinion::sampled`, SHA-256 of the id modulo
100 below 8): the id is random, so the share is a random 8% sample, inside the owner's 5 to 10%, and asking twice gives
the same answer (idempotent).

## Decision

- **`Decision.stance`** (`model::Stance`: `approve`, `decline`, empty for neither) is set on an assist that answered:
  a permission's allow option, a change applied, an exception accepted, a failure or a held firing retried approve; the
  other answer of each declines; a question's answer is neither. `record_assist` takes the chosen option.
- **`overturn_rates`** adds per kind: `approved`, `declined`, `approve_share` (approved per answer that approved or
  declined), `rechecked` (answers a second judge compared), `dissented` and `dissent_rate`.
- **`verdict_rates`** (coaching, every agent of the chain): the newest 500 verdicts of the Chairs (an agent's `accept`
  or `rework` decision, per its chain's playbook) and the newest 500 approvals (per role: `judge`, `reviewer`, `qc`,
  `check_review`), each with verdicts, approved, declined and `approve_share`, the most approving first. Computed by
  Remit (`coach::verdict_rates`, pure); the coach writes none of it.
- **The second judge** (`second_opinion.rs`, role `second_opinion`, origin judge, `cfg.judge_model`, as the duel
  judge is started): when the Assistant answers an item of a chain that has not finished and the answer is sampled, Remit
  creates a card in that chain with the item, its options, the asker and the chain's goal (never the answer) and stores
  the `SecondOpinion` with it in one change. Its tools are `second_opinion` (once: an option id, or words, and why) and
  `compare_answers` (once, after: `same` and why). A turn or session that ends before the comparison ends the second
  opinion with why; it counts for nothing. An item of no chain (Remit's own) or a credential or sign-in is never
  re-decided.
- **A dissent** (`DecisionKind::Dissent`, by the judge, `item` the item) is stored with the comparison. The Assistant's
  feed (`FeedLine.dissent`, matched by item) shows it on the answer, above Overturn, and in the inbox's decided line;
  a dissent he has not seen counts in the feed's unseen. He overturns the answer as before; the dissent itself decides
  nothing.
- The coach's brief: step (6) reads the new figures and tells the owner in one line when a decider approves nearly all it
  judges while dissents, refusals or overturns show it misses.

## Consequences

- About one answer in twelve costs one more judge session, at the judge's model.
- The coach can tell "the owner overturns little" from "the Assistant is right": a high approve share with dissents
  rising is the rubber stamp Vend's CEO was.
- A judge's verdict figures read the approvals table whole; the Chairs' are read from decisions of two kinds, the newest
  500 of their union.
- Migration 0107's place in the list is positional (`store::migrations::MIGRATIONS`): file n is entry n, which a test holds.

## Revisit when

- Dissents of a kind of item stay high: that kind goes to a second judge before the answer stands (ADR-0093's brake),
  never to the owner.
- The owner wants another share: `SAMPLED_PERCENT` becomes a setting.
- Judges of the same model family agree with each other for the wrong reasons: the second judge runs on another model.
