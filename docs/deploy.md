# Deploying Remit (the codebase Remit evolves from) on another machine

How to rebuild Remit from a package (`make package`, `scripts/package.sh`) and bring it up. What each flag means is in
`README.md`; how the system works is in `docs/design.md`. This guide holds what neither says: what to carry, what the
machine must have, what lives outside the repository, and how to move the owner's data.

## What the package holds

`make package` writes into `bin/package`:

| File | What |
|---|---|
| `remit-<sha>[-dirty].tar.gz` | The source tree as it was when packed: tracked files plus untracked ones git does not ignore, so uncommitted work travels. Holds the docs, `deploy/`, `scripts/` and a `PACKAGE` manifest (the commit it is based on, how many paths differ from it). No database, key or environment file, no binary. |
| `remit-<sha>.bundle` | The git history: HEAD, every branch and tag. Without it the tree builds, but its binary reports revision `unknown` on `/healthz`, and `git log` / `git blame` (which the working agreements rely on) have nothing to read. |
| `SHA256SUMS` | Checksums of the files above. Check them after copying (`sha256sum -c SHA256SUMS`). |
| `remit-state-<time>.tar.gz` | Only with `make package ARGS=--with-state`. The owner's data and a **secret**: a consistent copy of the database and the key its source credentials and event signals' door keys are sealed with. Mode 0600. Carry it over a channel you trust and delete it after restoring. |

The database holds the chains, their history, the playbooks, agents, skills, the connections of the sources and their grants,
the extensions, event signals and their events, objectives, employees and their memory, client keys, the schedule and the inbox. Without the state archive the new machine starts empty (the built-in agents and library skills are
seeded at the first start).

Packing needs `git`, `tar`, `gzip` and, for `--with-state`, the `sqlite3` command-line tool.

## What the machine needs

To build (Linux amd64 and macOS arm64 are what the build and its tests are verified on; the jail has an adapter for each,
`docs/adr/0021-the-jail-has-an-adapter-per-os.md`):

- Rust through `rustup`, which installs the toolchain `rust-toolchain.toml` pins (1.99.1), `git`, `make`, `tar`, Node 24 and `npm` (the web app is built with them and embedded into the binary).
- `make gen` regenerates the TypeScript of the contracts in `crates/wire` (`crates/tsgen`), and `make test` checks the generated files.
- GNU make 4 for `make test`, which runs its parts with `--output-sync`: macOS ships make 3.81, so `brew install make` and put
  `$(brew --prefix make)/libexec/gnubin` first on the PATH.
- Network access to crates.io and the npm registry: the package carries no vendored dependencies, so an offline build is not possible from it.

To run:

- On macOS, version 14.4 or later, which provides identity-bound process signaling. Force kill on Linux requires pidfds and `openat2` (kernel 5.6 or later).
- The `claude` CLI (Claude Code) on the PATH of the service's OS user, **signed in as that user** (`claude`, then log in). Every agent runs on it; there is no API key to set. `-claude-config-dir` points at a login kept elsewhere. Agents run in `<data>/claude`, which links that login on Linux; on macOS, where the login is in the Keychain, sign the default account in with `CLAUDE_CONFIG_DIR=<data>/claude claude auth login`. Without a login the server still starts and the default account is signed out: agents run on the other accounts (a login, or a subscription's token, ADR-0129); more accounts are added with the Add account button on the Accounts page, which gives the owner a sign-in link (no command to type; `claude-accounts/<id>` is created next to `-claude-config-dir`); sessions started before it existed keep their transcripts in the login's `projects/`, and are found again once the `-claude-config-dir/projects/` entries of the chains' directories are copied there.
- `git`, and the forge's own command line signed in to each host the repositories live on (`gh` for GitHub, `glab` for GitLab, as `-forges` names them) with an account that may push to each base branch Remit lands on (ADR-0114: Remit's merge queue fast-forwards the base itself, never forced, so a protected base gives that account a bypass or an allowed role; a refused push goes back to its Chair and raises an item for the Assistant), plus an SSH key for pushing. Every cell needs at least one such remote: all work ends as a landing on a base. A chain's branch (`feature/mc-*`) stays in its worktree and is never pushed.
- Agents run as the service's OS user (`docs/adr/0004-agents-run-as-the-servers-os-user.md`): give it its own account, not a shared one.
- Outbound access to Anthropic and to each forge host in `-forges`.

## Unpack and build

