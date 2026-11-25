# 0071. A plugin provides Claude accounts, one for each of its connections

Status: accepted, 2026-11-10; its allowlist takes `CLAUDE_CODE_OAUTH_TOKEN` by ADR-0129 (2026-11-23). Extends ADR-0040 (the pool of accounts; its *Revisit when* "an account backed by an API key, Bedrock
or Vertex"); adds one method to ADR-0034's plugin protocol and keeps ADR-0058's split (the runtime holds the config dirs). Source: the
owner's words of 2026-11-10, "cho phép plugin cung cấp các account" (let plugins provide accounts), and his decision that a plugin
account supplies the session's environment (an API key, Bedrock or Vertex variables, or a gateway's address and token) from an
allowlist of Claude Code's variables, and never a Claude.ai subscription's OAuth token. Read against the working tree of 2026-11-10.

## Context

The customer is the owner: his agents stop when every account he signed in is at its limit (ADR-0040), and the capacity he can add
is one more Claude.ai subscription, signed in by hand through Anthropic's flow. Capacity he already pays for elsewhere (an Anthropic
API key, Bedrock, Vertex, a company LLM gateway) cannot run an agent.

What the code does today:

- **An account is a signed-in config dir.** `model.Account{ID, Email, CreatedAt}` (`internal/model/account.go:13-17`), one row of
  `accounts` (`internal/store/migrations_accounts.go`, migrations 63-64; `internal/store/accounts.go`). The owner adds one with Add
  account (`AddAccount`, id `newID("a_")`, `internal/orch/account_signin.go:51`) and removes one with Remove (`removeAccount`,
  `internal/orch/accounts.go:200`), both rails.
- **One place chooses, one breaker judges.** `chooseAccount`, `accountFor`, `bindAccount` (`internal/orch/accounts.go:114-160`);
  `outages` per account (`internal/orch/health.go:37`); a probe per account runs a real session (`probe`,
  `internal/orch/health.go:349`, `launcher.Start` at `:370`). A card whose account is gone gets another (`chooseAccount`: its own
  only while it is listed).
- **Three places open a session**, each naming only the account id: a card (`internal/orch/access.go:103`), the intake
  (`internal/orch/intake.go:350`), the probe (`internal/orch/health.go:370`). The runtime's `SessionSpec` carries `account`
  (`web/src/rpc/runtime/define.ts:55`, `internal/runtimelink/convert.go:24`); `ClaudeLauncher.accountEnv` turns it into
  `CLAUDE_CONFIG_DIR` (`internal/agent/claude.go:85-90`, used at `:106`). The runtime holds the dirs (`agent.ClaudeAccounts.Prepare`,
  `internal/agent/accounts.go:38`; `orch.AccountDirs`, `internal/orch/accounts.go:22-35`).
- **A session inherits no provider credential.** `childenv.Of` passes a closed list of names, and its test proves
  `ANTHROPIC_API_KEY`, `CLAUDE_CODE_OAUTH_TOKEN` and `AWS_*` secrets do not reach a child (`internal/childenv/childenv.go:15-33`,
  `childenv_test.go:22-26`).
- **A plugin's credential is a connection's variables.** The owner types each into a write-only inbox field (`SetCredential`,
  `internal/orch/credentials.go:178`), sealed; a plugin receives them per call (`CallRequest.vars`, `PollRequest.vars`,
  `SenseRequest.vars`, `ProcessRequest.vars` in `sdk/pluginpb/plugin.proto`), a sense and a process through `source.Set.Lend`, which
  passes the one gate as a named caller (`internal/source/lend.go:32`; `sensorAgent`, `internal/orch/sensor.go:39`). A change of
  extensions goes through `loadExtensions` (`internal/orch/extensions.go:532`; called at start, on Apply, removal, enable and
  disable), a change of a connection's values through `sourceChanged` (`internal/orch/credentials.go:306`).
