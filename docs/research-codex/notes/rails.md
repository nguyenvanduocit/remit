# Codex vs Remit: rails and entrances

Read-only study of OpenAI Codex (`references/codex/codex-rs`, Apache-2.0) against Remit's jail, egress, secrets, childenv,
procgroup, auth and entrance keys. Both directions: what Remit can take into the crates it has, and Codex engineering
Remit has nothing like. Paths starting `codex-rs/` are in the Codex clone. Other paths are Remit's, under
`crates/` unless they start with `docs/`.

Labels: **HIGH** = verified (code read at that line, or an experiment run on this Mac), **MEDIUM** = inferred from code
or from OS behaviour I know but did not run, **LOW** = a guess with the step that would check it.

## Coverage

- Remit, read in full: `jail/src/{lib,spec,plan_darwin,sandbox_darwin,plan_linux,namespaces_linux,supervise_linux,wrap}.rs`,
  `jail/README.md`, `egress/src/{lib,policy,denied}.rs`, `childenv/src/childenv.rs`. Read in part: `jail/src/mounts_linux.rs`
  (lines 1-75), `egress/src/{egress,inject,inject_rules,inject_exchange}.rs` (headers and signatures), `secrets/src/secrets.rs`,
  `auth/src/local.rs`, `procgroup/src/procgroup.rs`, the module headers of `orch/src/{client_keys,worker_keys,entrance_keys}.rs`,
  `door/src/lib.rs` (grep), `orch/src/repochecks.rs:130-230`, `remit/src/flags.rs:435-505`, ADR-0018 and ADR-0021 *Revisit when*.
- Codex, read: `process-hardening/src/lib.rs` (all), `sandboxing/src/{lib,denial,seatbelt_daemon}.rs`, `seatbelt.rs:470-580,980-1078`,
  all four `.sbpl` files, `violation.rs:1-80`, `policy_transforms.rs:1-175`, `manager.rs:1-60`, `sandbox_integrity/*`,
  `linux-sandbox/src/seccomp.rs:1-140,180-305`, the `bwrap.rs` argument list (grep), the test names in `linux-sandbox/tests/suite/sandbox.rs`,
  `network-proxy/README.md`, the heads of `credential_broker.rs`, `credential_broker/providers/github.rs`, `secret_substitution.rs` and `mitm_hook.rs`,
  `execpolicy/README.md` and `src/decision.rs`, `secrets/src/{lib,sanitizer}.rs` and `local.rs` (grep), `websocket-auth/src/lib.rs:1-60`,
  `user-verification/src/lib.rs`, `responses-api-proxy/src/read_api_key.rs:1-60` and `lib.rs` (grep).
- Skipped: `windows-sandbox-rs` and `windows*.rs` (Remit has no Windows adapter and none was asked for), `landlock.rs` (Codex
  itself calls it legacy, `codex-rs/linux-sandbox/src/seccomp.rs:3-4`), the bodies of `bwrap.rs`, `proxy_routing.rs`, `manager.rs`,
  `shell-command`, `keyring-store`, and every test body except the names.
- Experiments: two runs of `/usr/bin/sandbox-exec` on this Mac against throwaway files in `mktemp -d` (removed afterwards).
  Nothing was built and no Remit code was run.

## Defects found on the way (fix now: these are not hardening)

AGENTS.md waits for the owner's request before building hardening, but a rail that does not do what its docs promise is a
defect. These three come straight from Codex's handling of the same problems.

### D1. macOS: renaming a parent directory un-hides a hidden file — **HIGH (reproduced)**

- Codex: `codex-rs/sandboxing/src/seatbelt.rs:1013-1021` denies `file-write-unlink` on every ancestor directory of a protected
  path, because "renaming an allowed ancestor relocates its protected descendants past their pathname carveouts". The same
  in `seatbelt_daemon.rs:14-22` and for writable roots in `seatbelt.rs:504-509`.
- Remit: the Wrap profile is `(allow default)` plus `(deny file-read* file-write* (subpath hidden))`
  (`jail/src/sandbox_darwin.rs:62-77`). It has no rule on the ancestors. The jail profile is the same (`sandbox_darwin.rs:49-54`).
  `flags.rs:466-505` hides the sealing key, owner key, database and others inside the data directory, which is in neither list.
