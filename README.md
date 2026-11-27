# Remit

Remit is a single-owner console where AI agents discover, solve and review work as **chains**. Each chain is run by an
orchestrator agent, the **Chair**, which hires as many agents as the case needs. Every agent is a card (one Claude Code
session); edges show handoffs, feedback and questions; the owner chats with any card and reads what the chains
landed. Remit lands every approved change on its repository's base itself and releases it by the repository's own
`.remit/release/`, with no merge request and no release waiting on the owner. Every employee is an AI agent; people are mentors behind them: when an agent is stuck, its Chair brings a mentor onto that
card, and the mentor steers its session from their own board in the web app. The owner states what he wants in his own words; one that needs shaping becomes an objective of key results he applies,
and agents work toward them. Routines the owner sets up
find and start work on a schedule; nothing waits on the owner. The core is small and closed; a tool source, an event source, a
forge or a screen the core lacks is a plugin, and the result of all work is files in Git, landed on the base branch.

It serves one owner and one more team; nothing here is for outside customers yet.

| Read | For |
|---|---|
| [docs/design.md](docs/design.md) | the system as it is |
| [docs/architecture.md](docs/architecture.md) | the system as it is, in diagrams: processes, a chain's life, signals, the change path, packages |
| [docs/remit-design.md](docs/remit-design.md) | where it is going: principles, decisions, what is built and what is not |
| [docs/adr](docs/adr/README.md) | why each standing decision was made |
| [docs/deploy.md](docs/deploy.md) | rebuilding it on another machine |
| [AGENTS.md](AGENTS.md) | how to work on this repository |
| [docs/archive](docs/archive/README.md) | the evidence and review record behind the design: blueprint, research, council, papers |

## Run

```sh
make deps    # the web app's packages (web/); agents run on the `claude` CLI, signed in on this machine
make build   # the client core bound for the web app, web/dist embedded into bin/remit, and bin/remit-client
./bin/remit            # http://127.0.0.1:4320, data in ~/.remit
```

For development, `make dev` (`gmake dev` on macOS) stops the last run, rebuilds and starts the backend, and serves the web app
with hot reload at http://localhost:5173 (data in `~/.remit-dev`; Ctrl-C stops both).

### Signing in

The owner comes in with the owner key (`docs/adr/0022-a-local-server-lets-in-the-holder-of-the-owner-key.md`), or with
Google when the server is set up for it (below). A mentor comes in from their own device with a code instead (below).

```sh
./bin/remit   # prints: open http://127.0.0.1:4320/#key=…
```

The first start makes an owner key in `<data>/owner.key` (mode 0600; an agent session runs as your OS user and can read it, ADR-0138). Open the
link the server prints in its terminal: the browser keeps the key and drops it from the address, and comes in from then on.
Started by a supervisor (no terminal), the server logs where the key is instead of printing it; add `#key=` and the file's
content to the address. Deleting the file makes a new key at the next start, which shuts out every browser that held the old
one. `-addr` must be this machine's loopback: the key travels in plain HTTP. To reach it from your other devices, put a
TLS proxy of your private network in front and keep `-addr` as it is, for example Tailscale's (tailnet only, not Funnel):
`tailscale serve --bg 4320`, then open `https://<machine>.<tailnet>.ts.net/#key=<the key>` once on each device. Who may reach
that proxy is the operator's to secure.

The owner may also sign in with Google (`docs/adr/0105-the-owner-may-also-sign-in-with-google.md`), through a Firebase
project with Google enabled as its sign-in provider and every host you open Remit at (the Tailscale address above, for
example) among its authorized domains:

```sh
./bin/remit -firebase-project <project id> -firebase-api-key <web API key> -google-emails you@gmail.com
```

The sign-in page then offers **Sign in with Google**; the server checks Google's token itself and lets in only the accounts
`-google-emails` names, each for 14 days. The owner key keeps working beside it.

