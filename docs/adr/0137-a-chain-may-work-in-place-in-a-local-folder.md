# 0137. A chain may work in place in a local folder

Status: accepted, 2026-11-27; a landing in place commits on the folder's branch by 0139. Source: the owner's request of 2026-11-27 ("Remit forces git and worktrees, which is hard;
design a mode that runs on the local file system, as Paseo does"). Narrows ADR-0038 and ADR-0114 for a folder opened in
place; clone work is unchanged.

## Context

Every chain that changes files needs a remote on a supported forge with an account that pushes (`open_repo` refuses
anything else, `crates/shipping/src/repo.rs`), a clone in `<data>/repos`, a worktree on `feature/mc-*` inside the chain's
directory, and `land`: checks, gates, the merge queue and a push. A Remit user is one person on their own machine. A
folder with no remote (a side project, notes, a script directory, a repository that is not Git at all) cannot be worked
on, and a folder that has a remote sees the agents' work only after the queue pushed it and the user pulled. A path given
to `open_repo` is read for its `origin` and cloned again: the user's own checkout is never what the agents touch.

## Options

1. **A server-wide local mode.** One flag turns every chain into in-place work. A shared repository then loses its queue
   and its reverts on the same server where a personal folder gains speed; the choice is per folder, not per server.
2. **A local Git repository without a forge, still through a worktree and a local merge.** Keeps the queue's guarantees,
   but the user still waits on worktrees and a merge to see a change, which is the friction the owner named, and a folder
   that is not Git stays out.
3. **A folder opened in place, with checkpoints Remit keeps outside it.** Agents work in the folder itself; every change
   is on disk the moment it is written; Remit keeps a shadow Git directory per folder so reviewers read a diff and a
   change can be taken back, whether or not the folder is a Git repository.

## Decision

Option 3.

- **One concept, two places.** `RepoWork` says where the chain's work on a repository lives: a clone (host, forge path,
  worktree, branch, base, as today) or a folder in place (its absolute path). Stored chains migrate forward once, every
  existing `RepoWork` becoming a clone.
- **Opening.** `open_repo` given an absolute path to a directory opens it in place: no remote, no forge account, no clone,
  no worktree. A remote or a project's repository name opens a clone, as before. The agents that change, review or
  verify the work are hired with the folder as their cwd.
- **Checkpoints.** Remit keeps a shadow Git directory for the folder under its data directory
  (`git --git-dir=<data>/folders/<id>.git --work-tree=<folder>`), so the user's own `.git` is never written. It honours the
  folder's `.gitignore` files. A checkpoint is taken when the folder is opened and at each `land`. The diff a reviewer
  reads is the diff between two checkpoints.
- **Landing in place.** `land` on a folder in place takes a checkpoint and records the version as a `Landing` whose base
  is the previous checkpoint. It runs no `.remit/checks` and pushes nothing. Its gates judge its patch id as for any
  landing; once they approve it, it is `landed`. The files were already in the folder, so a landing in place has no
  merge queue, no batch and no forge checks. A refusal returns it to the Chair, who fixes the folder or restores it.
- **Taking back.** Restoring a checkpoint writes the folder back to it; it refuses when a file it would overwrite changed
  since the checkpoint after it (the user's own edit), naming the file.
- **One chain per folder.** A folder in place is held by one chain at a time: `open_repo` refuses a folder another chain
  that is not done holds, naming the chain. The user may keep editing it.

## Consequences

- A user points Remit at any folder and sees the agents' edits in their editor as they happen.
- A folder in place has no merge queue, no canary stages, no forge checks and no revert onto a shared base: those rails
  remain clone work's. A shared repository is opened by its remote.
- The user's edits made while a chain works fall into the chain's next checkpoint diff; reviewers see them as part of
  the change.

## Revisit when

- The user's edits and an agent's need to be told apart in a diff: checkpoint at each writing turn's start and end.
- Two chains need the same folder at once: a lease with a queue of chains instead of a refusal.
- A folder in place needs checks before a landing: run `.remit/checks` on the checkpoint, as clone work does.