- **Claude Code reads the variables.** The installed `claude` 2.1.322 contains `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`,
  `ANTHROPIC_BASE_URL`, `ANTHROPIC_CUSTOM_HEADERS`, `CLAUDE_CODE_USE_BEDROCK`, `AWS_BEARER_TOKEN_BEDROCK`,
  `ANTHROPIC_BEDROCK_BASE_URL`, `CLAUDE_CODE_SKIP_BEDROCK_AUTH`, `CLAUDE_CODE_USE_VERTEX`, `CLOUD_ML_REGION`,
  `ANTHROPIC_VERTEX_PROJECT_ID`, `ANTHROPIC_VERTEX_BASE_URL`, `CLAUDE_CODE_SKIP_VERTEX_AUTH`, `ANTHROPIC_DEFAULT_{FABLE,OPUS,SONNET,HAIKU}_MODEL`,
  and `CLAUDE_CODE_SUBPROCESS_ENV_SCRUB` next to a list of the credential names above (`grep -a` over the binary). **Checked on this
  machine, 2026-11-10:** `claude -p` with `ANTHROPIC_API_KEY` set in an empty config dir calls the API with that key, no prompt
  (a made-up key answers `401`, "Invalid API key · Fix external API key"); the same dir with no key says "Not logged in". A session
  started on the machine's login with `ANTHROPIC_CUSTOM_HEADERS` and `AWS_SESSION_TOKEN` set shows both to `env` run from its Bash
  tool, and shows neither with `CLAUDE_CODE_SUBPROCESS_ENV_SCRUB=1`, its Bash still running; the same holds under `sandbox-exec`
  with a profile shaped as the jail's macOS `hiddenProfile` (`internal/jail/sandbox_darwin.go:164-175`), the hidden path still
  refused. The Linux jail (namespaces) is not run here. With the scrub, `ps -E` from that Bash
  still reads them in its parent processes' environment (macOS, same OS user).

## Options

**(a) What a plugin account is to a session.**

1. **Variables only, on the default config dir.** Smallest, but the default dir holds the machine's subscription login: a session
   whose variables fail or are ignored runs on that login, and its failure or its use is charged to the wrong account. Rejected.
