# 0011. The GitLab webhook ships with Remit, as a built-in playbook does

Status: accepted, 2026-10-29; superseded in part by ADR-0024: the webhook it ships is the builtin event signal `gitlab`, and its builtin rule stands. Source: the owner's requests of 2026-10-29 (a built-in webhook for the default GitLab handling, then
its preconditions open to his own rules so that a repository can be ignored), `docs/design.md` (Webhooks), and ADR-0008.

## Context

The rules that tell a Chair about a comment, a merge, a close, an approval or a failed pipeline on its merge request were written
in the Assistant's manual (`references/webhooks.md`) as a JSON block. The Assistant copied that block into every GitLab webhook,
and the tests read the block out of the manual and ran it. The behaviour of the system lived in a prose file, each webhook held
its own copy, and a copy could drift from the others.

The webhook the owner runs carries a rule of his own ahead of the shipped ones (let the posts of one repository pass) and a prompt.

## Options

1. Keep the manual as the source and check that stored copies equal it. The rules stay in prose, and a copy per webhook stays.
2. A webhook whose settings are code and which nothing edits. Nothing of the owner's fits beside the rules: an exception that
   takes away from them cannot be written in another webhook, which only adds.
3. A webhook that ships with Remit like a built-in playbook: its settings start as the code gives them and follow it, the
   owner's changes are kept, and he can reset them. The one nearest pattern in the system.

## Decision

Option 3.

- **`gitlab` (`/hooks/gitlab`) is built in** (`Webhook.Builtin`). Its note and preconditions are `gitlabRules` in
  `internal/orch/webhook_builtin.go`, with tests. `SeedWebhooks` runs at each start: it makes the webhook when it is missing, and
  brings it up to what ships while the owner has not changed it. A start that finds all as it should be writes nothing.
- **The owner changes it as any webhook**, by telling the Assistant. His version is kept through every start (`Webhook.Edited`)
  until `reset_webhook` puts back what ships. An edit that sets back what ships follows Remit again.
- **It is never deleted.** The sender is pointed at its address and a start would make it again; pausing it lets nothing in.
- **The webhook the owner opened as `gitlab` becomes it** without losing a word: id, key (GitLab holds it), pause, deliveries and
  settings stay, and the settings are his version when they differ from what ships.
- **The door keeps the data right, for any webhook.** An event a `chair` rule settles names merge requests (its marks); for the
  chains those marks find, the door reads each such MR from GitLab and records how it stands (`followMRs` calling `syncMR`, the
  step the periodic look takes), before it tells a Chair, and also when the chain is finished. The post is never the source: a
  reopen or a merge is recorded only as GitLab answers it, so a forged post changes nothing. A reopened MR becomes open and
  undecided again, which the periodic look, which follows open MRs only, never could.
- **The manual no longer carries the rules.** It says the webhook exists, how to put an exception before the shipped rules and
  when to reset; `list_webhooks` gives its settings as the example for another GitLab.

## Consequences

- One owner for the default GitLab rules, which is code with a test. Every installation that has not changed them gets a fix at its
  next start.
- A webhook the owner changed does not receive a later fix to a shipped rule: it keeps his version, as an edited playbook does.
  Reset takes the fix, at the cost of his rules. A rule that can no longer compile stops the door for that webhook (design.md,
  Webhooks), so a shipped rule that changes its inputs is a decision to take care over.
- The state of a chain's MR no longer lags a merge the owner makes by up to the period of the periodic look, and the Chair that is
  told reads the state already changed. The periodic look stays for posts that never arrive (Remit off, a hook not set).
- **Threats** (STRIDE over the door's entrance; the key, its sealing and the door's checks are unchanged): *Tampering*: only the
  owner's words, through the Assistant, change the rules, as for any webhook, and `skip` and `chair` still lift no rail. *Spoofing/Tampering of data*: a post cannot set an MR's state, only ask GitLab to be read, and the address read is the chain's own, never one from the post; a holder of the key can cause one `glab` read per chain MR it names. *Elevation*:
  an edit cannot make the door tell the wrong chain, since a chair rule still matches by the chain's own marks. *Denial of
  service*: an edit that drops the shipped rules sends every post to Dispatchers, whose limits (pending deliveries, chains a day
  per webhook) hold; reset restores the rules.

Enforced by: TestTheGitlabWebhookComesWithRemit, TestSeedingTheWebhooksTwiceChangesNothing, TestAnEditThatSetsBackWhatShipsIsNoEdit, TestResetPutsBackWhatShips, TestAnOwnersGitlabWebhookBecomesTheBuiltinOneKeepingEverything, TestAnOwnersGitlabWebhookWithTheShippedSettingsIsNotMarkedEdited, TestAWebhookTheOwnerDidNotEditFollowsWhatShips, TestTheBuiltinWebhookIsNotDeletedAndOnlyItHasAnOriginalToResetTo, TestAnotherWebhookBesideTheBuiltinOneIsTheOwners, TestEveryShippedEventSubscriptionPassesCheckAndIsBuiltinOfItsSignal, TestTheShippedGitlabSubscriptionsReadAnEventByItsOwnFields, TestAMergedPullRequestSyncsTheChainAndTellsItsChair, TestATellSkipsACommentFromTheAccountRemitWorksAs, TestATellDoesNothingAndSaysWhyWhenItsChainCannotBeFound, TestATellDoesNotGuessWhoRemitIsAndGivesUpAfterAWhile

## Revisit when

- A shipped rule changes and edited webhooks need it too (a merge of the shipped rules into his version would then be worth building).
- A second sender gets a shipped webhook (Sentry, Jira), which makes the list of shipped webhooks a catalogue.
