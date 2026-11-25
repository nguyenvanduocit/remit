# plugin-sdk

What a Remit plugin is written with, and the contract every runner of one keeps (ADR-0072 decision 7).

- `Plugin`: the one async trait (`describe`, `call`, `poll`, `sense` as a stream of `Sensed`, `process`, `asset`,
  `account`, and `grant_forge`). A plugin that ships with Remit implements it and is called in-process; an applied
  extension is a child process Remit calls through `plugin::RemotePlugin`, which implements the same trait.
- `Definition`: a plugin written as data, its handlers typed functions (`Tool::new::<In>()`, `SensorKind::new`,
  `Processor::new`, `Poll`, `Account`), input schemas from `schemars` in one shape (no meta-schema or title, subschemas
  inline, objects closed). `validate` and `manifest` read it without calling it.
- `serve(impl Plugin)`: the child's `main`, answering `wire::plugin` over `link` on stdin and stdout. Stdout is the link, so
  a plugin logs to stderr.
- `pipe`: a `link::Conn` driven over one byte pipe each way, shared by `serve` and `RemotePlugin` (`link` has no driver of
  its own for a pipe).

The wire form is `wire::plugin`; its number is `PLUGIN_CONTRACT`, sent as the `hello`'s bundle.
