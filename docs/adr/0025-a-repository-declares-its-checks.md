# 0025. A repository declares its checks in `.remit/checks`, run in a jail, and a chain cannot change them

Status: accepted, 2026-11-02; `.remit/release/` written by chains as a rail, `.remit/checks` still closed to them (ADR-0116). Source: the owner's decision of 2026-11-02 that every kind of work lives in Git (`docs/remit-design.md`, K6),
the blueprint's rule that deterministic checkers come before judges (`docs/ai-run-company-blueprint.md`, section 3.5), ADR-0018, ADR-0021,
`AGENTS.md` (Evidence and verification).

## Context

The checks a submit runs (`runChecks`) knew one language. A Go module was built, vetted and tested; any other repository, and with
the owner's decision of 2026-11-02 that is every repository that is not software (a plan, a price list, a policy, a campaign, data), had
no automatic check at all: `runChecks` returned nothing when there was no `go.mod`, and the MR said "no automatic checks for this kind of
repo". What verified such work was the reviewer, the QC and the goal judge: agents of one model family, whose errors are correlated.

A second fact shaped the answer. The checks run the clone's own code on the server as the server's OS user, outside any jail (`repoCommand`:
"what the command runs is the agent's too"). A new kind of check that ran the same way would widen that gap with every repository's
program, and a repository's checks that the chain could edit would let the chain grade itself, which is why the project already forbids
chains to change CI configuration (`forbiddenPath`).

## Options

1. Keep Go only. Non-software work stays verified by judges alone.
2. The repository declares its checks, as executables in `.remit/checks`, and the platform runs them. The checks version with the repository.
3. The owner lists the checks in the repository catalog, outside the repository. They cannot be edited by a chain, but they do not travel with the repository and
   each repository needs a catalog edit.
4. A chain writes the checks for its own change. It grades itself.

## Decision

Option 2.

- A repository's checks are the executable regular files of `.remit/checks` (names beginning with `.` and directories are ignored), run
  in name order, each in a jail of its own, stopping at the first that fails. A check passes when it exits 0. Its output (stderr then stdout, cut) is
  the failure's text, and the MR body lists each check by path.
- A check sees the repository read-only at its own path, a scratch directory as its work directory, and nothing else of the host's files but
  the system paths. It has no network (no `Socket`). Its environment is `PATH`, `HOME` and `TMPDIR` (the scratch directory),
  `REMIT_REPO` (the repository) and `REMIT_CHANGED` (a file of the changed paths, one per line). It ends after five minutes of wall time
  or two of processor time, and keeps 64 KiB of each output stream.
- Nothing is skipped without a word. A file that cannot run (not executable, not a regular file, unreadable) and a jail that cannot be built
  fail the check with the reason, so a submit never passes for want of a place to run its checks. A repository with no `.remit/checks` has none.
- The language's checks run first and the declared ones only if those pass. A repository that is not Go has only the declared ones.
- `.remit/` is in `forbiddenPath`: a chain's change that touches it is refused with the other protected paths. The owner commits
  the checks himself, as he does CI configuration. A check keeps the code it needs under `.remit/`: a program elsewhere in the repository is
  the chain's to edit.

## Consequences

- Any kind of work can carry a deterministic verifier: a schema validator for a configuration, a recomputation of a table's totals, a lint
  for a document, a script that compares a plan to a rule. This is what makes Git the substrate for work that is not software.
- Writing a check is the owner's act, not an agent's inside a chain. That departs from "configuration is changed by talking to an agent" on purpose: a check is
  what grades the agent's work, so it sits with the rails.
- A check is only as good as its author, and the platform cannot tell a weak one from a strong one.
- The language's own checks still run unjailed on the clone (`repoCommand`); only the declared ones run in a jail. Closing that is its own unit.
- Threat model (the checks are a code-execution surface on the gate to publication):

| Threat | Control |
|---|---|
| A chain edits the check that grades it (tampering) | `.remit/` is a forbidden path; the check runs against the repository read-only |
| A check reads secrets and sends them out (information disclosure) | It sees the repository and system paths only, has no network, and the server's files are not mounted; its output is shown to the chain, which can read the repository anyway |
| A check runs forever or fills memory or disk (denial of service) | Wall and processor limits, output caps, stop at the first failure; macOS applies no memory limit (ADR-0021) |
| A check breaks out of the jail (elevation) | The jail of ADR-0018 and ADR-0021: user, mount, network and pid namespaces on Linux, a Seatbelt profile on macOS |
| A check is missing or cannot run and the submit passes (repudiation of assurance) | Fail closed: an unrunnable file or an unbuildable jail is a failed check |
| A check calls a program elsewhere in the repository that the chain changed (tampering) | Stated above: a check keeps its code under `.remit/`; the platform cannot enforce what a check calls |

## Revisit when

Owners ask an agent to maintain their checks: then an owner-approved path for changing `.remit/` (a change item, as for the rails) replaces
the owner committing them by hand. Or the language's own checks move into the same mechanism, so the jail covers them too.

Enforced by: TestASubmitIsGradedByTheChecksTheRepositoryDeclares, TestADeclaredCheckPassesOrFailsOnItsExitCode, TestADeclaredCheckSeesTheRepositoryReadOnlyAndWhatChanged, TestADeclaredCheckThatCannotRunFailsInsteadOfBeingSkipped, TestADeclaredCheckFailsWhenTheJailCannotBeBuilt, TestDeclaredChecksRunAfterTheGoChecksAndOnlyIfTheyPass, TestAChainMayNotChangeTheChecksThatGradeIt
