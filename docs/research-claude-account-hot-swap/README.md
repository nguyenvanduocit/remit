# Switching a running Claude Code session to another account

Research, 2026-11-11, on the minified source of Claude Code 2.1.322. Question: how Claude Code finds its login, and
how Remit can move a session to another account without closing and resuming it.

**Answer:** a session started with `--input-format stream-json` takes a new OAuth access token from one stdin line, and
its next turn runs on it. `poc.py` here shows it.

## The stdin line

```json
{"type":"update_environment_variables","variables":{"CLAUDE_CODE_OAUTH_TOKEN":"<access token>"},"request_id":"x"}
```

- Handler: `structuredIO.processLine` in `chunk-zjsmbvvt.js`. It sets `process.env`, clears the token caches (`U_()`:
  the cached login, the cached keychain read), signals that the login changed, and answers
  `{"type":"control_response","response":{"subtype":"success","request_id":"x"}}`.
- Accepted keys: `CLAUDE_CODE_OAUTH_TOKEN`, `CLAUDE_CODE_SESSION_ACCESS_TOKEN`, `CLAUDE_CODE_SESSIONS_OAUTH_TOKEN` (the set
  `Z` in the same chunk). Any other key is refused and logged; `ANTHROPIC_API_KEY` is not one of them.
- Anthropic's remote runner rotates its children's tokens through the same line (`chunk-0p6farb3.js`, "pushed to child
  via stdin"), so it is a supported path.

**Test** (`python3 -I poc.py`, haiku): turn 1 answers `ONE`; the pushed fake token is acknowledged with `success`; turn 2
fails with `401 OAuth access token is invalid`. The pushed token replaced the keychain login on the next turn. Run on
2026-11-10 and again on 2026-11-11 with the same result. A switch to a second real account (`poc.py <token>`) has not been
run: it belongs on the dev instance with two of Remit's test accounts, never with a seat's own login.

**Mid-turn test** (`python3 -I mid_turn.py`, 2026-11-11): the token is pushed while the agent's `sleep 8` tool call runs.
The acknowledgement comes back at once (4.4 s, the moment of the push); the tool call finishes; the turn's next model
request, inside the same turn, already uses the pushed token (401 at 16.9 s). The process keeps its pid and stays alive:
no restart, no resume, no pause. With a real token the turn goes on uninterrupted. The request in flight at the moment of
the push finishes on the old token. The one visible cost is the first request after a switch between organizations,
which reads the whole context without the prompt cache and so answers slower on a long conversation. A request refused with 429 at the usage limit is not retried on a subscription login: the retry rule
returns `!Tt()||ame()||aDn(e)` for a 429 (`chunk-9r1hbf9p.js`), where `Tt()` is a claude.ai login, `ame()` the Enterprise
plan and `aDn(e)` a 429 without the `anthropic-ratelimit-unified-*` headers that mark the usage limit. That turn fails;
the process stays, and the host pushes a token and sends the message again. Recommended design for the ADR: push the
token on the `rate_limit_event` warning, before the limit, so no request is refused.

**Rust check on real Claude Code** (2026-11-11, `agent::Session::move_to`, two dev accounts): a session on an account at its
weekly limit failed turn 1 with that limit; `move_to` the second account took 146 ms (its stored login read, a fresh token
set); turn 2 in the same process and session answered `TWO`. Claude Code renews an idle account's expired login when it
starts on that dir, before any model request (3.2 s measured), which is how `Auth::fresh_token` renews.

## Where Claude Code finds its login (`chunk-yqezv9gr.js`)

| Order | Source | Notes |
|---|---|---|
| 1 | `CLAUDE_CODE_OAUTH_TOKEN` env | No refresh token: once it expires the next call gets a 401 that does not recover. Plan and rate tier come from `CLAUDE_CODE_SUBSCRIPTION_TYPE` and `CLAUDE_CODE_RATE_LIMIT_TIER` (`hg()`). |
| 2 | `CLAUDE_CODE_OAUTH_TOKEN_FILE_DESCRIPTOR` | Read once. |
| 3 | Stored login | macOS: keychain entry `Claude Code-credentials-<hash of the config dir>`. Linux: `<config dir>/.credentials.json`. Claude Code refreshes it itself under `<config dir>/.oauth_refresh.lock`. |
| — | API keys | `ANTHROPIC_API_KEY`, or the `apiKeyHelper` command set in settings. |

Two other paths pick up a changed login; neither suits Remit:

- **The stored login is watched** (`XC`, `Dl`): Claude Code compares the modification time of `.credentials.json`, or the
  token in the keychain, and reloads when it changes. Using it means rewriting an account's stored login under a running
  session, and every other session on that account reads the same login.
- **Recovery after a 401** (`vj`): when the stored token differs from the one that failed, Claude Code takes the stored
  one. It fires only after a 401, never on a usage limit.

## What changes after a switch

- **Plan and tier come from env.** The plan getter `ime()` reads the cached login, which after a switch is rebuilt from the
  env token (`hg()`): plan and tier are `CLAUDE_CODE_SUBSCRIPTION_TYPE` and `CLAUDE_CODE_RATE_LIMIT_TIER`, `null` when
  unset, and neither can be sent over stdin. The plan's eight call sites in four chunks are feature eligibility checks,
  none is inference: remote-session eligibility (`unsupported_subscription` when `null`, `chunk-av62kgta.js`), a
  plan-gated feature (`plan_unreadable`, `chunk-5wt8czvv.js`), Team/Enterprise organization pinning
  (`chunk-av62kgta.js`), and an auth-diagnostics record (`chunk-r4rjhc57.js`).
- **The account record stays the first account's.** The config dir's `oauthAccount` (organization uuid) is not reloaded;
  it is read, for example, to check the organization's default model (`nBt`).
- **API-key accounts switch through `apiKeyHelper`.** Its output is cached 5 minutes (`UW=300000`, or
  `CLAUDE_CODE_API_KEY_HELPER_TTL_MS`); once expired, the request in progress keeps the old key while the helper runs
  again, and the next request takes the new one. Any 401 drops the cache and retries the request
  (`if(e.status===401)return ISt(),!0`, `chunk-9r1hbf9p.js`).
- **The prompt cache does not cross organizations.** *"Caches are isolated between organizations. Different organizations
  never share caches, even if they use identical prompts"*, and on the Claude API also by workspace
  (https://platform.claude.com/docs/en/build-with-claude/prompt-caching). Separate subscriptions are separate
  organizations, so the first turn after a switch reads the whole context without the cache. Whether two seats of one
  Team organization share a cache is not documented for subscription traffic (inference: same organization, likely yes).

## What it means for Remit

Today `internal/orch/failover.go` closes the session and resumes it with `--resume` on another config dir
(`internal/agent/claude.go:95`). The stdin line removes that restart. What has to be decided first:

- **Remit keeps the token fresh.** A token passed by env has no refresh token; Remit pushes a new access token before the
  old one expires.
- **One holder per refresh token.** Each refresh issues a new refresh token and the other copy then fails with
  `invalid_grant`; Claude Code marks it dead and clears it from the stored login (`VRo`). Remit hands out only access
  tokens, and one owner (the runtime that keeps the logins) refreshes each account.
- **ADR-0040.** It reads "a subscription is a login the owner signs in himself, never a token handed over"
  (`internal/agent/accountenv.go:37`). The runtime passing an account's own access token keeps the owner signing in
  himself, and the ADR has to say so.
- **Sessions start on an env token** with both plan variables set to the pool's plan, so a switch changes only the token.
- **API-key accounts** switch through an `apiKeyHelper` that asks Remit for the current key.
- **Remit's own record names the account** a session runs on; the session's own account record is stale after a switch.
  The prompt cache is lost once per switch between organizations.

Next step: an ADR (architect and chain-runtime agents) deciding who refreshes each account's token, the ADR-0040 change,
how sessions start on an env token, and how `failover.go` pushes a token instead of closing and resuming. First test: a
switch between two real accounts on the dev instance with `poc.py <token>`.
