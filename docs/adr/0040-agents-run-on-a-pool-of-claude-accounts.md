# 0040. Agents run on a pool of Claude accounts

Status: accepted, 2026-11-02; its logins live in the AI runtime by ADR-0058 (2026-11-04), which signs accounts in and out for the core; a plugin provides accounts beside the logins by ADR-0071 (2026-11-10); a subscription also runs agents by its OAuth token, a connection of the bundled plugin `claude`, by ADR-0129 (2026-11-23). Extends ADR-0006 (one login) and ADR-0005 (a card waits for a provider). Source: the owner's
request to connect several Claude accounts, and to give the sign-in link to the owner from the server's own Claude Code client.

## Context

Every agent ran on one Claude Code login (ADR-0006). When that account hit its usage limit, every card waited for it
(ADR-0005), for hours, or for days on a weekly limit, while the owner holds more than one subscription.

Claude Code keeps its login in its config dir (`CLAUDE_CONFIG_DIR`; on macOS the Keychain entry is named after the dir's path), so
one config dir is one account. Session transcripts live in `<config dir>/projects/`, and resuming a session replays its transcript.
Anthropic's terms do not allow an application to hold or forward Claude.ai credentials for its users: the owner signs in with
the unmodified `claude` binary through Anthropic's own flow.

## Options

A. An account pool: one config dir per account, the server picks the account a session runs on, charges failures to that
   account and moves a card to another usable one.
B. One account, with a second one switched in by hand when the first is out (and an account added by telling the Assistant, which cannot run while every account is out). Rejected: the owner is the net, and every
   waiting card stays stuck until he acts.
C. A proxy that holds the tokens of several accounts and rotates them. Rejected: it intermediates Claude.ai credentials, which
   Anthropic's terms exclude.
D. API keys. Rejected for now: a different cost model than the owner's subscriptions (see *Revisit when*).

## Decision

Option A.

- An account is a Claude login in a config dir of its own, known to the owner by the address it signed in as. It is stored in
  the `accounts` table (migrations 63 and 64) with its `id`, which names its config dir, and its `email`. The account `default` always
  exists and is the machine's own login (`-claude-config-dir`); it cannot be removed. Its address is read from that login at start
  (`claude auth status`), since no sign-in of Remit recorded it.
- An extra account lives in `<dir of -claude-config-dir>/claude-accounts/<id>`. Its `projects/` is a link to the default
  account's, so a session resumes on any account (`agent.ClaudeAccounts.Prepare`).
- A card has one `Account`, chosen when its session starts (`chooseAccount`: its own while usable, else the least loaded
  usable one). `ClaudeLauncher` gives the session `CLAUDE_CONFIG_DIR=<that account's dir>`.
- The breaker of ADR-0005 is kept per account (`outages`): a failure is charged to the account the card ran on, and a probe
  runs for each account. The Health bar speaks only while no account can run agents, and says why each is out; the Accounts page lists every account that is out, each by its address.
- Choosing the account and storing it on the card is one step (`placeMu`), for a card that starts and for one that moves, so cards
  placed together are counted one after the other. The chain's intake is placed the same way and goes on another account when its
  own fails; when none is usable its card says it waits for an account, the chain stays queued and the intake is hired again
  (the queued fact is recorded again, `requeueIntake`) once an account can run agents.
- A card whose account fails moves to another usable account at once and resumes its session there; when none is usable it
  waits as in ADR-0005 and is looked at again on the soonest cooldown.
- The owner adds an account with the **Add account** button on the Accounts page (Integrations), and removes one with **Remove**.
  Both are rails (`AddAccount`, `RemoveAccount`, the contract's `accountAdd` and `accountRemove`): they sign a login in or out, so
  they are the owner's own act and no agent has a tool for either. It needs no name and no working agent, so the owner can add
  an account while every other one is out. The Assistant has `list_accounts` only.
- Add account makes the server pick an id, make the config dir and run `claude auth login --claudeai` in it through
  `internal/claudecode` (`StartLogin`). The server reads the link the process prints and puts it in an inbox item of kind
  `login`, shown on the Accounts page. The owner opens the link; the sign-in finishes by itself when his browser can reach the
  machine, otherwise he types the code the page shows into the item's write-only field (`SubmitLoginCode`). No agent sees the code.
- When the sign-in ends signed in, the server runs `claude auth status --json` on the dir (`claudecode.Auth.Status`) and keeps
  the account under the address it reports. An address that has an account already is replaced by the new login (the old
  account is removed and its cards move), which is how an account whose login ran out is signed in anew. A sign-in that does not
  end signed in with a Claude address keeps nothing: its config dir is signed out and removed. One sign-in runs at a time.
- The config dir is made before the sign-in and named by an id of its own, not by a hash of the address: on macOS Claude Code
  keeps the login in the Keychain under a name made from the config dir's path, so a dir renamed after the sign-in would lose
  its login. The address is how the owner and the server tell accounts apart; the id is only the dir.

## Consequences

- One account's limit no longer stops the work while another has capacity; cards spread across usable accounts.
- A new account counts as out until its first probe answers, so no card is placed on a login that was never confirmed.
- A card that moves takes its session along through the shared transcripts. If an account's login cannot read the session,
  `ClaudeLauncher.Start` falls to the replacement-session path of ADR-0006 (`ContextLost`).
- Every session can read the config dirs of the accounts on the machine, as it already could read the default one.
- Removing an account signs it out (`claude auth logout`) and deletes its config dir, so nothing of it stays on the machine.
- A server that stops mid sign-in ends it and removes its dir. A crash leaves a dir under `claude-accounts/` that no account
  names; it holds a login nobody uses and is removed by hand.
- ADR-0006's "the machine's own login" becomes "the default account's login plus the accounts the owner added".

Enforced by: TestASessionRunsInTheConfigDirOfItsAccount, TestEveryDatabaseHasTheDefaultAccount, TestAccountsAreKeptOldestFirstAndTheDefaultStays, TestMigration64NamesTheMachinesLoginDefault,
TestAStartNamesTheDefaultAccountByItsLogin,
the account tests in `internal/orch/accounts_test.go` and `account_signin_test.go`, the login and status tests in
`internal/claudecode`, `TestAnAccountIsKnownByItsLoginsAddressAndDiscardedWhole`, the `accountAdd` and `accountRemove` cases of the
rpc leave and replay tests, `AccountsView.test.ts`, `TestAnIntakeRunsOnAnAccountThatTakesWork`, `TestCardsStartedTogetherAreSpreadOverTheAccounts`.

## Revisit when

- Sessions must not read another account's config dir (hide the others from the sandbox).
- The number of accounts, or a limit per account, needs a control (a cap, an owner-set weight).
- An account backed by an API key, Bedrock or Vertex is wanted next to subscriptions.
- Moving a session between two genuinely different logins loses context in practice (not yet observed against two real logins).
- Signing in on a headless Linux machine, where the owner's browser cannot reach it, needs a smoother path than typing the code.
- Login links and codes need hardening (an expiry shorter than the process's lifetime, a rate limit on code attempts).
- Dirs left by a crash mid sign-in should be swept at start.
- Accounts differ in capacity (a plan's size) and agents are counted equally: an account may need a weight.