- Reproduced with a Remit-shaped profile: `cat data/secret.key` → `Operation not permitted`; then
  `mv data moved && cat moved/secret.key` → `SECRET`. Any Claude Code session on macOS can do this to `~/.remit`
  (the server breaks too, but by then the key has been read).
- Fix, also tested here: for each hidden path, add `(deny file-write-unlink (require-all (vnode-type DIRECTORY) (literal A)))`
  for every ancestor A, starting at the file's parent. With it, both renames are refused, a `cp -R` copy fails on the
  hidden file, and creating a sibling file still works. It is a few lines in `hidden_profile` and `Sandbox::profile`,
  plus a case in `jail/tests/jail.rs` `hides_in_a_wrap` that runs `mv` on the parent. Effort S.

### D2. Linux Wrap: `/proc/<pid>/root` reads past the hiding mounts — **MEDIUM (not run, Linux)**

- Codex: bubblewrap always runs with `--unshare-pid` and a fresh `--proc` (`codex-rs/linux-sandbox/src/bwrap.rs:282,288,356,364`).
- Remit: a Wrap unshares only the user and mount namespaces (`jail/src/namespaces_linux.rs:65-68`) and keeps the host's
  `/proc`. Hiding is done with mounts inside that mount namespace (`mounts_linux.rs:21-28`). Any process of the same user
  outside the namespace (the server, pm2, a shell) exposes the host's view at `/proc/<pid>/root/...`. On Linux the only
  check on that is `ptrace_may_access(PTRACE_MODE_READ)`, which a same-uid process passes, and Yama only restricts
  ATTACH. `jail/README.md` already lists "a Linux Wrap has no pid namespace" under *Revisit when*, but only because of
  orphaned processes, not this.
- How to falsify: on the pod, from a session, run `cat /proc/$(pgrep -o -u $USER pm2)/root$HOME/.remit/<owner key file>`.
  Refused means this finding is wrong.
- Fix: give a Wrap its own pid namespace and mount a new `/proc`. The second stage then becomes init and reaps, as
  `supervise_linux.rs:38-53` already does for a jail. A Wrap still needs the host network, so no `CLONE_NEWNET`. Effort M.
  Making the server non-dumpable (I1) does not close this, because every other same-uid process still exposes it.

### D3. A key-result command check can write the worktree, `.git` included — **HIGH (code), MEDIUM (exploit)**

- Codex: inside every writable root, `.git`, `.agents`, `.codex` and `.aws` stay read-only
  (`codex-rs/protocol/src/permissions.rs:43-51`, enforced at `sandboxing/src/seatbelt.rs:558-562` and `linux-sandbox/src/bwrap.rs:786-790`).
  Its suite has `sandbox_blocks_metadata_writes_inside_writable_root` (`linux-sandbox/tests/suite/sandbox.rs:908`).
- Remit: `command_check` says "the worktree read-only" (`orch/src/repochecks.rs:167-169`), but it passes the worktree as
  both `work_dir` and `repo` (`repochecks.rs:184-190`), so it is `Spec::dir`, "the one path the program can write"
  (`jail/src/spec.rs:22`). On macOS the profile allows writes under `dir` and denies nothing on `read_only`
  (`sandbox_darwin.rs:39-42`). On Linux `(dir, true)` sorts after `(dir, false)` and is mounted last, read-write
  (`mounts_linux.rs:46-51`).
- Why it matters: a check can rewrite the worktree's `.git` file to point at a gitdir it controls, with `core.fsmonitor` or
  `core.hooksPath` set. The server's next `git` command in that worktree then runs the attacker's program outside the jail.
- Fix, two parts: (a) `command_check` gets a scratch `work_dir` and keeps the worktree read-only, as the declared checks
  already do (`repochecks.rs:160-163`). (b) In the jail, paths listed in `read_only` stay read-only even when they lie
  inside `dir`: on macOS a `(deny file-write* ...)` after the allow, on Linux the read-only bind mounted after the
  writable one. Then a caller can protect `dir/.git`. `repochecks.rs` belongs to review-and-ship, so (a) goes to them.
  Effort S.

