# 0142. A project chooses how each repository is opened

Status: proposed. The owner chose to keep approved local worktree commits on the chain branch. Extends ADR-0141.

## Context

Projects can name a forge remote or a local folder. Today the name also determines where the agent works. A local Git checkout can supply either the working directory or the Git object store for an isolated worktree, including when the checkout has an origin of its own.

## Options

1. Add a project-wide worktree switch. A project with both a shared remote and a personal checkout would have to give both the same policy.
2. Give each project repository a typed opening policy. The agent sees where work starts and where an approved result will be recorded before it opens the repository.

## Decision

Option 2. The owner tells the Assistant how each repository should be opened. The Assistant stores the policy through the project tools. The Projects page shows the policy and the Chair uses it when opening the repository.

| Opening policy | Where agents edit | Where approval records the result |
| --- | --- | --- |
| Forge remote | A chain worktree from Remit's clone | The forge base through the repository merge queue |
| Local folder in place | The given folder on the Remit server | The folder checkpoint, with a commit on its current branch if it is Git |
| Local Git worktree | A chain worktree from the selected checkout's committed HEAD | The chain branch in that local repository |

The owner may give the Assistant a Git origin or a path on the Remit server. An origin identifies a forge remote. A local path lets the owner choose in-place work or an isolated worktree. The local worktree starts from committed HEAD and leaves the checkout's working files and branch ref alone. Its approved result shows the branch name and commit on the chain. Archiving may remove the chain worktree and retains the branch.

Use a tagged repository policy in the shared model. Existing remote strings migrate to the forge policy and saved paths migrate to the in-place policy. A local folder without Git is eligible for the in-place policy. The local worktree policy requires Git. Opening and landing use the policy recorded by the chain so a later project edit cannot redirect work already under review.

## Revisit when

- A local worktree needs a declared local integration target. Add an explicit landing policy and serialize updates to that branch.
- Installations need directory authorization. Enforce it at the shared local repository open boundary before creating a worktree.
