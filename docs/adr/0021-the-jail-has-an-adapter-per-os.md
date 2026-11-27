# 0021. The jail has an adapter per OS: namespaces on Linux, Seatbelt on macOS

Status: superseded by 0138; accepted, 2026-10-30. Source: the owner's request of 2026-10-30 (run Remit on a Mac; "the app runs on both"),
ADR-0018 ("Where the executor runs"), ADR-0004.

## Context

`internal/jail` isolates the two things ADR-0018 runs: an agent session (`HiddenWith`: Remit's secret paths hidden, the server's
binary and config read-only) and a source's program (`Run`: its work directory and the system read-only, no network but a unix socket
to an egress gate, a deadline, CPU and memory limits). It did so with unprivileged Linux user, mount, network and pid namespaces, so
the binary did not build on macOS (`syscall.CLONE_NEWUSER` is Linux's), and Remit could not run there at all.

macOS has no namespaces. What it has without privilege is Seatbelt, the kernel's sandbox, which `/usr/bin/sandbox-exec` applies to a
program and everything it starts, from a profile of allow and deny rules over files, network, signals, sysctls and services. Chrome
and other agent tools isolate their children with it. Measured on macOS 26.5 (arm64) before deciding:

- A deny on a path refuses reading it, writing, renaming, deleting and hard-linking it, through a link too (rules match the real
  path: `/tmp` is `/private/tmp`), and holds in every child. A later rule wins over an earlier one.
- A `deny default` profile importing Apple's `bsd.sb` runs a shell, Go, Python and curl, and refuses every TCP connection, every
  host path not allowed, a signal to a process outside the sandbox, and starting a setuid program.
- Connecting to a unix socket is a network operation: a file rule on its directory does not refuse it.
- A sandboxed process cannot apply another profile.
- macOS refuses to lower `RLIMIT_DATA`, `RLIMIT_AS` and `RLIMIT_RSS` (`EINVAL`), and allocates 2 GiB afterwards.
- `RLIMIT_CPU` sends `SIGXCPU` and nothing more: a program that keeps the signal's default ends, Go's runtime ignores it and runs on.
- Any process of the same user reads another's environment through `kern.procargs2` (a platform binary's excepted); no Seatbelt rule
  on sysctl names or process info refuses it, and refusing every sysctl stops the Go runtime from starting.

## Options

1. **Run Remit in a Linux VM on the Mac.** The rails stay as they are; the Mac is not a host Remit runs on.
2. **Build without the jail on macOS.** Sessions read the key file and the database: refused, it lifts the rail.
3. **One jail API, an adapter per OS chosen at build time** (`*_linux.go`, `*_darwin.go`), callers unchanged. **Chosen**, as the
   owner asked for the app on both and Seatbelt holds every property the jail's callers rely on but those listed below.

## Decision

- **`internal/jail` keeps one API** (`Spec`, `Run`, `Mask`, `HiddenWith`, `Wrap.Start`, `Init`, `SystemPaths`, `CAPaths`) and the
  shared code (validation, the helper's plan and status pipes, the relay, the proxy variables, `lookPath`). Each OS supplies
  `prepare`, `Wrap.start`, `self`, `helperAttr` and its `plan`. No caller knows which OS it runs on.
- **macOS (`sandbox_darwin.go`, `init_darwin.go`):**
  - A session runs under `sandbox-exec` with `(allow default)` and denies: reading, writing or connecting to a hidden path, writing a
    read-only one. A hidden path refuses to be read (Linux shows it empty).
  - A program starts through the helper, which sets `RLIMIT_CPU` and `RLIMIT_CORE`, finds the program among the paths the profile
    shows, and becomes `sandbox-exec` with `(deny default)`, `bsd.sb`, and: start programs from and read the readable paths, write
    the work directory, signal the same sandbox, connect to the gate's socket and its relay; hidden paths denied last.
  - The relay listens on the host's loopback at a port of its own per run, closed when the program ends, as macOS gives a program
    no loopback of its own.
  - Every path in a profile is its real path, a path that does not exist yet its nearest parent's; a path is quoted so that it
    cannot add a rule.
  - Seatbelt matches a rule by path, so renaming a directory above a hidden or read-only path would carry it past its rule
    (2026-11-11: a Wrap read a hidden key after `mv` of its parent). Both profiles end with a `file-write-unlink` deny on every
    directory above a hidden or read-only path (in a jail, those inside a writable path), as Codex's Seatbelt does.
- **On Linux the same directories are pinned** (2026-11-11): each is bound over itself before the hiding and read-only mounts,
  so it is a mount point, which the kernel refuses to rename or remove (`EBUSY`). Before, a mount moved with its directory and
  the old name was free: a Wrap ran `mv data moved` and planted a fresh `data/locked/kept.txt` where the read-only one had been.
  A directory inside a hidden or read-only path is not pinned: a read-only mount refuses the rename already, and a writable
  pin there would undo it.
- **A Wrap on Linux does not need a pid namespace to keep `/proc/<pid>/root` closed**: its user namespace does, as the kernel
  lets a process follow another's root only from the same user namespace or with `CAP_SYS_PTRACE` over it (measured
  2026-11-11 on OrbStack, from inside a user namespace and from outside).
- **A read-only path inside a writable one stays read-only** on both OSes (a `file-write*` deny after the writable paths'
  allow; on Linux the read-only bind mounted over the writable one, and at the same path it wins). A key result's command check
  writes its worktree but not the worktree's `.git`, from which the server's git runs hooks and `core.fsmonitor`.
- **A jail writes its work directory and the `Spec::writable` paths** (2026-11-11): a command check runs in the worktree with
  a scratch directory of its own as HOME and TMPDIR, which it could not write before, so a check whose tool writes HOME failed.
