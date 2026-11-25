# 0003. Grow from the working core: hold new entities until the rails prove themselves

Status: accepted, 2026-10-27. Source: review Appendix 8 section 6, with the stress-test in Appendices 1, 7 and 8.

## Context

The product added entities faster than it proved the ones it had (30 migrations, 76 files in `internal/orch`). Each new entity
adds a table, a notification, a view and a brief. `AGENTS.md` says "do the larger change, not the quick one"; applied to
every new idea, that rule grows the surface faster than review capacity (ADR-0001) can absorb.

## Decision

- No new persisted entity (table, notification kind, `ChainView` field) is added until ADR-0001 and ADR-0002 have held for
  four weeks. Changes to existing entities, fixes, fitness functions and the review's work packages are not held.
- The larger-change rule of `AGENTS.md` stays for what is built: it means the design is done properly, not that more is built.
- Fitness functions make the rules executable: a commit-size budget and a migration check, each starting with an allowlist of
  today's exceptions.

## Consequences

Slower feature intake by design. The time goes to the rails, tests and fitness functions that make later changes cheap.

## Revisit when

Four weeks after ADR-0001 is live, or earlier if the owner asks for an entity this blocks (the owner's request overrides).