A mentor works from a code the owner has made for them (`docs/adr/0060-a-person-works-from-codes-the-owner-has-made-for-them.md`,
`docs/adr/0062-every-seat-is-an-ai-employee-and-people-mentor-them.md`): tell the Assistant to add them ("add Lan as a mentor,
strong in visual design") and make them a code ("make Lan a code for her phone"), open their row in Crew → Mentors, show the code
and pass it on, or copy its sign-in link there. The mentor opens the link, or pastes the code on Remit's sign-in page, and
works from their own board: the helps they are asked for, and each card's session as a chat they steer. A mentor holds as
many codes as they work from devices; each works until the Assistant revokes it on your word or removes the mentor.

### Taking your data with you

```sh
./bin/remit export -out ~/remit-export   # a new directory; -data names the data directory
```

It writes one JSON Lines file per table (a row an object, the documents as JSON) and a `manifest.json` with the schema version and row
counts, read as one moment of the database and beside a running server, which it never writes to. What is sealed under the server's key
(source credentials, event signal keys, client keys) is left out: the key stays with the server. The output directory must be new or empty.

### Handing a key to an outside agent

An agent outside Remit (an evaluator, a script) cannot sign in as the owner and must not. Tell the Assistant to give it a client key
("give the doctor a key; it may start `research`"): the Assistant makes it, with limits, and the **Client keys** page (under Integrations)
shows it with "Show key", Pause and Resume. The holder puts the key in a file only it reads (mode 600) and connects to
`/mcp/client` on this machine with a `headersHelper` that reads the file, never with the key in a prompt or an argument. Name
that file in `-client-key-files` so agents are asked before reading it. What a key may do, and what it never may, is in
`docs/adr/0009-client-keys.md`.

### Flags

Every flag has a default; `./bin/remit -h` prints them all. The table below is written from the same definitions (`make gen`).

<!-- flags: written by `make gen` from crates/remit/src/flags.rs -->
| Flag | Default | What |
|---|---|---|
| `-addr` | `127.0.0.1:4320` | listen address (TCP for HTTP, UDP for the link's WebTransport) |
| `-link-addr` |  | where the link's WebTransport (UDP) listens instead of -addr, for browsers on other machines: an address that also takes this machine's own, 0.0.0.0:<port> or [::]:<port>, as the AI runtime dials the link on loopback; HTTP, which carries the owner key, stays on -addr (empty: -addr) |
| `-data` | `~/.remit` | the database, its backups and the chains' working directories |
| `-agent` | `claude` | Claude Code command every session runs on, signed in with this machine's login (-claude-config-dir), in a config dir of its own (<data>/claude) |
| `-principles` |  | owner's principles file given to every agent (empty or absent: none) |
| `-top-model` | `opus` | the most capable model Remit runs: no model above its tier is offered, and every judge runs on it |
| `-chair-model` | `opus` | model for the Chair; the Chair chooses the model of every agent it hires |
| `-chair-effort` | `medium` | effort for the Chair (low, medium, high, xhigh, max; empty: Claude Code's default); the Chair chooses the effort of every agent it hires |
| `-assistant-model` | `opus` | initial model for the Assistant |
| `-assistant-effort` | `medium` | initial effort for the Assistant (low, medium, high, xhigh, max; empty: Claude Code's default) |
| `-mode` | `auto` | Claude Code permission mode for every agent |
| `-autonomous` | `true` | chains decide without the owner: the Chair asks him only to clear a block from outside the chain; Remit lands each approved change itself |
| `-glab` | `glab` | glab binary, logged in on this machine, that Remit reads GitLab hosts through (where a branch stands, the checks on a commit) |
| `-gh` | `gh` | gh binary, logged in on this machine, that Remit reads GitHub hosts through (where a branch stands, the checks on a commit) |
| `-forges` | `github.com=github,gitlab.com=gitlab` | the forge of each host a repository may be on, host=kind separated by commas, kind gitlab or github: a repository on a host not named here cannot be opened |
| `-vault` |  | a directory of the owner's files: <vault>/secret holds credential env files (empty: no vault) |
| `-event-chains-per-day` | `3` | chains the events of one event signal may start a day; past it its events wait for the next day (0: no limit) |
| `-turn-idle` | `30m0s` | how long an agent in a turn may send nothing (no message from Claude Code) before Remit cancels the turn, and closes its session if the turn has not ended a minute later; the turn fails, with a retry in the inbox (0: never) |
| `-turn-repeats` | `3` | how many times the same tool call with the same result, or the same error, among a turn's last 8 calls (numbers, times and ids masked) makes Remit steer a note into the turn; the second time it tells the agent to end the turn with its report, and ends the turn if the calls go on (0: never; docs/adr/0092) |
| `-spin-turns` | `10` | how many turns a running chain's agents may take with nothing produced (no plan step completed, no work accepted, no verdict) before Remit reminds its Chair, and again every as many turns, finishing the chain after 3 reminders; for the Assistant, turns in a row on agents' replies with no word from the owner before its messages to agents carry a reminder, refused after 3 (0: never; docs/adr/0104) |
| `-language` | `English` | language of everything the owner reads (messages, why, labels, summaries) |
| `-claude-config-dir` | `$CLAUDE_CONFIG_DIR` | Claude Code config dir whose login the agents use (on Linux; on macOS the agents sign in once for <data>/claude, as the start says), and whose skills and agents the library imports from; empty uses ~/.claude |
| `-secret-key-file` |  | the key the sources' credentials and the event signals' keys are sealed with, created (mode 0600) when the first one is sealed, and backed up in -key-backup-dir before anything is sealed under it (default <data>/secret.key) |
| `-key-backup-dir` |  | where the secret key is backed up, one owner-only file per key named by its id; a directory named secret keeps it under the agents' secret-read rules, and one on another disk survives losing the data directory (default <data>/backups/secret) |
| `-client-key-files` |  | comma-separated absolute paths of the files where the holders of client keys keep theirs, which no agent session can read, as for -secret-key-file (ADR-0018) |
| `-secret-dirs` |  | comma-separated absolute paths of directories that hold the owner's credentials and that no agent session may read (a vault's secret directory is hidden already) |
| `-keep-worktrees` | `168h0m0s` | how long a finished chain with no landing on its way keeps its worktrees before they are removed (its branches stay; reopening the chain brings them back) (0: for ever) |
| `-idle-alert` | `2h0m0s` | how long nothing may run while work waits (queued chains, changes waiting to land or questions for the owner) before the owner's inbox says so, once (0: never) |
| `-snapshot-every` | `24h0m0s` | how often the database is copied, whole and consistent, into <data>/backups; the newest five copies are kept (0: never) |
| `-jira-env` |  | env file with ATLASSIAN_HOST, ATLASSIAN_EMAIL, ATLASSIAN_TOKEN the issues capability's list_issues reads Jira with (default <vault>/secret/atlassian/jira.env) |
| `-jira-jql` |  | the JQL of the Jira issues list_issues shows (without it Jira is not read) |
| `-source-env` |  | env file with the credentials of the sources agents read the outside world through (MONGODB_URI, STARROCKS_HOST, BYTEBASE_URL, ...; which variables each source needs: crates/source, ADR-0015). The default connection of each source reads the values the owner entered in his inbox first, then the process environment, this file and the vault's own env files (devops/grafana.env, devops/coroot.env, devops/datadog.env, services/intercom.env, services/amplitude.env); a named connection reads only the values entered for it (ADR-0017). Agents are asked before they read it (default <data>/sources.env) |
| `-openrouter-key-file` |  | file holding the OpenRouter key memory is searched by meaning and judged for relevance with (qwen3-embedding-8b, qwen3-reranker-8b), read at each use; with no key memory is searched by words and links (default <data>/openrouter.key) |
| `-datadog-env` |  | the env every record sent to Datadog is tagged with: the server's logs and spans and the agents' telemetry, each naming its chain, sent to the site of the Datadog source's default connection once the owner enters its key (ADR-0119) (empty: this machine's host name) |
| `-update-repo` |  | the GitHub repository this server runs the releases of, owner/name: a release of it that its procedure published is installed here as its last step, and taken back here when it breaches (ADR-0116) (empty: this server does not run from releases, as a dev instance) |
| `-update-restart` |  | the command, run by sh -c, with which this server's supervisor restarts it on <data>/releases/current, as remit update -restart takes it: set, a release of -update-repo is installed here (empty: nothing restarts this server; needs -update-repo) |
| `-firebase-project` |  | the Firebase project id the owner signs in with Google through, beside the owner key (ADR-0105); set with -firebase-api-key and -google-emails (empty: Google sign-in is off) |
| `-firebase-api-key` |  | the web API key of -firebase-project, which the way in sets Firebase up with (not a secret: Firebase shows it to every page that signs in) |
| `-firebase-auth-domain` |  | the domain Firebase's Google sign-in window runs on (empty: <firebase-project>.firebaseapp.com) |
| `-google-emails` |  | the Google accounts that come in as the owner, comma-separated; any other account is refused (set with -firebase-project) |
<!-- /flags -->

The review cap comes from `docs/adr/0001-admission-by-review-capacity.md`.

### Event signals

Everything that comes in from outside is an event of an event signal, `event:<name>` (`docs/adr/0024-everything-that-comes-in-is-an-event-signal.md`),
whichever way it came: an outside system (GitLab, GitHub, Sentry, Jira) posting to the signal's door at `/hooks/<name>`, a script
through `remit emit`, a sensor reading a source (GitHub issues, Datadog monitors and the numbers it computes), or a Chair whose chain
holds the `events` capability. Tell the Assistant to open one ("open a way in for the billing group's GitLab", "make a signal of the
bug issues of octo/app"). Each signal is on the Signals tab, whose page shows its settings, its door's address and, on
**Show key**, its door's key (the Assistant never sees it); its rules are a subscription on the Subscriptions tab, like any other.

A post to the door is let in only with that signal's key: GitLab's secret token (`X-Gitlab-Token`), GitHub's signature of the
body with it (`X-Hub-Signature-256`), `Authorization: Bearer <key>`, or `?token=<key>` in the URL for a sender that cannot set
headers; anything else gets 401. For a GitLab project: Settings → Webhooks → URL the signal's door, Secret token its key, and
tick pipeline events, which the rules that ship read. For a GitHub repository: Settings →
Webhooks → Add webhook → Payload URL the signal's door, content type `application/json`, Secret its key, and the event
workflow runs. Tick more events only for subscriptions of your own. The page has a guide for both. Keys are sealed
under `-secret-key-file` (default `<data>/secret.key`); the door must be reachable from the sender, and the web server in front
of Remit forwards `/hooks` to it (`web/vite.config.ts`).

A script writes an event with `remit emit`, a client of the same door:

```
remit emit -name <event signal> [-url <server>] [-key-file <path>] [-sender-id <id>] [file | -]
```

It posts the file, or stdin, to `<server>/hooks/<name>` (the server defaults to `http://127.0.0.1:4320`) and prints the server's
answer, `{"event":"pl_…"}`. The key is read from a file of mode 600 (one others can read is refused) or from
`REMIT_EVENT_KEY`, never from an argument, which every user of the machine can list. The same sender id posted again is the
same event; with none, the same body within ten minutes is.

A sensor reads a source on a period for a signal, through the connection of a source the owner named (the Tools page
lists them), as a read. Each new or changed item (an issue, a monitor that is not OK) is an event of the signal; the first read emits nothing
older than the sensor. A sensor that someone other than the owner makes or points at a new source, connection or query waits for his
Apply in his inbox. A source that fails three reads in a row is silent, and the signal's line on the Signals tab says so.

What is done about an event is a subscription to its signal: a condition in CEL over the event (`when`) and one action, which tells the
Chair of the chain the event is about, starts a chain from it, or runs a job on it. The `gitlab` and `github` signals ship with
the subscriptions that have Remit read the checks on what a chain landed when a pipeline or workflow run on it finishes, and
tell the chain's Chair when one failed. An event no subscription holds for is stored and does nothing (`-event-chains-per-day`, default 3, is
the chains one subscription may start a day; see `docs/design.md`, Event signals). Open an event to read what came in and what
each subscription did about it. To have something done about what comes in ("start a chain when a Sentry issue is fatal in
billing"), tell the Assistant: it makes the subscription, which shows on the Subscriptions tab and under "Subscribed by" on the
signal's page.

### Models, and what happens when the provider fails

Every agent runs on Claude Code, signed in with the machine's own Claude Code login (`-claude-config-dir`, default
`$CLAUDE_CONFIG_DIR`, else `~/.claude`): no API key is set. The sessions run in the AI runtime, `remit runtime`: the same
binary, started by the server as its child, which dials it back over `/runtime` and holds every Claude Code process, the
accounts' logins and the transcripts (`docs/adr/0058-claude-sessions-run-in-an-ai-runtime-outside-the-core.md`). Agents run in a config dir of their own, `<data>/claude`, that holds
a link to that login and nothing else, so the plugins and skills installed for the machine's user never reach them. A card's model is `fable`, `opus`, `sonnet` or `haiku`
(apex, frontier, workhorse and fast tiers), run at an effort of `low`, `medium`, `high`, `xhigh` or `max` (empty: Claude
Code's default). The Chair picks the pair per hire (an agent's default, else its own), and `-chair-model`/`-chair-effort` set the Chair's choices (default `opus` at `medium`).
`-assistant-model`/`-assistant-effort` set the Assistant's initial choices. The owner can change them in the Assistant's composer between turns. `-top-model` (default `opus`) is the most capable model
any card runs on, and every judge runs on it
(`docs/adr/0133-the-owner-caps-the-model-and-agents-think-at-medium.md`). When the provider fails (the account is signed out, a rate or usage limit is hit, the service is down), the card
waits for it, its message kept, and goes on by itself once the provider answers; the Health bar says why, and the card's
own badge reads `Waiting for a provider` (`docs/adr/0006-claude-code-is-the-only-agent.md`).

More Claude accounts: press Add account on the Accounts page (Integrations). The server runs `claude auth login` and shows the
sign-in link on the page and in the inbox; open it, sign in to the account to add, and type the code the page gives into the item
if the browser cannot reach the server. The account is kept under the address it signed in as. A card whose account is out moves
to another usable account and keeps its session; it waits only when none is usable
(`docs/adr/0040-agents-run-on-a-pool-of-claude-accounts.md`).

On macOS Claude Code keeps a login in the Keychain under the name of its config dir, which no file can link, so the default
account is signed in for the agents' own: `CLAUDE_CONFIG_DIR=~/.remit/claude claude auth login`. The server starts without
it: the default account is then signed out, the Health bar says so, and agents run on the other accounts.

## Layout

The server is a Rust workspace of small crates (`crates/`, ADR-0072); each crate's `README.md` says what it holds.

| Path | What |
|---|---|
| `crates/remit` | The `remit` binary: wires store, orchestrator, the link, the MCP endpoints and the embedded web app; also the subcommands `emit`, `export` and `runtime` (the AI runtime) |
| `crates/orch` | Chains, cards, the agents' MCP tools (`hire_agent`, `route`, `ask_owner`, …), the rails, the scheduler, the event signals; its `README.md` maps its modules |
| `crates/model` | The records Remit stores and sends to its channels |
| `crates/store` | SQLite (one file): numbered migrations and the reads and writes |
| `crates/workflow` | The pure engine of the workflows a Chair writes as a script |
| `crates/signal` | Signals and subscriptions: the pure evaluation behind every wake, start and firing |
| `crates/source` | The tools through which agents read and act on outside systems, behind one gate |
| `crates/grant` | The pure decision of what a chain may do on a connection, and the effect ladder |
| `crates/egress` | The egress gate and the injecting proxy, on a loopback port, through which a program reaches a connection |
| `crates/plugin`, `crates/plugin-sdk` | The runner of the plugins the owner applied, and what a plugin is written with |
| `crates/bundled` | The plugins that ship in the binary (GitHub, GitLab, Intercom, Amplitude, Google Analytics, script, forge), run in the server's own process |
| `crates/forge` | One adapter per forge (GitLab, GitHub), driven through its own command line |
| `crates/clientmcp` | The endpoint a client key opens, on this machine only |
| `crates/auth` | Who comes in: the owner key, and the owner's Google sign-in (the Firebase ID token check, the credential it mints) |
| `crates/secrets` | Seals source credentials and event signals' door keys under the key file |
| `crates/door` | The door outside systems and `remit emit` post to, `/hooks/<name>`: a channel adapter like the browser's, one of the event signals' writers |
| `crates/library` | What an agent session is given, and whether a definition may be stored (pure) |
| `crates/rollout` | The rule a merge passes to reach users in stages: bake, guardrails, the way back (pure) |
| `crates/replay` | Which recorded goals a change's candidate is replayed against, and how one is judged (pure) |
| `crates/outcomes` | How well the agents do: the KPIs and the Outcomes report (pure) |
| `crates/agent` | The session port (`Launcher`) and its Claude Code adapter: one Claude Code session per card, told in Remit's terms |
| `crates/claudecode`, `crates/claudehooks` | One `claude` process over its SDK protocol, and the typed format of Claude Code hooks |
| `crates/airuntime` | The AI runtime (`remit runtime`): holds every Claude Code session, the accounts' logins and the transcripts, and dials the core on the link |
| `crates/runtimelink` | The core's end of the runtime: starts and supervises it, and implements the session port over the link |
| `crates/openrouter` | OpenRouter's embeddings and rerank APIs, which memory is searched and judged with |
| `crates/datadog` | Everything Remit runs sent to Datadog's OTLP intake: the server's logs and spans, the agents' telemetry stamped with its chain and put under its turn (ADR-0119) |
| `crates/builtin` | The skills that ship in the binary, the Assistant's manual among them |
| `crates/importer` | Reads the agents and skills of this machine's Claude Code configuration into the library |
| `crates/skills` | What a skill is: the `SKILL.md` format, the rules it must meet, the plugin directory skills are rendered into |
| `crates/childenv` | What a child process an agent can steer inherits of the server's environment |
| `crates/procgroup` | Runs commands as process groups, so ending one ends all it started; `Program` is the one runner every child but a session starts through |
| `crates/provider` | The catalog of models agents run on, each with its tier |
| `crates/link`, `crates/link-quic` | The protocol every Remit peer speaks, and its driver over WebTransport |
| `crates/wire` | The one definition of what crosses the link: the contracts `app`, `runtime` and `plugin` |
| `crates/rpc` | The web app's adapter onto the orchestrator, for the owner and, scoped to their helps, a mentor |
| `crates/replica`, `crates/client-web`, `crates/remit-client` | The client cores (one link session and the replica of what it pushes), bound for the browser with wasm-bindgen, and the native client binary |
| `crates/tsgen` | Writes the TypeScript of the contracts for the web app (`make gen`) |
| `web/` | Vue 3 app: it renders what the client core holds; `web/src/core/generated` is the contract's TypeScript |
| `scripts/` | `make dev`, `make package`, the release's tag, the git hooks, the change's tests and the commit-size check, with their tests |
| `deploy/` | The systemd unit and the launchd job (macOS) to run Remit with |
| `docs/` | `design.md`, the system as it is; `architecture.md`, its diagrams; `remit-design.md`, the direction; `adr/`, why its standing decisions were made; `deploy.md`; `archive/`, the evidence and review record |

## The link

Every peer speaks one protocol, the link (`crates/link`), over WebTransport at `/link` (`/link.json` says where it listens
and the certificate's hash). A session's `hello` names its contract, all of them defined once in `crates/wire`:

| Contract | Between |
|---|---|
| `wire::app` | the web app (the browser, through the client core) and the server, for the owner or a mentor |
| `wire::runtime` | the server and its AI runtime |
| `wire::plugin` | the server and a plugin process |

After editing a contract run `make gen` and commit the result: it writes the TypeScript the web app
compiles against; `make check-gen` fails when a generated file differs from its contract. A new call is then a compile error
on the server until its adapter (`crates/rpc`) answers it.

## Test, release and health

Tests come in two tiers. `make test-changed FILES="<files>"` runs on every change and tests what the named files can break
(`scripts/test_changed.sh`: the tests of the crates that hold them, clippy over those and the crates that depend on them, the
related web tests, the contract check, the script tests) and says which file it could not map. `make test`
is the whole of it, which CI runs: the contract check, the workspace formatted, clippy with every workspace lint and every
Rust test, the web typecheck and tests, and the tests of the scripts.

What runs is a GitHub release. Every push to `master` is one: `.github/workflows/release.yml` builds and tests the commit and
only then publishes it, tagged `<yyyymmdd>-<short sha>` (`scripts/release_tag.sh`), with the linux-amd64 server and client
and their `SHA256SUMS`. On the machine that runs it,
`remit update -repo <owner/name> -restart '<the supervisor's restart>'` puts the newest release live: it installs it as
`<data>/releases/<tag>/remit`, points `current` at it and `previous` at the release before, copies the database, restarts
the server and waits until it is healthy on the release's commit, going back to `previous` when it is not, with the
database's copy put back when the release migrated it; its outcome is also left in `<data>/releases/update.json`
(`docs/deploy.md`, *Install a release*). `remit doctor` checks a running server and changes nothing. Its agents run on the `claude` on the supervisor's PATH
(`-agent`).

`GET /healthz` (no sign-in) answers 200 while the database answers, the scheduler has ticked within 3 minutes and the AI runtime is connected, 503 with
the reason otherwise, and says the commit the binary was built from.

`make hooks` runs `scripts/hooks/pre-commit` on every commit of this clone, which runs clippy over the crates the commit touches
and the crates that depend on them, and `scripts/hooks/pre-push` on every push, which stops a push that does not build or pass `make audit` and
warns about a commit past the size budget.

## License

Remit is published for reading. Rights in original Remit material are reserved. Third-party material retains its own licenses. See [LICENSE](LICENSE).