- **A writing session cannot change what the server's git runs** (2026-11-11, from Codex keeping `.git` read-only in every
  writable root). Sessions commit in their worktrees themselves (`submit.rs` commits only what they left), so `.git` as a
  whole cannot be read-only: the objects, refs and the worktree's index and HEAD stay writable. Every session's Mask lists
  read-only, for every clone Remit holds (`orch::policy::controls`): the clone's `config` and `hooks`, and per worktree
  the registry's `commondir` and `gitdir` and the worktree's `.git` file. A session reached any of them before, so a hook or
  `core.fsmonitor` it planted ran at the server's next git, outside the Wrap, able to read every hidden file.
- **The jail owns the proxy variables** (`HTTPS_PROXY` and the others): only it knows where its relay listens. `jail.ProxyPort` and
  the runner's `withProxy` are gone. **`Network` and `Loopback` are gone:** no caller used them, and macOS could not give a jail a
  loopback of its own.
- **What the system is belongs to the jail** (`SystemPaths`, `CAPaths`), per OS; a source's PATH (`source.ProgramPath`) is per OS,
  with Homebrew's on macOS, and is the one PATH of every program a source runs.

## Consequences

What the jail does less on macOS than on Linux, each measured:

- **No memory limit.** `MemoryBytes` is still required and applied on Linux; on macOS the deadline and the CPU limit bound a program.
- **The CPU limit ends only a program that keeps `SIGXCPU`'s default** (a shell, Python, a C program); one that ignores it, as Go's
  runtime does, runs until the deadline.
- **A child that starts a session of its own (`setsid`) survives the deadline**: there is no pid namespace to end with the program.
- **The relay's port is on the host's loopback** for the run: another process on the machine may connect to it, and reach what the
  gate allows (or, for a script, what the injecting proxy reaches with the connection's credential) until the program ends.
- **A session reads the environment of the server and of every other process of the user** (`kern.procargs2`). On macOS the
  server's environment must hold no secret: credentials go in the owner's inbox, `-source-env` or the vault's env files, which a
  session cannot read; and the server is not started from a shell that holds tokens. The server checks it at start on macOS: it warns with the names (never
  the values) in its environment that look like credentials (`cmd/remit/envcheck.go`, `childenv.SecretNames`); it goes by the
  name, so a credential named otherwise (a connection URI) is not found.
- **A session cannot use a sandbox of its own** (Claude Code's sandbox mode): a sandboxed process applies no other profile.
- `bsd.sb` lets a program read any path's metadata (its existence, size and times, not its content), the system's libraries, and
  the system's trust and preference services.
- `sandbox-exec` is marked deprecated by Apple and still ships and works on macOS 26.

- **Threats** (STRIDE over the boundaries a program and a session cross on macOS): *Tampering*: a session cannot change the
  server's binary, checkout or config (deny `file-write*`, renames included); a program writes only its work directory.
  *Information disclosure*: the key file, database, env files and process manager's state cannot be read, nor its daemon's socket
  connected to (`network-outbound` deny on hidden paths); a program sees no host path but the system's and its own; a path with a
  quote cannot write a rule into the profile; residual: the server's environment (above). *Elevation of privilege*: a setuid
  program does not start, a profile cannot be replaced, a program cannot signal outside its sandbox. *Denial of service*: deadline,
  output cap and CPU limit; residuals: memory, a CPU-limit-ignoring program until the deadline, a `setsid` child.
  *Spoofing / repudiation*: unchanged from ADR-0018; the gate records what it refused.

Enforced by: TestASessionCannotReadWhatIsHiddenFromIt, TestASocketInAHiddenDirectoryIsOutOfReach, TestAReadOnlyPathCanBeReadAndNotWrittenRenamedOrDeleted, TestAJailedProgramCannotReachTheHostsNetwork, TestTheSocketIsTheOnlyWayOut, TestAHiddenPathCannotBeReadWhileItsNeighbourCan, TestAJailedProgramWritesOnlyInItsDir, TestAJailedProgramCannotSignalTheHostsProcesses, TestAJailedProgramHasNoWayToLiftTheJail, TestCPULimitEndsASpinningProgram, TestASocketSetsTheProxyVariablesInPlaceOfTheSpecs, TestTheRelayClosesWithTheJail, TestAPathCannotWriteARuleIntoTheProfile, TestAPathIsHiddenThroughALinkAndBeforeItExists, TestASessionCannotChangeTheServersFilesNorReachItsProcessManager, TestMacOSWarnsWhichCredentialsTheServerWasStartedWith

## Revisit when

- Apple removes `sandbox-exec`, or macOS gains a way without privilege to refuse a process another's environment.
- Agents get an OS user of their own (ADR-0004), which closes the environment residual on both OSes.
- A third OS is asked for: it is a third adapter behind the same API.
- The owner asks to harden sessions against what the server runs on their behalf. The server's git still reads files a
  session can write: the user's `~/.gitconfig` and `~/.config/git` (git reads them for every repository), `~/.ssh/config`
  (`ProxyCommand` at a fetch or push), and a program of the same name planted earlier on the PATH (`/opt/homebrew/bin` is
  the user's). On Linux a path that does not exist when a session starts is not protected: a worktree another chain opens
  later, and a `config.worktree`. The structural fix replaces the list: the server runs git for a clone inside the same Wrap
  as the sessions (`repo::repo_command`), so whatever git runs reads nothing hidden.
- The owner asks for a sweep of escape classes on macOS: a rename that swaps two names (`renamex_np` with `RENAME_SWAP`,
  `exchangedata`) is not tested against the ancestor rule; `jail/tests/jail.rs` tests a plain rename.