## Ranked ideas: Remit's crates against Codex

| # | Idea | Value | Effort | Kind |
|---|------|-------|--------|------|
| 1 | D1 ancestor-unlink deny (above) | HIGH | S | defect |
| 2 | D2 Wrap pid namespace + own `/proc` (above) | HIGH | M | defect (if confirmed) |
| 3 | D3 read-only metadata inside writable dirs (above) | HIGH | S | defect |
| 4 | Make the server process hard to inspect | MED-HIGH | S | hardening |
| 5 | Write a Seatbelt base of our own in place of `bsd.sb` | MED | M | hardening |
| 6 | Deny the fcntl, TIOCSTI and XPC holes on macOS; setsid on Linux | MED | S | hardening |
| 7 | A seccomp layer and the IPC/UTS namespaces on Linux | MED | M | hardening |
| 8 | Placeholder credentials and a broker with many destinations | MED | L | make it work (when CLIs need creds) |
| 9 | A decision path when an allowlist misses | MED | M | make it work (needs ADR) |
| 10 | Keep the model-provider credential out of sessions | MED | L | hardening |
| 11 | Wildcard and deny entries in the egress allowlist | LOW-MED | S | make it work when asked |
| 12 | An integrity test of everything the server trusts | LOW-MED | S | hardening |
| 13 | Paths passed to Seatbelt as `-D` parameters | LOW | S | quality |
| 14 | Linux parent-death signal on the helper | LOW-MED | S | hardening |

### 4. Make the server process hard to inspect — MED-HIGH, hardening

- Codex: before `main` it sets `PR_SET_DUMPABLE=0` on Linux, `PT_DENY_ATTACH` on macOS, `RLIMIT_CORE=0`, and removes
  `LD_*`/`DYLD_*` (`codex-rs/process-hardening/src/lib.rs:43-61,82-100,102-117`). It uses `unsafe` libc calls for all of
  it (`lib.rs:46,85,109,127`). **HIGH**
- Remit: core limit 0 for jailed programs only (`jail/src/supervise_linux.rs:136-137`, `init_darwin.rs:76`). The server
  holds the sealing key and the owner key in memory and gets no protection. **HIGH** (`rg dumpable|DENY_ATTACH` finds nothing)
- Proposal: a `harden` function in `procgroup` or a new small crate, called first thing in `main`:
  `rustix::process::set_dumpable_behavior(NotDumpable)` and `setrlimit(Core, 0)`, both safe APIs, so no exception to
  `forbid(unsafe)`. Non-dumpable makes `/proc/<server>/{mem,environ}` unreadable to sessions on Linux. `PT_DENY_ATTACH`
  has no safe binding I know of (**LOW**: check `nix::sys::ptrace` on macOS). If there is none, macOS stays as ADR-0021
  already describes. Revisit-when entry: ADR-0021.

### 5. A Seatbelt base of our own in place of `bsd.sb` — MED, hardening

- Codex: starts from `(deny default)` with a hand-picked base derived from Chrome (`codex-rs/sandboxing/src/seatbelt_base_policy.sbpl:1-116`):
  sysctls by name, `process-info*` only `same-sandbox`, a few named mach services. The network extras are in a separate
  file (`seatbelt_network_policy.sbpl`), and it adds `(deny mach-lookup (xpc-service-name-prefix ""))` (`seatbelt.rs:1011`). **HIGH**
- Remit: the jail starts `(deny default)(import "bsd.sb")` (`sandbox_darwin.rs:28`). On this Mac, `bsd.sb` grants
  `(allow file-read-metadata)` on every path (`/System/Library/Sandbox/Profiles/bsd.sb:18`) and imports `system.sb`, which grants
  `(allow sysctl-read)` for everything (`system.sb:191`) and every XPC service lookup (`system.sb:21-23`). **HIGH**
