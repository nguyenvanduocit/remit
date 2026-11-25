# 0027. An effect is ranked by how far it can be undone, and a refusal names its kind

Status: accepted, 2026-11-02. Refines ADR-0018: its two effects, read and write, become four. Source: `docs/remit-design.md` (K4), the
blueprint's rule that the cost of being wrong sets the gate, ADR-0018.

## Context

The gate knew two effects, read and write, and two types for them: `grant.Effect` (a number, ranked, what `Decide` reads) and
`source.Effect` (a string, what a tool declared). One concept had two owners, converted at the border by `ParseEffect(string(...))`.
Write was one step: a draft saved and a message sent to a customer asked the owner the same question and were granted the same way.
What an agent may do on its own depends on how far its act can be put back, and two steps could not say that.

## Options

1. Keep read and write; add a flag on a tool for "can be undone". A second field that must agree with the first.
2. Rank the effect itself: read, reversible, compensable, irreversible, one type in `internal/grant`. **Chosen.**
3. Let each tool declare its own risk score. Tool authors would rank their own work.

## Decision

- `grant.Effect` is ordered `Read < Reversible < Compensable < Irreversible`. A grant of one covers every lower one. `Decide` is unchanged.
  - Reversible: can be put back exactly (a draft saved). Compensable: another act answers for it (a charge and its refund).
    Irreversible: nothing puts it back (a message sent, a record destroyed).
- `source.Effect` is gone: `source.Action`, `source.Call` and `source.Tool` carry `grant.Effect`. `internal/source` imports `internal/grant`
  for it and for nothing else; `grant` imports nothing.
- The platform ranks a tool, in its declaration. A tool that does not say reads. The Chair does not rank.
- Text becomes an effect once, at the boundary (`ParseEffect`): `read`, `reversible`, `compensable`, `irreversible`. `write` names none.
- Migration 54 rebuilds `chain_grants` for the new names. A stored `write` becomes `irreversible`, which covers every lower effect, so no
  chain loses a right it held. A grant ask the owner has yet to answer is rewritten the same way.
- A refusal names its kind: `grant.Refusal` is `no_grant` (the chain holds no grant that covers the action) or `rail` (a rule of the
  platform that no grant moves: a program on a protected connection, a form the server does not offer, a call that names no chain, a
  policy that failed or gave no answer). `Decision` and the gate's `Verdict` carry it, the gate keeps it on the `Call`, and the
  account of calls (`source_calls`) stores it. A refusal nobody named is a rail. Kinds join when the rules that make them do: a stop
  and a law, with their units.
- Any effect above read still needs the owner to grant it (`NeedsOwner`); a read of a protected connection too.

## Consequences

- No tool writes today (`NewTool` makes readers), so no behaviour of a running chain changes. The scale is there for the tools that will.
- An owner who read "write" in an inbox item sees `irreversible` for an old ask: the strongest word, chosen for safety.
- Threat model (a rail, the gate to the outside world):

| Threat | Control |
|---|---|
| A tool ranks its own write as a read (tampering) | The rank is in the tool's declaration in code the platform ships; an agent cannot declare one. A tool that declares nothing reads, and a read tool that writes is a defect found in review of the tool |
| A migration turns a write grant into a lesser one (elevation) | `write` becomes the highest rank, never lower; `TestMigration54RanksAWriteGrantAsIrreversible` |
| An unknown effect text is read as a grant (spoofing) | `ParseEffect` refuses it; the store's CHECK refuses it; `Decide` denies an invalid effect |

## Revisit when

A tool needs a rank between two of these, or the owner wants to grant by effect across all connections at once.

Enforced by: TestEveryAnswerThatRefusesNamesItsKind, TestACallThatIsRefusedKeepsTheKindOfRefusal, TestAProgramOnAProtectedConnectionIsRefusedByARailNotForWantOfAGrant, TestTheEffectsAreAScaleOfWhatCanBeUndone, TestAnEffectIsParsedOnceAtTheBoundary, TestMigration54RanksAWriteGrantAsIrreversible