```sh
sha256sum -c SHA256SUMS
git clone -n remit-<sha>.bundle remit            # the history, nothing checked out
tar -xzf remit-<sha>[-dirty].tar.gz -C remit --strip-components=1   # the packed tree over it
git -C remit reset -q                                 # index from HEAD: `git status` now shows what was uncommitted, and `PACKAGE`
cd remit && make deps build                           # bin/remit, web app embedded
```

A package with `uncommitted: 0` in `PACKAGE` leaves `git status` clean. For a package packed from a dirty tree, `git status`
lists the same paths the old machine's did; commit them there before packing when you want the history to say what runs.

`make test` runs the contract check, the Rust workspace (formatted, linted, tested), the web and script tests; `make build` alone is enough to run.

## What lives outside the repository

Remit starts with no flag, but does its job with the rest:

1. **The owner key.** Made at the first start in `<data>/owner.key`; the server prints the link that holds it. Details: `README.md`, Signing in.
2. **Repositories need no registration.** A chain opens one by its remote (`github.com/owner/name`, a remote URL, or the path of a checkout) and Remit clones it into `<data>/repos/<host>/<project>` with the machine's git credentials (ADR-0038). The new machine needs the SSH key or credential helper that can clone them, and a forge for each host in `-forges`.
3. **The principles file** (`-principles`) given to every agent. When the file is absent it is left out.
4. **Jira for `list_issues`** (optional): the env file `-jira-env` names. Absent, `list_issues` of a chain whose playbook holds the `issues` capability has no Jira to read.
5. **Credentials that are not in any package:** the `claude` login, the forge CLIs' tokens (`gh`, `glab`), the SSH key, the Jira env file, and the client keys the holders of outside agents keep in their own files (`-client-key-files`, `docs/adr/0009-client-keys.md`). Redo each on the new machine.

## First start

```sh
./bin/remit -addr 127.0.0.1:4320 -data ~/.remit
curl -s http://127.0.0.1:4320/healthz
```

`/healthz` answers `{"ok":true,"revision":"<sha>",...}` once the database answers, the scheduler has ticked and the AI runtime has connected; 503 with the
reason when not. A revision of `unknown` means the tree had no git history when it was built. The data directory (`-data`)
is created at the first start: `remit.db`, `secret.key`, `owner.key`, `node.key`, `sources.env`, `backups/`, `work/`, `repos/`, `library/`, `claude/`, `assistant/` and, once a plugin is applied, `ext/`.

## Keep it running

`deploy/remit.service` is a systemd unit: copy it to `/etc/systemd/system/`, set the user, the working directory and the flags, then `systemctl enable --now remit`. Its `KillMode=process`
matters: Remit ends its AI runtime (`remit runtime`, its child, which holds every agent process) itself, cancelling
the agents' turns so the next start resumes them, and a stop that signals the whole control group kills them mid-turn. A server
that dies, even by SIGKILL, takes the runtime with it (ADR-0058), so the supervisor runs only the server. Any other supervisor needs the same property (pm2:
`treekill: false`).

The unit has not been run on a systemd host by the people who wrote it: read its first start in `journalctl -u remit`.

On macOS, `deploy/remit.plist` is the same as a launchd job of the user's own: replace its paths, copy it to
`~/Library/LaunchAgents/remit.plist` and `launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/remit.plist`; it
starts at every login and again whenever it ends, and its AbandonProcessGroup key is the unit's `KillMode=process`. A server started from a
shell (`nohup … &`) is not supervised: it shares the shell's process group and ends with it.

### Reaching it from a browser