- Gap: a jailed check can stat any host path, hidden ones included (sizes, times, existence), and read every sysctl.
  Whether that includes `kern.procargs2`, the argv and environment of other same-user processes, is **LOW**:
  `/bin/ps` would not exec in my probe. Check with a small probe that calls `sysctl(KERN_PROCARGS2)` from inside the jail.
- Proposal: vendor `seatbelt_base_policy.sbpl` (Apache-2.0 notice in the file header and in NOTICE) as
  `jail/src/base_darwin.sbpl`, `include_str!` it, and drop the `bsd.sb` import. Run `tests/jail.rs` plus the real
  checks (git, cargo, node, python) to find what it breaks. Effort M, mostly testing tools.

### 6. The fcntl, TIOCSTI and XPC holes — MED, hardening

- Codex: `(deny system-fcntl (fcntl-command 80 110))` because F_MAKECOMPRESSED and F_TRANSFEREXTENTS change a file through a
  read-only descriptor and skip `file-write*` (`seatbelt.rs:1025-1029`, test `seatbelt_fcntl_tests.rs:53`). It also has
  `(deny file-ioctl (ioctl-command TIOCSTI))` when a terminal is inherited (`seatbelt.rs:1031-1037`), and bwrap
  `--new-session` (`bwrap.rs:264,334`). **HIGH**
- Remit: `read_only` in a Wrap is `(deny file-write* ...)` only (`sandbox_darwin.rs:73-75`), and `procgroup` uses
  `process_group(0)`, not a new session (`procgroup/src/procgroup.rs` `command`). A session started from `make dev` in a
  terminal shares that terminal. **MEDIUM**: I did not try TIOCSTI injection.
- Proposal: add both denies to both profiles. On Linux the helper calls `rustix::process::setsid()` (safe) before it
  starts the program. Effort S.

### 7. A seccomp layer and more namespaces on Linux — MED, hardening

- Codex: a seccomp filter (`seccompiler`, whose API is safe) blocks `ptrace`, `process_vm_readv/writev` and `io_uring_*`.
  `io_uring` can create AF_VSOCK sockets without calling `socket()`. AF_VSOCK itself is blocked even when the network is
  allowed (`codex-rs/linux-sandbox/src/seccomp.rs:192-200,272-279`; VM sockets reach host services: `seccomp.rs:59-63`).
  bwrap also passes `--unshare-ipc` (`bwrap.rs:279,358`). **HIGH**
- Remit: user, mount, net and pid namespaces, nothing else (`namespaces_linux.rs:65-68`). There is no seccomp. **HIGH**
  AF_VSOCK is not network-namespaced, so a jail on a VM host can reach the host's vsock services. **MEDIUM**
- Proposal: add `CLONE_NEWIPC | CLONE_NEWUTS` to the jail (one line). The second stage installs a seccomp filter
  after `give_up_privileges` (`supervise_linux.rs:20`), applied to the program's thread. Revisit-when entry:
  ADR-0021. Effort M.

### 8. Placeholder credentials and a broker for many destinations — MED, make it work when a CLI needs a credential

- Codex: the child gets a *dummy* credential shaped like the real one, with the same prefix and length, so tools that
  check its format accept it (`codex-rs/network-proxy/src/credential_broker/providers.rs:204`,
  `providers/github.rs:13-60`). The proxy swaps it for the real one only on a request that carries the placeholder and
  matches a host/method/path hook. It re-signs AWS SigV4 and bounds body substitution exactly
  (`network-proxy/README.md:281-330`). **HIGH**
- Remit: the `Injector` has one destination (`base`). It replaces the credential headers on every request and admits
  only reads (`egress/src/inject.rs:47-76`, `inject_rules.rs:60`). The `Sign` hook already covers AWS
  (`inject.rs:36-41`). **HIGH**
- Gap: an agent cannot run `gh` or `aws` in a jail against more than one endpoint, or write anything, without holding
  the credential.
- Proposal: `InjectConfig` grows a list of `(host, methods, path prefixes, credential)` routes, and a placeholder value
  the program must present before its credential is swapped in. `childenv` hands out the placeholder. Owner:
  rails-and-sandbox with sources-and-plugins (grants decide the routes). Effort L. Pre-mortem first: a placeholder that
  leaks into a commit is harmless, but a swap that fires on the wrong host leaks the credential, so the swap must
  follow the destination check, as Codex does (`secret_substitution.rs:1-2`).

