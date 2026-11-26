# ADR-0133 · The owner caps the model, and agents think at medium by default

Status: accepted, 2026-11-26. Amends ADR-0070 (the Chair's and the Assistant's defaults), ADR-0102 decision 1 (the
judges' model and effort) and ADR-0118 point 4 (the review depths).

## Context

The owner asked that the flow run on a smaller model and think less. Every chain ran its Chair and the Assistant on
`fable` at `high`; a hire defaulted to the Chair's model unless a decider was sure of a cheaper tier; every judge (goal,
round, second opinion, duel, and a card that signs a gate) ran on `fable` at an empty effort, which Claude Code runs at
`xhigh`; and a change a decider found risky raised its reviewers to `xhigh` or `max`.

## Options

- **Lower the Chair's and the Assistant's defaults only.** The judges stay on `fable` at `xhigh`, and every change is
  reviewed by them: most of the cost stays.
- **Point the judges at a model of their own (a `-judge-model` flag).** A Chair could still name `fable` for a hire, and a
  judge would then grade an author stronger than itself, which ADR-0102 rules out.
- **Cap the catalog at a model the owner names (`-top-model`, chosen).** The catalog Remit offers stops at that model's
  tier (`provider::Catalog::up_to`), so no card runs above it, and the judges, which run on the catalog's strongest, move
  with it: ADR-0102's rule holds with no second setting.

## Decision

1. `-top-model` (default `opus`) is the most capable model Remit runs. The server builds its catalog with
   `Catalog::up_to` (`crates/remit/src/config.rs`); a Chair, Assistant or hire naming a model above it is refused, and
   `judge_model` is the capped catalog's strongest.
2. The Chair and the Assistant default to `opus` at `medium` (`-chair-model`, `-chair-effort`, `-assistant-model`,
   `-assistant-effort`). A hire's effort is still its agent's, else the Chair's.
3. A judge Remit starts (goal, round, second opinion, duel) runs at the Chair's effort (`-chair-effort`) instead of
   Claude Code's default; a card that signs a gate already took it.
4. The depths `gate.risk` chooses between are `medium`, `high` and `xhigh` (`crates/chains/src/review_depth.rs`); they
   still only raise the judges above their usual effort.
5. The judge template names no model: every path that hires a judge sets `judge_model`.

## Consequences

- Every session runs on `opus` or below, and thinks at `medium` unless an agent definition, the Chair or a risky change
  asks for more.
- The owner gets `fable` back for everything with `-top-model=fable -chair-model=fable -assistant-model=fable`.
- A risk score already stored on a landing's version reads as the new depth of its level.

## Revisit when

- Judges' recall on seeded faults (ADR-0102) or the overturn rate falls after the change: raise `-top-model` or the
  depths, the two seams this ADR set.
