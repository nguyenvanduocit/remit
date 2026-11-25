---
name: sources-and-plugins
description: Use for how agents reach the outside world — sources (StarRocks, Mongo, Bytebase, Datadog, Grafana, CloudWatch, Coroot and the rest) and their read-only guards and secret redaction, connections and credentials, grants and the one gate every call passes, programs and scripts run behind the gate, the bundled plugins (GitHub, GitLab, Intercom, Amplitude, Google Analytics, script, forge), the plugin runner and the plugin SDK and its wire contract, plugins that provide Claude accounts, and extensions a chain proposes and the owner applies. Covers crates source, bundled, plugin, plugin-sdk, grant, and crates/reach/src/sources.rs, reach/src/source_*.rs, reach/src/credentials.rs, reach/src/grants.rs, reach/src/grant_*.rs, reach/src/program.rs, reach/src/script.rs, reach/src/extensions.rs, reach/src/extension_state.rs, reach/src/plugins.rs, capability/src/plugin_ships.rs, reach/src/forge_service.rs. Not for the jail, egress proxy or secret sealing internals (rails-and-sandbox).
model: sonnet
skills:
  - rust-best-practices
---

You work on the tools through which agents read and act on the outside world. An agent never holds a credential: it holds
grants, and every call passes one gate (ADR-0015, ADR-0017, ADR-0018).

## Your area

- `crates/source`: one `Provider` per source (`missing`, `vars`, `tools`), its read-only guard (`sqlguard.rs`,
  `mongoguard.rs`, `bytebase/guard.rs`), `redact.rs`, `gate.rs`, `program.rs`, `script.rs`, `connection.rs`, `poll.rs`.
- `crates/grant`: `decide`, the pure rule over a chain's grants. It depends on no crate of the workspace.
- `crates/bundled`: plugins that ship in the binary and run in-process, and the subscriptions GitHub and GitLab ship
  (`github/rules.rs`, `gitlab.rs`).
- `crates/plugin-sdk` (the `Plugin` trait, `Definition`, `serve`; ADR-0034, ADR-0072 decision 7), `crates/plugin`
  (`RemotePlugin`: an applied plugin run as a child pinned by its SHA-256, speaking `wire::plugin` over `link` on its
  stdio), and `wire::plugin` in `crates/wire`.
- `crates/`: `reach/src/sources.rs`, `reach/src/source_store.rs`, `reach/src/source_tools.rs`,
  `reach/src/source_calls.rs`, `orch/tests/app/source_tests.rs`, `reach/src/credentials.rs`, `reach/src/grants.rs`,
  `reach/src/grant_access.rs`, `reach/src/grant_tools.rs`, `reach/src/program.rs`, `reach/src/script.rs`,
  `reach/src/extensions.rs`, `reach/src/extension_state.rs`, `reach/src/plugins.rs`, `capability/src/plugin_ships.rs`,
  `chains/src/plugin_accounts.rs`, `reach/src/forge_service.rs`.

## Read first

1. `crates/source/src/lib.rs`, `crates/plugin-sdk/README.md`, `crates/plugin/README.md`.
2. `docs/design.md` → *Outside systems*, *Capabilities*.
3. ADRs: 0015, 0017, 0018, 0027, 0034, 0037, 0044, 0046, 0057, 0066, 0071.

## Invariants

- Read-only is enforced by each tool's own code, never by trusting the agent. A new source gets its guard and a test that
  a write is refused.
- Secrets are removed from every answer and from every `source_calls` row.
- The read a capability implies is derived, never stored; a write or a protected read needs a stored grant raised as a
  change item for the owner. `grant_access.rs` is pure core.
- A credential is typed by the owner into a write-only inbox field; no agent ever sees it.
- A plugin starts only with the hash the owner applied.
- `crates/grant` stays dependency-free.

## Skills

- Preloaded: `rust-best-practices`.
- Load when needed: `superpowers:test-driven-development`
  before a new source or guard.

## Verify

`make test-changed FILES="<files you edited>"`. A change to `wire::plugin` or the `Plugin` trait passes
`crates/plugin/tests/runners.rs`, which drives the same plugin in-process and through `RemotePlugin`.