### 9. A decision path when the allowlist misses — MED, make it work (needs an ADR)

- Codex: a `NetworkPolicyDecider` can turn an allowlist miss into allow, deny or ask. "Explicit deny rules still win",
  and the decider never overrides `denied` or `not_allowed_local` (`network-proxy/README.md:189-198`). **HIGH**
- Remit: a miss is refused and counted (`egress/src/egress.rs:280-292`, `denied.rs`). The owner sees it after the run.
- Proposal: a seam `Config::on_miss: Option<Arc<dyn Miss>>` that holds the connection while the owner, or the Assistant
  on his behalf, decides. No other agent may widen egress, since that would lift a rail (AGENTS.md). Never for a
  non-routable address. architect writes the ADR.

### 10. The model credential out of sessions — MED, hardening

- Codex: `responses-api-proxy` reads the API key from stdin into one zeroized, mlocked buffer, with `read(2)` and no
  BufReader, so no second copy exists (`codex-rs/responses-api-proxy/src/read_api_key.rs:38-60`). It admits only
  `POST /v1/responses` (`lib.rs:234-239`). **HIGH**
- Remit: a session holds its account's login and "cannot run without reading it" (`childenv/src/childenv.rs:18-21`). **HIGH**
- Proposal: run an `Injector` in front of the provider, with the session given a placeholder token and the base URL.
  Whether Claude Code works through a base-URL proxy with a dummy OAuth token is **LOW**: test with one plugin account
  first. Revisit-when entry: ADR-0071.

### 11. Wildcard and deny entries in the allowlist — LOW-MED

- Codex: `*.x` matches subdomains, `**.x` the apex and subdomains, a bare `*` is refused, deny wins
  (`network-proxy/README.md:251-258`). Remit: exact `host:port` only (`egress/src/policy.rs:122-159`). Build it when a
  grant needs it, and keep the punycode-only rule (`policy.rs:82-108`).

### 12. Integrity test of what the server trusts — LOW-MED, hardening

- Codex: `FileContentsChecker` reports when a policy lets the sandbox write a file the next sandbox is built from
  (`codex-rs/sandboxing/src/sandbox_integrity/contents.rs:1-40`, `finding.rs:6-18`). **HIGH**
- Proposal for Remit: a test over `Flags::protected_from_agents` (`remit/src/flags.rs:466-505`) that lists every file
  the server reads to build a rail or start itself (config, launchd plist, `update.sh`, ext programs, plugin binaries)
  and asserts each is hidden or read-only. The data directory is writable by design, so new files in it are the risk.

### 13. Seatbelt paths as parameters — LOW, quality

- Codex passes each path as `-DNAME=value` and refers to `(param "NAME")` (`seatbelt.rs:1046-1052`). Remit quotes paths
  into the text (`sandbox_darwin.rs:84-88`). That is correct today, but a parameter cannot be an injection. Take it with #5.

### 14. Parent-death on Linux — LOW-MED, hardening

- Codex: `--die-with-parent` (`bwrap.rs:265,335`). Remit: if the server is SIGKILLed, nothing drops the
  `procgroup::Child`, so the jail lives until the program ends. Only `RLIMIT_CPU` bounds it, and a sleeping program
  costs no CPU (**MEDIUM**). `rustix::process::set_parent_process_death_signal` is safe, but it fires when the
  *thread* that spawned the child exits, and a tokio blocking thread exits when idle. The first stage would have to be
  spawned from a long-lived thread. **MEDIUM**

## Codex does better / Remit lacks (Codex on its own terms)

