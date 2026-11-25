# 0074. A running session moves to another account without a restart

Status: accepted, 2026-11-11; its option 4 (`claude setup-token`) is how a subscription is added beside the logins by ADR-0129 (2026-11-23). Extends 0040 (the pool of Claude accounts and failover) and 0058 (the runtime holds the
logins). Source: the owner's goal of 2026-11-11, "switch the account without reloading the agent", after the research in
`docs/research-claude-account-hot-swap/`; built in the Rust backend only (ADR-0072), the owner's choice of the same day.

## Context

The customer is the owner, whose agents stop when an account reaches its limit. Today failover closes the session and
opens it again with `--resume` on the next account's config dir (`internal/orch/failover.go`, `internal/agent/claude.go:95`):
the process restarts, the turn in progress is cut, and the card waits for a new process to load its transcript.

Facts (Claude Code 2.1.322, proofs in the research folder):

- A process started with `--input-format stream-json` takes
  `{"type":"update_environment_variables","request_id":…,"variables":{"CLAUDE_CODE_OAUTH_TOKEN":…}}` on stdin, answers
  `control_response success`, and its next request to the model, also inside the turn in progress, goes out with that
  token; the process, its pid and its session stay (`poc.py`, `mid_turn.py`).
- A token given that way carries no refresh token: once it expires (8 hours after it was minted) a request fails with 401.
- A config dir's login (macOS Keychain item `Claude Code-credentials-<sha256(dir)[:8]>`, elsewhere `.credentials.json`)
  holds the access token and the refresh token. Each refresh issues a new refresh token; a second holder of the old one
  gets `invalid_grant`, and Claude Code then clears that login.
- Claude Code renews a config dir's login when it starts, before any request to the model: an idle dev account whose token
  had expired 27 hours before held a fresh one 3.2 s after a process was started on its dir and initialized.
- A request refused with 429 at the usage limit is not retried on a subscription login.
- Prompt caches are isolated between organizations: the first request after a move between two subscriptions reads the
  whole context without the cache.

## Options

1. **Close and resume (today).** Simple; restarts the process and cuts the turn.
2. **Remit refreshes tokens itself** with Claude Code's OAuth client id. Couples Remit to an undocumented endpoint, and puts
   a second holder beside Claude Code's own refresh of the same login.
3. **The runtime hands a session the access token of another account's stored login, which Claude Code alone renews.**
   The refresh token never leaves its store; the runtime only reads, and starts Claude Code on the account's dir to renew.
4. **`claude setup-token` per account** (a token valid a year). A different sign-in for every account, a long-lived
   credential Remit would have to keep; not needed for the goal.

## Decision

Option 3.

- `claudecode::Conn::set_oauth_token(token)` writes the frame and waits for its answer. `claudecode::Auth::fresh_token`
  reads the config dir's stored login and, when its token is good for less than the margin asked, starts Claude Code on
  the dir, waits until the store holds a fresh token, and ends that process.
- The core asks the runtime `session_move(session_id, account)`. The runtime takes a fresh token of that account, sets it
  on the live session, and keeps it fresh: before it has 15 minutes left it renews the account and sets the new token.
  Moving again, or closing the session, ends that keeping. A session moved back to the account of its config dir is set
  that account's token the same way: one path for every move.
- Failover (`failover.rs`, ported from `failover.go`) moves the live session instead of closing it; a session not open is
  opened on the new account as today. The card's account is recorded as before, so a later open starts on its dir.
- Only claude.ai logins move this way. An account of a plugin (ADR-0071) whose session runs on an API key has no stored
  login to read; a session on one is closed and resumed as today.
- ADR-0040's "a subscription is a login the owner signs in himself, never a token handed over" holds: the owner still
  signs every account in himself; the runtime passes the access token of that login to a session it runs, and no plugin
  or other party gives or receives one.

## Consequences

- No restart, no resume, no cut turn on a move; the first reply after a move between organizations is slower (no cache).
- After a move, Claude Code reads the plan and rate tier of the session from `CLAUDE_CODE_SUBSCRIPTION_TYPE` and
  `CLAUDE_CODE_RATE_LIMIT_TIER`, unset, so plan-gated features of Claude Code's own (remote sessions, an org pin) treat
  the session as planless, and its `oauthAccount` stays the first account's. Remit uses none of them; the card's account
  is Remit's record of where the session runs.
- A request in flight when the token is set finishes on the old account; a request already refused at the limit fails its
  turn, and the core sends the message again on the moved session (no restart).
- The runtime reads the Keychain with `security`; a runtime started where the login Keychain is locked (over ssh on macOS)
  cannot read it, and the move falls back to close and resume with the error recorded.

## Tests (written first, main path first)

- claudecode: the next turn runs on the token set on a running process (fake Claude); the Keychain item name of a config
  dir; a stored login gives its token, a cleared one none, and the token never prints.
- airuntime: `session_move` sets the account's token on the live session and keeps it fresh; a move of a session not open
  is refused.
- orch: failover of a card with a live session moves it and does not close it.

## Revisit when

- Claude Code changes the frame, the accepted variables or where it stores a login (a build after 2.1.322): the tests
  against the fake pin our side; the research folder's scripts check the real one.
- The pushed token should also carry the plan: Claude Code accepting `CLAUDE_CODE_SUBSCRIPTION_TYPE` over stdin.
- Moving before the limit is reached (on `rate_limit_event`'s warning) instead of after: a scheduling choice in failover,
  not a change of this mechanism.
- Security hardening of the token in transit (core never sees it; runtime memory only): threat model when asked.