The binary serves HTTP on TCP at `-addr`, which stays on loopback (the owner key travels in it), and the link
(WebTransport, `/link`) on UDP at `-link-addr` (default: `-addr`). HTTP carries the web app, `/link.json` (where a page opens
its link session: the host it reached the server by, at the link's port, and the hash of the certificate to trust there),
`/healthz`, `/auth/config`, `/auth/google`, `/hooks/<name>`, `/mcp` and `/mcp/client`. The link carries the web app's
session (the owner's or a mentor's, `wire::app`) and the AI runtime, which dials it on loopback. Its certificate is
self-signed, made at each start and valid less than two weeks (`crates/remit/src/identity.rs`), so a browser trusts it by
the hash `/link.json` gives, which it asks again before each dial: an open tab reconnects after a restart.

For other devices, put a TLS proxy in front that forwards HTTP to `-addr`, and let the link listen beyond loopback with
`-link-addr=0.0.0.0:<port>` so the browser reaches its UDP port directly: no HTTP proxy, Cloudflare's included, carries
WebTransport. The browser dials the link at the host name it opened the page by, so that name must resolve to an address of
this machine that the browser can reach on UDP. Agents reach `/mcp` on loopback and a client key's holder `/mcp/client`,
so the proxy forwards neither; senders of events (GitLab, Sentry, a script through `remit emit`) must reach `/hooks`, and
each event signal's own key protects its door.

#### Your own host name on a tailnet

Remit on a private network under a host name of your own, here `remit.example.com` on a Tailscale tailnet:

1. DNS: an A and an AAAA record for the name pointing at the machine's Tailscale addresses (`tailscale ip`), DNS only
   (not proxied). The name resolves for anyone, but only the tailnet routes to those addresses.
2. A certificate: Let's Encrypt cannot reach a tailnet address over HTTP, so the proxy proves the name by a DNS challenge.
   With Cloudflare DNS, Caddy built with the `caddy-dns/cloudflare` module (`caddyserver.com/api/download?os=<os>&arch=<arch>&p=github.com%2Fcaddy-dns%2Fcloudflare`)
   and an API token with DNS Edit and Zone Read on that zone only, in a file of mode 600.
3. The proxy, run by the same supervisor as Remit (a launchd job or systemd unit running `caddy run --config <Caddyfile>`):

```
{
	auto_https disable_redirects
}

https://remit.example.com {
	tls {
		dns cloudflare {file./path/to/cloudflare.token}
		resolvers 1.1.1.1
	}
	@outside not remote_ip 100.64.0.0/10 fd7a:115c:a1e0::/48
	abort @outside
	@agents path /mcp /mcp/*
	abort @agents
	reverse_proxy 127.0.0.1:4320
}
```

   macOS lets a user's process bind port 443 only on the wildcard address, so Caddy listens on every interface and
   `@outside` closes each connection that does not come from the tailnet. Port 80 is not opened.
4. Remit with `-link-addr=0.0.0.0:4320`, and, when the owner signs in with Google, the name among the Firebase project's
   authorized domains.

Check from a device on the tailnet: `curl https://remit.example.com/healthz` answers, `/link.json` names
`https://remit.example.com:4320/link`, and the page's link session opens.

For a tailnet's own name instead, `tailscale serve --bg 4320` is the whole proxy (README, *Signing in*), with the same `-link-addr`.

## Moving the owner's data

On the old machine, with the package built `--with-state` (the copy is consistent while the server runs). Before it, let the
chains that run finish or stop them: a running agent's conversation is resumed from Claude Code's own transcripts under the
claude config dir (`projects/`), which the state archive does not carry, and `work/` (the chains' git worktrees; 9.7 GB on the
host this was written on) is not carried either. Check that nothing you need exists only in a worktree: a change
lands on its base once its gates approve it, and one still judging or ready lives only in its worktree and the database. (Not checked by reading the code: do it before retiring the old machine.)

On the new machine, with the server **stopped** and `-data` not yet holding a database:

```sh
tar -xzf remit-state-<time>.tar.gz
cd remit-state-<time>
install -m 600 remit.db secret.key <data>/
cp -R backups <data>/                     # the key's backups, one file per key id
```

Start it. The server refuses a key file that others can read, or one that is not 32 bytes, and leaves it alone: replacing
it would orphan every credential sealed under it. If an event signal's key or a source credential reads as damaged, the key is not
the one that sealed it. The database migrates forward at the start, after a copy of it in `<data>/backups`.

## After the move

- `/healthz` is 200 and its revision is the commit in `PACKAGE`.
- Open the link with the owner key (the old machine's `owner.key` moves with the data, or a new one is made); the board shows the chains of the old machine.
- The Health bar names no provider problem (the `claude` login works).
- The door of the `gitlab` event signal (Signals → Signals, the page of `event:gitlab`) is reachable from GitLab at the new host name; update it in each project when the host name changed.
- Retire the old machine's service so two Remit do not land on the same bases.

## Backups and upgrades

The server copies its database into `<data>/backups` daily (`-snapshot-every`), before each migration and before each
`remit update` restarts it, and keeps the key's backups in `-key-backup-dir`. Remit has no restore feature: a snapshot is
put back by copying the file, and the work itself lives in Git on the forge.

To upgrade, run `remit update` (below). When the new release does not come up it goes back to `previous` by itself. A
release that migrated the database is one-way, as the release before refuses a database migrated past what it knows:
when it does not come up, `remit update` also puts the `pre-update-…` copy back (`store::restore`, which keeps the
migrated database as a `pre-restore-…` copy), losing only what the failed release wrote. After a healthy one-way
release the way back is forward: a release that reverts the code and keeps the migration.

## Install a release

A machine that only runs Remit holds no source. Every push to `master` is a release: GitHub's release workflow
(`.github/workflows/release.yml`) builds and tests the commit and only then publishes it, tagged `<yyyymmdd>-<short sha>`
(`scripts/release_tag.sh`, the day the commit was made, in UTC), with `remit-linux-amd64`,
`remit-client-linux-amd64` and `SHA256SUMS`;
only linux-amd64 is published. The commit's code builds and runs under a read-only token; a separate job holding the
write token runs none of it and only uploads the built files. A run made again for a release missing any of them
uploads the whole set again, so `SHA256SUMS` matches every file, and never creates the release twice. On the running machine, with `gh` signed in to GitHub with read access to the repository (`gh auth login`, or `GH_TOKEN`), and the supervisor running
`~/.remit/releases/current/remit` with `~/.remit` as its working directory:

```sh
~/.remit/releases/current/remit update -repo OWNER/remit -restart 'pm2 restart remit'
```

Under a lock, so two never race, it downloads the newest release (`-to <tag>` names another), checks it against
`SHA256SUMS`, installs it as `~/.remit/releases/<tag>/remit`, points `current` at it and `previous` at the release before
(each by a rename), copies the database into `<data>/backups/pre-update-…`, restarts the server with the `-restart`
command (run by `sh -c`; `systemctl restart remit` and `launchctl kickstart -k gui/<uid>/remit` for the other
supervisors), and waits up to `-wait` (2 minutes) until `/healthz` is healthy on the release's commit and a hello with
the owner key is welcomed on the link. When it is not, it points `current` back at `previous` and restarts again,
putting the database's copy back first when the release migrated it (*Backups and upgrades*). It prints one JSON object,
`{id, from, to, changed, backup, healthy, rolledBack, oneWay, error}`, writes the same object to
`<releases>/update.json` (the server reads the last update's outcome there by its `id`), and exits 1 unless the server
ends healthy on the release asked for. Run again, it only checks. Give it the server's `-addr` and `-data` when they are not
the defaults; `-gh` names a `gh` that is not on the PATH. The first install, with no release on the machine yet, runs
the downloaded binary itself:

```sh
gh release download -R OWNER/remit -p remit-linux-amd64 -D /tmp/remit-first && chmod +x /tmp/remit-first/remit-linux-amd64
/tmp/remit-first/remit-linux-amd64 update -repo OWNER/remit -restart 'pm2 restart remit'
```

A server that runs Remit's own repository installs each release by itself (ADR-0116): started with `-update-repo
OWNER/remit` and `-update-restart '<the supervisor's restart command>'`, it releases what lands on that repository by the
repository's `.remit/release/`, whose one step waits for the workflow's release of the landed commit, and then runs
`remit update -to <tag>` with its own flags as above, guarded by `fact:remit.health` for an hour; a breach installs
`previous` and reverts what the release carried. The step reads GitHub through the connection its `connections` file
names (`github/default`): give the `github` source a connection `default` by telling the Assistant, type its token into
the inbox item that asks for it, and the repository's grant on it is asked by the chain that adds the connection and
decided by the Assistant. Any repository releases the same way: the connections its `.remit/release/connections` names
are the only places its steps reach, each through a grant the repository holds, behind Remit's proxy, which adds the
credential; a step publishes (writes) only through an `irreversible` grant. A repository with no `.remit/release/` is live
once its landing's checks stand.

`remit doctor` (same `-addr` and `-data`) checks a running server and changes nothing: `/healthz`, whether it runs the
commit of the binary asking, the link's certificate (made at each start, valid 13 days) and a hello on the link over UDP.
One JSON row per check, `{name, ok, detail, fix}`; exit 1 when one fails.

## Limits of this guide

- Not verified: a systemd start of the unit, nginx in front, a Windows or Linux arm64 build, and an offline build. macOS
  has no systemd: a supervisor there (launchd, pm2) needs the same property as the unit's `KillMode=process`.
- On macOS a session can read the server's environment (ADR-0021): start the server with no secret in its environment.
- No container image is provided. Agents run as the OS user with that user's `claude` login, `glab` token and SSH key, and work in git worktrees of checkouts on the host; an image has to mount all of them.
- The package is the working tree, not a release. A release is what GitHub publishes from a push to `master` (*Install a release*).