| Codex engineering | Where | Remit today | Take it? | Conf. |
|---|---|---|---|---|
| **Attack-catalog tests**: symlink replacement, metadata writes in writable roots, retained capabilities, no_new_privs set, real `curl/wget/ping/nc` blocked, carve-outs under bwrap. 53 Seatbelt tests and 34 policy-transform tests | `linux-sandbox/tests/suite/sandbox.rs:725-1328`; `sandboxing/src/seatbelt_tests.rs` (3216 lines) | `jail/tests/jail.rs`: three scenarios (`hides_in_a_jail`, `hides_in_a_wrap`, `stop_ends_everything`, `:76-121`) | **Yes, first.** D1-D3 would have been caught by such tests. Write one test per escape class: rename ancestor, `/proc/<pid>/root`, `.git` rewrite, hard link, symlink swap, fcntl, TIOCSTI. They guard the rail, so they count as the rail's main path, not later hardening | HIGH |
| **Process hardening** before `main` | `process-hardening/src/lib.rs:12-25` | none for the server | Yes (#4), without `unsafe` | HIGH |
| **Unified violation events**: `SandboxViolationEvent::{FileSystem,Network}` with backend, typed reason, path, a 512-char output snippet | `sandboxing/src/violation.rs:8-80`; heuristics `denial.rs:13-72` | egress denials only, with a free-text reason (`egress/src/denied.rs:11-20`). A jail's filesystem refusals are not recorded. `RunResult` has no "refused" (`jail/src/spec.rs:44-54`) | Yes, a typed reason enum for `Denial` now. Filesystem refusals: Codex guesses from output keywords (`denial.rs:50-58`), which is weak. Better for Remit to read the kernel's own report (macOS `log stream --predicate 'sender=="Sandbox"'` for the sandbox's pid) so the agent's screen shows what the rail stopped | HIGH (Codex), MEDIUM (log approach) |
| **Machine-readable refusal**: `x-proxy-error: blocked-by-allowlist|denylist|method-policy|policy` | `network-proxy/README.md:145-149` | text body `egress gate: <reason>` (`egress/src/egress.rs:280-285`) | Yes, cheap: an agent's tool can tell "refused by the rail" from "upstream 403" | HIGH |
| **Permission-profile algebra**: merge and intersect (requested ∩ granted), so a request can never exceed its grant; glob entries only as deny-read | `sandboxing/src/policy_transforms.rs:19-40,93,147-175` | callers build `jail::Spec` by hand (`orch/src/repochecks.rs:214-225`, `program.rs`, `script.rs`) | Later. When a second caller composes a Spec from a grant plus a playbook, put the intersection in one pure function (sources-and-plugins with us) | MEDIUM |
| **Policy files that test themselves**: each rule has `match`/`not_match` examples checked at load time, and `forbidden` carries a justification that suggests an alternative | `execpolicy/README.md` (rule shape), `src/decision.rs:7-18` | rules live in code, and their tests are tables (`egress/src/inject_rules.rs`) | Take the idea, not the language: when the owner's words become a grant or allowlist, store examples with it and check them at Apply. This matches ADR-0045's dry-run gate | MEDIUM |
| **Seatbelt protection of the privileged socket** with ancestor-unlink denies | `sandboxing/src/seatbelt_daemon.rs:7-22` | the hidden-socket deny exists (`sandbox_darwin.rs:67-71`), the ancestor part is missing (D1) | Yes (D1) | HIGH |
| **Network proxy as a product**: SOCKS5, limited mode (GET/HEAD/OPTIONS enforced through MITM), MITM hooks by host/method/path/query/header, strict config that rejects unknown fields, OTEL audit per decision with no URL path | `network-proxy/README.md:1-330`, `mitm_hook.rs:33-50` | the Gate (blind CONNECT, allowlist) and the Injector (reads only, one base) | Hooks and strict config, yes, folded into #8. Skip SOCKS5 until a tool needs it. The audit-without-path rule matches what `denied.rs` already does | HIGH |
| **Secret redaction regexes** (Bearer, `sk-`, AKIA, `key=value` assignments) | `secrets/src/sanitizer.rs:4-22` | `childenv::secret_names` checks names only (`childenv/src/childenv.rs:53-80`). The Injector scrubs its own credential (`inject.rs:43-44`) | Partly. Owned by sources-and-plugins (source redaction). A generic net for transcripts is LOW value: regexes miss most formats | MEDIUM |
| **Device-bound user verification**: Secure Enclave key plus biometrics, idempotent create and delete, a guard checked after every blocking wait | `user-verification/src/lib.rs:28-50` | worker keys are bearer codes (`orch/src/worker_keys.rs` header) | Later hardening for mentors' devices: sign a challenge instead of presenting a bearer code. ADR-0060 *Revisit when* | MEDIUM |
| **Verifier-only tokens**: the server stores the SHA-256 of a capability token, or a JWT secret with issuer, audience and clock skew | `websocket-auth/src/lib.rs:26-58` | entrance keys are sealed reversibly so the owner can show one again, and each request unseals and compares every key (`orch/src/worker_keys.rs:51`, `client_keys.rs:282`) | Keep reversible sealing (the owner reads the key back). If key counts grow, add a keyed-hash index for an O(1) lookup | MEDIUM |

## What Remit already does better

- **DNS pinning.** The Gate resolves once, checks every address with `routable`, and dials that address
  (`egress/src/egress.rs:287-329`). Codex says DNS rebinding "is hard to fully prevent" in its proxy
  (`network-proxy/README.md:277-279`). **HIGH**
- **Sealing per value with the slot as AAD and the key id on every value.** A sealed value moved to another slot does not
  open, and a missing key is named (`secrets/src/secrets.rs:240-269`, `orch/src/entrance_keys.rs` header). Codex seals a
  whole file with age under a keyring passphrase (`codex-rs/secrets/src/local.rs:14-19,41-44`). **HIGH**
- **No `unsafe`.** Every Remit rail runs under `forbid(unsafe_code)` (`jail/README.md:3-4`). Codex's hardening and
  sandbox code calls libc directly (`process-hardening/src/lib.rs:46,85,109,127`, `linux-sandbox/src/seccomp.rs:132`). **HIGH**
- **No unsandboxed fallback.** A session that cannot be isolated is not started (`claudecode/src/conn.rs:499-502`). Codex has
  `SandboxablePreference::{Auto,Require,Forbid}` and approval paths that rerun outside the sandbox (`sandboxing/src/manager.rs:43-47`).
  This follows from "no agent lifts a rail". **HIGH** (Remit), **MEDIUM** (how Codex uses `Forbid`)
- **Limits are required.** A Spec without a deadline, output cap, memory or CPU limit is refused (`jail/src/spec.rs:90-105`).
  **HIGH** (Remit). I did not find an equivalent required field in Codex's `SandboxExecRequest`. **LOW**
- **Bounded, sanitised refusals.** At most 64 distinct targets, each at most 256 bytes and printable
  (`egress/src/denied.rs:4-9,74-90`). Codex caps its output snippets at 512 chars (`sandboxing/src/violation.rs:9`), which
  is on par.
- **The credential is scrubbed from answers.** The Injector removes its credential from response bodies and never passes
  `Set-Cookie` or `WWW-Authenticate` on (`egress/src/inject.rs:7-9`). I did not check whether Codex's MITM does the same.
  **LOW** for Codex.

## Not worth taking

- The Windows sandbox (no Windows adapter has been asked for, ADR-0021 *Revisit when*). Landlock (Codex treats it as
  legacy). The execpolicy Starlark language (Remit's judgement belongs to agents. A command allowlist is the Claude Code
  permission layer, chain-runtime's area). The full SOCKS5 and UDP proxy (no tool needs it yet).
- Codex's crates as dependencies: they pin forks (rama) and Codex-specific types (`codex_protocol`). Copy the `.sbpl`
  text with its Apache-2.0 notice, and learn the rest.

## Next steps and owners

1. D1: rails-and-sandbox. Ancestor-unlink denies in both macOS profiles, plus a `mv`-parent case in `jail/tests/jail.rs`.
2. D3(b): rails-and-sandbox. `read_only` inside `dir` stays read-only. D3(a): review-and-ship, a scratch `work_dir` in
   `command_check`.
3. D2: rails-and-sandbox. First run the falsification command on the pod, then add a pid namespace and `/proc` to the Wrap.
4. Ideas 4-7 and 14 wait for the owner's request for hardening. Their *Revisit when* lines go into ADR-0021 (architect).
