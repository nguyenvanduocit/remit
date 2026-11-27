# 0139. A landing in place is a commit on the folder's own branch

Status: accepted, 2026-11-27. Source: the owner's question of 2026-11-27 ("what does landing mean now? can land be a merge
commit?") and his go-ahead ("think it through yourself and do it"). Narrows ADR-0137: Remit writes a folder's own Git
repository when a version of it lands.

## Context

ADR-0137 lands a version of a folder worked in place once its gates approve its checkpoint diff, and never writes the
folder's own `.git`. Nothing moves when it lands: its files were in the folder before the gates ruled and stay there if
they refuse. "Landed" then names no event, the screen says "Lands on <folder>" and "Landed" for a mere approval, and one
word carries two meanings. A user whose folder is a Git repository expects an approved change to be in their history.

## Options

1. **A commit on the branch the folder is on.** Once the gates approve, Remit commits the version's paths, with the
   content of the approved checkpoint, on the folder's current branch, with the Chair's title and description. The
   checkout does not move.
2. **A merge commit after working on a branch checked out in the folder.** The folder is switched to a branch of the
   chain's while it works: the user's checkout moves under them, their uncommitted edits travel or conflict, and it is
   a worktree in all but name: the friction ADR-0137 removed.
3. **A merge commit of a side line of checkpoint commits, without a checkout.** Keeps each step of the chain in the
   history, at the cost of separating the user's edits from the chain's in a three-way tree. Its value, the steps, is
   reached later from option 1 without changing what agents do.

## Decision

Option 1.

- **A Git folder** is one that is the top of a Git working tree; a folder inside a larger repository is not, as that
  history is not the folder's to write. When a version's gates approve, Remit commits it on the folder's current branch (or detached HEAD):
  a tree of the folder's `HEAD` with the version's paths taken from the approved checkpoint (written, or removed when the
  checkpoint has no such path), the Chair's title and description as its message, built with a temporary index and
  `commit-tree`, so no hook runs and the user's own index and working tree are not rewritten; the branch moves only if it
  still points where Remit read it (compare-and-swap), and a moved branch is read again. The version's paths in the user's
  index are reset to the new `HEAD`. The landing records the commit, and is `landed`: its change is in the folder's history. A commit
  that fails leaves the version judging, and Remit commits it again at its next look at open landings.
- **A folder that is not Git.** Its gates' approval is the landing: it is `landed`, and it is shown and said as
  approved, never as landed or as landing somewhere.
- **A refusal.** The version returns to its Chair, which is told that its files are still in the folder: fix them and
  land again, or `restore_checkpoint`.
- The `land` tool says what it does in place: checkpoint the folder and send the version to its gates; once approved, a
  commit on the folder's branch in a Git folder.

## Consequences

- An approved change in a Git folder is an ordinary commit the user can read, push or `git revert`.
- The user's uncommitted edits to the same paths, when they were in the folder at the approved checkpoint, go into that
  commit: the checkpoint is what the gates approved.
- The commit is the user's repository's own, made with its own configured identity; Remit pushes nothing.

## Revisit when

- The chain's steps are wanted in the history: option 3, a merge of a side line of checkpoint commits.
- A user's edits and an agent's need separating within one path (ADR-0137's per-turn checkpoints).
