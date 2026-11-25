# jail

Runs a program where it can touch nothing of Remit (ADR-0018, ADR-0021, ADR-0069). No `unsafe` in the crate
(`forbid(unsafe_code)` from the workspace).

| Module | What it holds |
|---|---|
| `spec.rs` | `Spec`, `RunResult`, `Error`, `validate` |
| `run.rs` | `run`, `with_proxy`, the output cap |
| `wrap.rs`, `status.rs` | `Mask`, `hidden_with`, `Wrap::command`, `Command::spawn`; the helper's report |
| `relay.rs` | the relay of a TCP port to the jail's unix socket (tokio); the Linux listener is bound in `init_linux.rs` |
| `lookpath.rs` | finding a bare program name along the program's own PATH |
| `lib.rs`, `plan_{darwin,linux}.rs` | `HELPER_NAME`; the plan, the parent's prepare and stop |
| `init_darwin.rs`, `sandbox_darwin.rs`, `sweep_darwin.rs` | the macOS helper and sweeper, the Seatbelt profiles, ending a sandbox |
| `init_linux.rs`, `namespaces_linux.rs` | the Linux helper's two stages |
| `mounts_linux.rs`, `supervise_linux.rs` | the program's view of the files (the directories above a hidden or read-only path pinned as mount points, which cannot be renamed) and loopback; giving up privileges and running it |

How it is built, each choice forced by `forbid(unsafe_code)` or by Rust's `Command`:

- **The helper's plan and status.** Adopting an inherited descriptor needs `OwnedFd::from_raw_fd`, which is `unsafe`. The
  plan holds no secret once the environment is the helper's own, so it travels in the helper's argv; the helper reports
  on a unix socket in a fresh 0700 directory whose path is in argv. The jail's Seatbelt profile has one rule for it:
  connecting to that socket.
- **Linux namespaces.** Without `pre_exec` a child cannot be cloned straight into its namespaces, so a first stage
  (`remit-jail-init`) unshares them (`nix::sched::unshare`), maps the user, passes CAP_SETPCAP, CAP_NET_ADMIN and
  CAP_SYS_ADMIN on as ambient capabilities and starts the second (`remit-jail-namespaced`), the pid namespace's init. The
  first stage creates no thread: after `unshare(CLONE_NEWPID)` the kernel refuses one.
- **Loopback.** rustix's `ioctl` is `unsafe`, so the helper brings `lo` up with an RTM_NEWLINK request over route netlink.
- **Wrap's shape.** A Rust `Command` cannot have its program replaced or its stdio read back, so
  `Wrap::command(program, args)` returns the command (a `procgroup::Command` by deref) and `Command::spawn` starts it.
- **Stop.** Dropping the `procgroup::Child` is the stop on both OSes: on macOS the helper starts a sweeper in the
  sandbox, in its own process group, that ends the sandbox once the helper is gone. `run`'s deadline still sends SIGTERM
  to the helper first.

Crates: `rustix` (mount, pivot_root, capabilities, prctl, prlimit, statfs, netlink, kill), `nix` (only `unshare`:
rustix's safe `unshare` is deprecated as unsound for CLONE_FILES, which the jail never passes), `libproc` (the macOS
process table and start times), `signal-hook` (signals without a handler of our own), `tokio` (the relay, `run`).

Tests: `tests/jail.rs` has no harness, as the binary is the helper too and `main` calls `jail::init()` first. On Linux, run the musl test binary as a non-root user where unprivileged user namespaces are allowed
(in Docker: `--security-opt seccomp=unconfined --security-opt apparmor=unconfined --user 1000:1000`, an image with git).
From macOS: `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=rust-lld cargo build -p jail --test jail --target
aarch64-unknown-linux-musl`.

A Wrap on Linux keeps the host's /proc and has no pid namespace, yet a session cannot read a hidden path through another
process's `/proc/<pid>/root`: it runs in a user namespace of its own, and the kernel lets a process read another's root,
environment or memory only from the same user namespace or with CAP_SYS_PTRACE over the other's
(`cap_ptrace_access_check`); measured 2026-11-11 on OrbStack's kernel: `Permission denied` from inside, the file's content
from outside.

Revisit when (security seams not built): the status socket accepts its first connection without checking that the
peer is the helper (SO_PEERCRED / LOCAL_PEERPID would); a Linux Wrap has no pid namespace, so a process that left the
command's group outlives the stop; on macOS a rename that swaps two names (`RENAME_SWAP`, `exchangedata`) is not tested
against the deny on moving a directory above a hidden or read-only path (ADR-0021).
