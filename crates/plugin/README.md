# plugin

The runner of the plugins the owner applied (ADR-0034, ADR-0072 decision 7). `RemotePlugin` is one program, pinned by its
SHA-256 (checked before exec), run as a child process with the host environment withheld, speaking `link` on its stdio
(`plugin_sdk::pipe`): the plugin dials with a `hello` that offers `plugin` and asks for `forge.read`, and is admitted with
`forge.read` only after `grant_forge`. It implements `plugin_sdk::Plugin`, the trait a plugin that ships with Remit
implements in-process, so the gate's caller holds an `Arc<dyn Plugin>` either way.

The process starts on the first call and again on the first call after it died. A call whose session ended answers
`DISCONNECTED` and drops the process; it is not repeated, as a tool that ran half way may have acted. Any other failure is
the plugin's own. A sensor's readings arrive as pushes routed to its `Senses`; dropping the `Senses` sends `sense_close`, and
a process that dies ends every open sensor `DISCONNECTED`. `stop` closes the session (the plugin's stdin), waits 2 s for it
to exit and kills it then, and says which happened.

Limits: a start waits 15 s for the plugin's hello; `call` waits 10 min for its answer, every other call the link's 60 s; a
pipe record is at most 64 MiB (`plugin_sdk::pipe::MAX_RECORD`).