2. **A config dir of its own that no one signs in, plus the plugin's variables (chosen).** Prepared as any account's (`projects/`
   linked to the default's, so a card moves across all accounts and resumes); with no login in it, nothing but the variables can
   authenticate, so a failure is the plugin account's.
3. **The plugin writes the dir** (a `settings.json` with `env` or an `apiKeyHelper`). The plugin runs beside the core, the dirs live
   in the runtime, and the secret would rest on disk. Rejected.

**(b) Where the secret travels.**

1. **The core asks the plugin at each session open and passes the variables to the runtime in the session's spec (chosen).** The
   secret is never stored by Remit outside the connection's sealed variables; a rotated key or a minted short-lived token reaches the
   next session with no refresh path of our own.
2. **The runtime asks the plugin.** Plugins are hosted by the core (`internal/plugin/host.go`); a second host or a lent service in
   the runtime is a second path to the same answer. Rejected.
3. **The core stores the variables on the account.** A copy of the connection's sealed values, stale on rotation: two owners of
   one fact. Rejected.
4. **The secret never leaves the core: sessions reach the provider through Remit's injecting proxy**, which adds the credential
   (ADR-0018 form T1; `CLAUDE_CODE_SKIP_*_AUTH` and the base-URL variables exist for gateways). The strongest control, but every
   model token streams through the core and the proxy must learn streaming and sigv4 for Bedrock. Deferred: it is the hardening
   (*Revisit when*), and (b1) leaves its seam.

**(c) What the plugin declares.**

1. **A plugin answers `Account` with the variables for a connection (chosen).** It may hand back the key as typed, or exchange it
   (STS, a gateway's token) per session.
2. **A declarative map in `Describe`** (variable name → connection variable), no plugin call at session start. Smaller, but it
   cannot mint a short-lived credential, the direction providers move in. Rejected: the RPC covers it and more.

**(d) How many accounts, and who owns the fact.**

1. **One account for each configured connection of an enabled plugin that provides accounts, derived, not stored (chosen).** The
   owner already names connections and types their credentials; a second Bedrock region or a second key is a second connection. The
   connection is the one owner of the fact; the pool lists it.
2. **Rows in `accounts` with a `plugin` field, reconciled with the connections.** A copy that two paths keep in step (sign-in and
   credential change), with a migration and a sweep for rows whose connection went. Rejected.
3. **The plugin lists its own accounts** (a pool behind one gateway). The gateway already pools; a list the plugin changes at will
   would need a refresh signal of its own. Rejected for now (*Revisit when*).

## Decision

Options (a2), (b1), (c1), (d1).

**The seam in a plugin.** `sdk.Plugin` gains `Account func(ctx context.Context, call Call) (map[string]string, error)`: given the
connection's variables (`call.Vars`), the environment a session runs with. `plugin.proto`: `DescribeResponse.accounts` (`bool`,
field 13: the plugin provides accounts, set when `Account` is not nil) and `rpc Account(AccountRequest) returns (AccountResponse)`
with `AccountRequest { map<string,string> vars = 1; }` and `AccountResponse { map<string,string> env = 1; }`. `plugin.Host.Account`
calls it as `Call` is called. A plugin is applied as today (ADR-0034): `manifestOf` copies `accounts` into
`source.Extension.Accounts`, and `extensionRail` adds a field to the Apply item saying that the plugin will provide Claude accounts,
one for each connection, and that agents on them send their work wherever its variables point.

**The account.** `model.Account` gains `Plugin` and `Connection` (`omitempty`). A plugin account's id is `<plugin>.<connection>`
(plugin ids and connection names are lower-case letters, digits and dashes, `internal/orch/credentials.go:160`, so the dot is
unambiguous and never meets `default` or `a_…`); `Label()` is `<plugin> · <connection>`. `Kind()` is `machine` (the default),
`login` or `plugin`. Plugin accounts are never stored: `store.PutAccount` refuses an account with `Plugin` set
(`store.ErrPluginAccount`), so the state cannot be written.

**The pool.** `o.accounts` is the stored accounts, oldest first, then the plugin accounts by id. `usePluginAccounts(ctx)` derives
the plugin accounts from the enabled extensions with `Accounts` and their configured connections (`source.Set.Status`, `internal/source/connection.go:349`, a connection with no `Missing` variable), and is
called at the end of `loadExtensions` and `sourceChanged`. For an account that appears it prepares its dir (`AccountDirs.Prepare`)
and marks it out until a probe answers (as `keepAccount` does), then probes it; for one that goes it forgets its outages and
discards its dir (`AccountDirs.Discard`, whose `claude auth logout` finds no login). A card on an account that went moves at its next
session (`chooseAccount`, unchanged). `chooseAccount`, `accountFor`, `bindAccount`, `outages`, failover and the probe are unchanged:
a plugin account is an account.

**The session.** Orch opens every session through one method, `openSession(ctx, spec, h)`, which the card, the intake and the
probe call in place of `o.launcher.Start`. For a plugin account it lends the connection through the gate as the caller
`accountAgent` (`source.AccountCall`, effect read), calls the plugin's `Account`, and parses the answer with
`agent.ParseAccountEnv`; any failure (plugin down, connection missing a variable, a name outside the list) is an `agent.Failure` of
category `access` charged to that account, so the breaker takes it out and cards move. `agent.Spec.AccountEnv` carries the parsed
variables; `SessionSpec.accountEnv` carries them to the runtime, which parses them again with the same function at its boundary
(`airuntime.agentSpec`). `ClaudeLauncher.accountEnv` adds them after `CLAUDE_CONFIG_DIR`, with `CLAUDE_CODE_SUBPROCESS_ENV_SCRUB=1`
so that the tools a session runs do not inherit them.

**The allowlist** is `agent.ParseAccountEnv`'s, one closed list: `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`,
`ANTHROPIC_CUSTOM_HEADERS`, `ANTHROPIC_DEFAULT_FABLE_MODEL`, `ANTHROPIC_DEFAULT_OPUS_MODEL`, `ANTHROPIC_DEFAULT_SONNET_MODEL`,
`ANTHROPIC_DEFAULT_HAIKU_MODEL`; Bedrock: `CLAUDE_CODE_USE_BEDROCK`, `AWS_REGION`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`,
`AWS_SESSION_TOKEN`, `AWS_BEARER_TOKEN_BEDROCK`, `ANTHROPIC_BEDROCK_BASE_URL`, `CLAUDE_CODE_SKIP_BEDROCK_AUTH`; Vertex:
`CLAUDE_CODE_USE_VERTEX`, `CLOUD_ML_REGION`, `ANTHROPIC_VERTEX_PROJECT_ID`, `ANTHROPIC_VERTEX_BASE_URL`,
`CLAUDE_CODE_SKIP_VERTEX_AUTH`. Any other name, `CLAUDE_CODE_OAUTH_TOKEN` and `CLAUDE_CONFIG_DIR` among them, refuses the whole
answer, so a subscription's token cannot ride in (ADR-0040's terms line) and a plugin cannot move a session's dir, `PATH` or
`NODE_OPTIONS`. An empty answer is refused too: it would run on nothing.

**The secret's path.** Sealed connection variables → the gate → the plugin's `Account` → `agent.Spec.AccountEnv` in memory →
`/runtime` (local, keyed, ADR-0058) → the Claude Code process's environment. It is in no record: not the card, not `accounts`, not an
event, not `AccountHealth`, not a log line, not a stream entry; no agent tool returns it.

**What the owner sees and does.** `AccountHealth` (`web/src/rpc/define.ts:802`) gains `kind` (`machine | login | plugin`), `plugin`
and `connection`. The Accounts page lists a plugin account with the others, by its label, with its health and the plugin and
connection it comes from; it offers Remove only for `kind === 'login'`, which replaces the page's own `id !== DEFAULT_ACCOUNT` rule
(`AccountsView.vue:54`) with the server's. `RemoveAccount` on a plugin account is refused: "this account is the connection
<connection> of the plugin <plugin>: remove the connection's credential, or disable the plugin". He adds one by applying the plugin and
typing its connection's credential, as for any plugin; the Assistant's `list_accounts` reads the same health. No agent gains a tool.

**Stored data: no migration.** Nothing new is stored: plugin accounts are derived, a card stores an account id as before, and a
stored `Extension` manifest without `accounts` reads false (a plugin applied before this change provides none until applied again).
The highest migration stays 96 (`internal/store/migrations_skills.go:7`).

## Threats

| Threat | Control now |
|---|---|
| A plugin hands a Claude.ai subscription token (terms, ADR-0040) | The allowlist refuses `CLAUDE_CODE_OAUTH_TOKEN` and every unlisted name, in the core and again in the runtime |
| A plugin points sessions at its own server (`*_BASE_URL`) and reads every prompt and file an agent sends (information disclosure) | The Apply item says so before the owner applies it; a plugin already runs as the server's OS user (ADR-0034) |
| A plugin sets variables that change what a session runs (`PATH`, `NODE_OPTIONS`, `CLAUDE_CONFIG_DIR`) (elevation) | Closed allowlist, whole answer refused |
| An agent reads the key from its environment (disclosure) | `CLAUDE_CODE_SUBPROCESS_ENV_SCRUB=1` (checked: `env` in a session's Bash no longer shows them); residual, checked: `ps -E` from that Bash reads its parent process's environment as the same OS user, as it can read the other accounts' config dirs today (ADR-0040 *Consequences*) |
| The key reaches a record, a log or a channel | It lives only in `agent.Spec` and the runtime's spec; a test asserts the store, the events and the logs of an opened session hold no part of it |
| A failing plugin silently runs the session on a subscription | The account's dir has no login, so nothing else authenticates; the failure is the plugin account's |

Threat modelling (`aio-threat-models`) runs when the owner asks for hardening.

## Consequences

- Capacity the owner holds outside Claude.ai subscriptions runs agents, chosen and failed over exactly as a login is.
- A plugin account is used as much as any account: `chooseAccount` spreads by load, so metered capacity is spent as readily as a
  subscription's (ADR-0010: Remit records no cost and caps none).
- A session open on a plugin account costs one plugin call; a plugin that is down takes only its accounts out.
- A card moves between a subscription and a plugin account and resumes through the shared transcripts; whether a Bedrock or a
  gateway session resumes a subscription's transcript cleanly is not yet observed (ADR-0040's last *Revisit when* covers it).
- `childenv`'s comment "no API key or token is set (ADR-0006)" becomes "only a plugin account's, which the launcher sets".

## Tests (written first, main path first)

- `TestAnAccountEnvOutsideTheListIsRefusedWhole` (`internal/agent`): every listed name passes; `CLAUDE_CODE_OAUTH_TOKEN`, `PATH`,
  `CLAUDE_CONFIG_DIR` and an empty answer are refused.
- `TestASessionOnAPluginAccountRunsWithItsVariablesInItsOwnDir` (`internal/agent`): the process's environment holds the variables,
  `CLAUDE_CONFIG_DIR` of the account's own dir and the scrub flag; a login account's session holds none of them.
- `TestEachConfiguredConnectionOfAPluginIsAnAccount` (`internal/orch`): applying a plugin with `accounts` and setting a connection's
  credential adds `<plugin>.<connection>` out until probed; removing the credential or disabling the plugin takes it away and a card on
  it moves at its next session.
- `TestACardOnAPluginAccountOpensWithWhatThePluginGave` (`internal/orch`): the spec reaching the launcher carries the plugin's answer;
  a plugin that fails charges the account and the card moves.
- `TestAPluginAccountIsInNoRecord` (`internal/orch`): after a session opens, no store row, event or log line contains the key.
- `TestAPluginAccountCannotBeRemovedOrStored` (`internal/orch`, `internal/store`): `RemoveAccount` refused with the words above;
  `PutAccount` returns `ErrPluginAccount`.
- `TestAPluginDescribesAndAnswersAccount` (`internal/plugin`, with `testplugin`): `Describe` says `accounts`, `Account` returns the
  environment for the variables given.
- `AccountsView.test.ts`: a plugin account shows its plugin and connection and no Remove; a login shows Remove; the default none.

## Revisit when

- **The key must not be in the session at all**: base-URL the session to Remit's injecting proxy (ADR-0018) with a run token and
  `CLAUDE_CODE_SKIP_*_AUTH`, the proxy adding the credential. The seam is `openSession`, the one place the variables are made.
- **Only some plugins may provide accounts**: a manifest capability that lists the base-URL hosts a plugin's accounts may reach,
  checked by `ParseAccountEnv`'s caller, or accounts lent only to bundled plugins, as ADR-0057 lends `Forge`. The seam is
  `DescribeResponse.accounts` and the Apply field.
- **A plugin fronts a pool it changes at will**: a `ListAccounts` method and a refresh signal, in place of one account per connection.
- **A credential expires inside a long session**: `apiKeyHelper` in the account's dir calling back to the core
  (`CLAUDE_CODE_API_KEY_HELPER_TTL_MS`), so a running session refreshes rather than failing at the provider.
- **Vertex with a service account's key file**: the variables cannot carry a file; a gateway (`ANTHROPIC_VERTEX_BASE_URL` with
  `CLAUDE_CODE_SKIP_VERTEX_AUTH`) is the path until a file can be lent.
- **Metered and subscription capacity should not be spent alike**: an owner-set order or weight per account (ADR-0040's weight).
