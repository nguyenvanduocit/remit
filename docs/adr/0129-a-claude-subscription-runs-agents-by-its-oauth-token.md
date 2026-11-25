# 0129. A Claude subscription runs agents by its OAuth token

Status: accepted, 2026-11-23. Amends ADR-0040 (its line that a subscription is only ever a login signed in on the machine),
ADR-0071 (its allowlist, which refused `CLAUDE_CODE_OAUTH_TOKEN`) and ADR-0074 (its option 4, rejected then as not needed).
Source: the owner, 2026-11-23 (translated): "switch this account from the login to an oauth-token", about the account
`default`, which had reached its weekly limit ("You've hit your weekly limit · resets Nov 27", `~/.remit/remit.log`,
every probe since).

## Context

The customer is the owner, whose agents all wait while the one login on his machine is at its limit. He holds other Claude
subscriptions. Adding one as a login (ADR-0040) runs `claude auth login` in a new config dir on the server's machine; its
refresh token then lives there, and any other holder of the same login (another tool on the same machine, the same account
signed in elsewhere) rotates it out from under Remit (`invalid_grant`, ADR-0074 *Context*).

Facts (Claude Code 2.1.328):

- `claude setup-token` signs a Claude account in and prints an OAuth token good for a year, for programs to run Claude Code
  on that subscription. It needs a terminal: run with piped stdio, as Remit runs `claude auth login`, it prints nothing
  (tried 2026-11-23), so Remit cannot run it for the owner.
- Claude Code reads `CLAUDE_CODE_OAUTH_TOKEN` from its environment in place of its config dir's login, and keeps it out of
  what it starts when `CLAUDE_CODE_SUBPROCESS_ENV_SCRUB` is set (the name is in its list of credentials, read from the
  binary).
- A plugin's connection is already an account (ADR-0071): its credential is typed by the owner into a write-only field,
  sealed in the store, and given at each session's open as variables the allowlist accepts.

## Options

1. **A login, as today.** Needs a sign-in on the machine, and the login is shared with every other holder of it.
2. **The owner pastes the token into a setting of the default account.** A field on the machine's own account, a second way
   to say what one account runs on, and a form.
3. **A bundled plugin `claude` whose connections are subscriptions, each by its token.** The existing path of ADR-0071:
   the token is a connection's sealed credential, the account is derived from it, the session gets it as a variable.

## Decision

Option 3.

- `crates/bundled/src/claude.rs` ships the plugin `claude` ("Claude subscription"): one secret variable,
  `CLAUDE_CODE_OAUTH_TOKEN`, no tool, and an `Account` that answers `{CLAUDE_CODE_OAUTH_TOKEN: <the token, trimmed>}`.
  Each configured connection is an account `claude.<connection>`.
- `agent::parse_account_env` accepts `CLAUDE_CODE_OAUTH_TOKEN`; the core and the runtime parse with that one list, as
  before. Everything else of ADR-0071 holds: the account is derived and never stored, placed, probed and failed over as
  any other; a session moved onto it is closed and resumed (`failover.rs`, it has no stored login to read).
- The owner adds one by telling the Assistant, which calls `request_source_credentials` for `claude`; he types the token
  into the item. The Assistant's manual (`builtin:app references/accounts.md`) says how. No agent sees the token.
- The machine's own login stays the account `default`: when it is at its limit it is out, and agents run on the
  subscriptions he added. Ordering is unchanged: logins first, then plugin accounts.
- ADR-0040's terms line read Anthropic's terms as excluding an application that holds Claude.ai credentials *for its users*.
  Remit runs for one owner on his own machine, with his own token, made by Anthropic's own command for this use; it
  forwards it to no one else.

## Consequences

- A subscription runs agents with no sign-in on the server's machine and no refresh token there to lose.
- The token is a year-long credential sealed in the store (ADR-0017's limit applies: an agent running as the same OS user
  can read what the server reads). It reaches a session's environment and, scrubbed, none of its tools.
- A session on a token account does not move live (ADR-0074): it is closed and resumed on another account.

## Revisit when

- A token nears its year: the account should say when it expires before it fails.
- A token account should move live between accounts: `Conn::set_oauth_token` takes the token as is, with no renewal.
- Hardening of the credential: a token readable by a same-user agent (ADR-0017), revocation when the connection is removed.
