# procgroup

Starts child processes: as process groups, so ending a command ends everything it started, or detached, so ending the
server does not end it. `command` makes a
`tokio::process::Command` that leads its own group (`process_group(0)`, safe std); a `Child` whose future or value is dropped
before the leader is reaped kills the whole group with SIGKILL (`rustix::process::kill_process_group`). Cancelling a command is the caller
dropping its future (ADR-0072, 4): a deadline is `tokio::time::timeout`. `Command::output` waits at most `WAIT_DELAY` for
pipes held by processes the command left behind.

`command` is the one place a child process starts: the root `clippy.toml` (and each pure crate's copy) disallows
`std::process::Command::new` and `tokio::process::Command::new` everywhere else, so a second way to start a child does not
compile. A call that owns its start (the jail's helper, the AI runtime, a plugin with an empty environment) says why at its
line with `#[expect(clippy::disallowed_methods, reason = "...")]`; a test says so at the top of its file. A child's environment
comes from `childenv::of`, never the server's whole.

`detach` starts a program that must outlive the server, as `remit update` restarting it does (ADR-0087, *Installing*). An
intermediate `/bin/sh`, its own process group through `command`, starts the program in the background with no input, its
output appended to a log, prints its pid and exits: the program is reparented to init and is in neither the server's process
group (what launchd kills without `AbandonProcessGroup`) nor its tree by parent pid (what pm2's `treekill` kills), and a fork
clears any parent-death signal. It shares the server's session, and a kill of the whole control group (systemd's
`KillMode=control-group`; `deploy/remit.service` sets `KillMode=process`) still ends it. The environment is the caller's
`childenv::of` list, whole.
