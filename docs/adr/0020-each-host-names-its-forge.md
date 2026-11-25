# 0020. Each host names its forge, an adapter behind one interface

Status: accepted, 2026-10-30; its merge requests superseded by ADR-0114 (the forge hosts the code, Remit lands it); superseded in part by ADR-0024: the `github` webhook is the builtin event signal `github`, and its forge rule stands. Source: the owner's request of 2026-10-30 (support GitHub beside GitLab, configurable so that any
version-control service can be added), `docs/design.md` (Forges, Merging, Webhooks), ADR-0011.

## Context

Every write Remit makes outside a chain's workspace went to GitLab through `glab`: finding and opening a draft merge request,
retargeting it, reading how it stands and its diff, and the identity its commits are made under. The calls were spread over
`submit.go`, `review.go`, `repo.go` and `door.go`, each building GitLab paths itself. The texts shown to the owner and to agents
wrote a merge request as `!12`, computed from its number wherever it was shown, and the built-in webhook and the door's key check
knew GitLab's way of proving a post only (`X-Gitlab-Token`). A repository on GitHub could be catalogued but never published.

## Options

1. **A branch per forge at each call site** (`if host is GitHub`): the fastest, and every later forge touches every site.
2. **One generic REST client with per-forge path tables**: the forges differ in more than paths (GitLab drafts by title, GitHub by
   a field and labels by a second call; GitHub pages a diff; GitHub has no option to skip CI on push), so the tables grow code.
3. **An interface the orchestrator declares, one adapter per forge, a host→kind map the owner sets.** **Chosen.**

## Decision

- **`orch.Forge`** (`internal/orch/forges.go`) is what the orchestrator asks of a forge: its account, the open MR of a branch,
  open or update a draft MR, retarget it, how it stands, its diff, and the push options a chain's branch carries. It is declared
  by its consumer, as the codebase's Go rules want.
- **`internal/forge`** answers it, one adapter per kind (`GitLab`, `GitHub`), each through the forge's own command line logged in
  on the machine (`glab api`, `gh api`), so no token is held by Remit. A new forge (Gitea, Bitbucket) is one adapter and
  one kind in `ParseHosts`.
- **The owner names each host's forge** with `-forges host=kind,…` (default `github.com=github,gitlab.com=gitlab`), and the
  command lines with `-glab` and `-gh`. A repository on a host with none is refused when a chain opens it, with the flag to set.
- **A merge request keeps its number as its forge writes it** (`MRRef.Ref`: `!12` on GitLab, `#12` on GitHub), written by the
  adapter that opened or read it. Pure code (texts, the board's search, goal and thread headings) shows it with no config.
  Migration 49 gives every stored merge request GitLab's, the only forge before this.
- **A chain's commits are made again, at submit, under the account its forge pushes as** (`pusher`, `pushable`), since each
  forge has its own account. They are made on `refs/remit/push/<branch>`, each original author kept as a
  `Co-authored-by:` trailer, and that ref is what is pushed: the worktree's commits keep their ids. GitHub gives an email only when the account made one public; its noreply address stands in.
- **The `github` webhook ships with Remit** as `gitlab` does (ADR-0011): `githubRules` tell a Chair about a comment, a
  review, a merge, a close, a reopen and a failed workflow run on its pull request or branch. The door also takes GitHub's proof of
  the key, an HMAC-SHA256 of the body in `X-Hub-Signature-256` (`KeyProof`), since GitHub never sends the key itself.
- **The merge rail is unchanged in kind.** The deny rules already refuse `gh pr merge`, `gh pr create` and a `gh api` call that
  names a merge; the forge refuses a merge by the account agents use (design.md, Merging, gives GitHub's ruleset).

## Consequences

- Publishing, following and reviewing a chain's MR are one path for every forge; a forge's differences live in its adapter.
- The texts the owner and agents read say "forge" where they meant GitLab, and name the MR by its own notation.
- GitHub has no push option that skips its checks, so a chain's push runs the repository's workflows; `.github/workflows/` joins
  `.gitlab-ci.yml` among the paths a chain may not change.
- A host must be named before its repositories can be published: an installation whose GitLab is not `gitlab.com` sets
  `-forges` once.
- **Threats** (STRIDE over the door's entrance and the forge calls): *Spoofing*: a signature opens a webhook only when it is the
  HMAC of the exact body under that webhook's own sealed key, compared in constant time; a signature with another key, over another
  body, or not hex is refused (401), and the key itself is never sent by GitHub, so it cannot leak from a captured post.
  *Tampering*: a post still cannot set an MR's state; the door asks the forge about the chain's own address. *Repudiation*: a
  commit published names the forge account that pushed it, never an agent's local identity. *Information disclosure*: the forge
  login stays in its command line's config, read by the server process only; `gh` and `glab` writes stay on the deny and ask
  rules for agents. *Elevation*: an agent with `gh` cannot merge through the deny rules, and the ruleset makes the forge refuse it
  when a raw call gets past them. *Denial of service*: the body is read up to its limit before the signature is checked, as before.

Enforced by: TestGitHubOpensADraftPullRequestLabelledAsRemit, TestGitHubFindsTheOpenPullRequestOfABranch, TestGitHubTellsAMergeFromAClose, TestGitHubReadsEveryPageOfAPullRequestsFiles, TestGitHubsAccountHasANameAndAnEmail, TestGitLabOpensADraftMergeRequest, TestTheOwnersHostsSayWhichForgeEachIs, TestARepositoryOnAHostWithNoForgeIsNotOpened, TestAChainsCommitsArePublishedAsItsForgesAccount, TestMigration49GivesEachStoredMergeRequestGitLabsReference, TestABodySignedWithTheKeyOpensTheWebhook, TestTheShippedGithubSubscriptionsReadAnEventByItsOwnFields, TestTheShippedEventSignalsComeWithTheirSubscriptions

## Revisit when

- A forge has no command line to drive (an adapter would then speak HTTP with a credential behind the gate, ADR-0018).
- One host serves two kinds, or a repository must be published to a forge other than its host's.
