# 0038. A repository is named by its remote, and Remit keeps its own clone

Status: accepted, 2026-11-02. Source: the owner's decision of 2026-11-02 to drop the repository catalog (a chain on aiocean.io
finished its change and could not publish it: `open_repo` refused with "no repository catalog configured"), `AGENTS.md` (One
concept, one owner; Agent-centric), ADR-0012, ADR-0020. Replaces the repository catalog (`repository-catalog.json`, `-vault`'s
`product-data/engineering/`).

## Context

`open_repo` found a repository through a JSON file the owner edited: for each repository an id, an alias, a name, the path of his
checkout under `-vault`, and its forge host and project. All of it except the checkout is what the repository's own `origin`
says, so one fact had two owners and each new repository needed an edit outside the product, which an agent could not make.
The checkout was the owner's working copy: a machine with no source (the pod) had none, and a chain's worktree was cut from a
tree whose state the server did not control.

## Options

1. **Keep the catalog** and let an agent add entries through the owner's gate. The duplicated fact stays.
2. **Scan a root of the owner's checkouts** and read each `origin`. No file, but it still needs the owner's checkouts.
3. **Name a repository by its remote and keep Remit's own clone.** **Chosen.**

## Decision

- A repository is named by where it lives: `host/project` (`github.com/aiocean/aiocean-io`), a remote URL, or the absolute path
  of a checkout (only its `remote.origin.url` is read). A repository Remit already holds is also named by its project path
  or last element when that is unambiguous. `parseRemote` and `resolveRepo` (`internal/orch/repo.go`) are the one place that
  reads a name.
- Remit keeps one clone of each repository at `<data>/repos/<host>/<project>` (`Config.ReposDir`). The first `open_repo`
  clones it, from the URL as given or, for `host/project`, `git@<host>:<project>.git`, with the machine's own credentials, the
  same ones a push uses. Each later open fetches. A clone is made beside its place and moved in, so two chains opening the same
  repository never see half of one. Every chain's worktree is cut from `origin/<base>` of that clone, never from a working copy.
- `RepoWork.Repo` is the repository's project path on its forge; `Host` says which forge. A chain holds one repository of
  a given project path.
- A host must have a forge in `-forges`; that and the forge account's rights are the limit on what can be opened. Merging stays
  the owner's (the rails of `AGENTS.md` are untouched).
- `-vault` keeps one use: the credential env files in `<vault>/secret`.

## Consequences

- A repository needs no registration: the owner tells an agent which one, in words, and the agent names its remote.
- A machine with no checkouts (the pod) runs chains on any repository its git credentials can clone.
- A chain's work starts from the remote's default branch (Git Flow: `develop` where it exists), not from the owner's working
  copy, which can be ahead of it (ADR-0012 already states this).
- A bare name resolves only for a repository already cloned; a chain that is given a name for a new one asks for its remote.
- Records opened before this change keep their stored `repo` key; one stored without a host cannot be published until it is
  reopened.
- An agent can name any repository the machine's git credentials reach. The result is still a draft MR the owner reviews.

## Revisit when

An outsider or an agent opens a repository the owner did not intend (add an allow-list or deny-list of `host/project`, checked in
`OpenRepo` before the clone, listed as a capability a playbook may carry); clones fill the disk (prune clones no chain holds);
a forge host needs another clone URL than `git@<host>:` (the forge adapter, ADR-0020, names it); a host's repository names
collide on `Project` alone (key `Repo` by host as well).
