# ADR-0131 · One independent review covers code and behaviour

Status: accepted. Supersedes ADR-0110 and the gate policy in ADR-0113.

## Context

The LifeOS dogfood gave a small CLI change separate reviewer and QC seats. The two seats both reviewed versions of the same change, with fresh builds after a follow-up commit. The user asked for one independent review covering both the diff and the evidence for the acceptance criteria.

## Options

- Keep separate gates and reduce the checks each agent runs. The queue still waits for two agents for every ordinary change.
- Put code and behaviour in one independent verdict. The reviewer owns acceptance and can use deterministic results from the exact change. The Chair can request a separate verdict for a distinct check.

## Decision

A new landing requires the reviewer verdict. The reviewer reads the diff, checks the acceptance criteria and runs or inspects independent evidence tied to the version. On revisions, the reviewer checks the new diff and repeats affected checks. The Chair uses `land.qc_reason` when a distinct environment or method calls for a separate QC verdict. The reason is recorded in the landing description, and the landing waits for both verdicts. The repository's declared checks still run at landing. The security gate continues to apply to every declared rail and to changes a decider identifies as touching a rail.

The landing records required gates for its version. Existing recorded versions retain their gates, and replacing an open version carries its separate QC requirement forward. `Submit.qc_reason` is optional in stored submissions. An absent reason means the Chair did not request a separate check. A stand-alone reviewer verdict carries `behaviourEvidence`. Existing approvals retain their original roles and patch identity. A historical review without this evidence cannot satisfy the stand-alone gate. Existing two-gate versions use their separate QC verdict.

The Chair requests the review through its existing tools. The chain's Verification tab shows the gates and their evidence. The merge queue continues to enforce the gates and bind approvals to the patch they cover.

## Consequences

Ordinary changes have one handoff for review. A separate QC agent takes a named verification task and supplies its evidence to the reviewer. Work requiring a security verdict keeps the separate security role. Seeded-fault trials can still exercise the two roles independently so the trial measures who catches a defect.

## Revisit when

- The same version needs machine-verifiable evidence for specific acceptance criteria. The landing's required gates and recorded verdict findings are the seam for binding a check's stored result to a criterion and revision.
- Multiple independent reviews demonstrably catch distinct defects on a class of changes. The landing's gate policy is where that class can require the relevant second verdict.
