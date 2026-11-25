# 0034. An extension is a go-plugin process a chain or a customer supplies and the owner applies

Status: accepted, 2026-11-02; the Go host and `sdk` superseded by ADR-0072 at its cutover; an MCP server is an extension of its own form by ADR-0077; its waits on the owner, but a secret act, are an agent's (ADR-0087). Replaces the program form of ADR-0029 (script, argv rules, egress list, jail); the API behind the injecting proxy
stays as ADR-0029 wrote it. Source: the owner's decisions of 2026-11-02 (`hashicorp/go-plugin`, "Anything-Plugin", full rights, federated
frontend, security after it works), `docs/remit-design.md` (K5), ADR-0018 (one gate).
Extended by ADR-0049: the contract also carries `Sense` (a plugin's kind of sensor, a stream of readings) and `Process` (a plugin's processor, what a firing does), and
`Describe` declares them with the subscriptions a bundled plugin ships. A plugin may be a program and an API behind the injecting proxy at once: `Describe` carries
`Auth` (base, scheme, variables, an address variable, headers) beside the tools, so an agent calls the tools and a script reaches the same API with the credential added.
A plugin that senses or processes through a connection is lent that connection's variables through the one gate (`Set.Lend`, caller `sensor` or `processor`).

## Context

ADR-0029 made an extension a script that runs in a jail with an argv allow-list. That bounds a plugin by what the engine can describe in a
manifest, so every richer extension (an agent, a signal, a judge, a screen) needs a new manifest field first. The owner's direction is the
opposite: users are engineers who do not want to build everything, and Remit changes itself through its own plugins, so a plugin can be
anything. The jail existed to bound a program; with the aim "make it work first, then make it right" (`AGENTS.md`) the bound is a seam, not a
control (below).

## Options

1. Keep the script form and grow its manifest per capability. Every new kind of extension is an engine release.
2. Go's stdlib `plugin` package. Host and plugin need the identical toolchain and dependency build; impossible for a closed-source engine.
3. `hashicorp/go-plugin` v1.8.0: the plugin is a child process that speaks gRPC over a unix socket. **Chosen.** Any language with a gRPC
   runtime works; Go authors get `sdk`.

## Decision

- **Contract.** `sdk/pluginpb/plugin.proto` is the one definition. Service `Plugin` has `Describe` (label, capability, credential
  variables, tools with input schema and effect, whether it polls, its frontend), `Call` (a tool, with the connection's variables for that
  call), `Poll` (query, cursor, limit, variables) and `Asset` (a file of the plugin's embedded frontend). `sdk` is the author API:
  `sdk.Plugin{...}`, `sdk.NewTool[In]` (the schema comes from the Go type), `sdk.Serve`.
- **Process.** `internal/plugin.Host` starts one process per extension id on first use, pinned by the SHA-256 of its file
  (`go-plugin` `SecureConfig`), with the host environment withheld (`SkipHostEnv`). A process that dies is started again by the next call;
  the failing call is not retried.
- **Manifest.** `source.Extension` carries what `Describe` answered, copied at proposal time: tools (`<id>_<tool>` on the agent's side), effect
  per tool, variables, `poll`, `ui` (entry and slots), or `auth` for an API behind the proxy. The owner reads it; the hash pins the file.
- **Gate.** A tool is an ordinary source tool: the effect the manifest declares ranks it on the ladder (read to irreversible), the chain's
  grant on the connection decides, the call is accounted for, and the output is cleaned of the connection's secrets. A plugin that polls is a
  source of events the same way (ADR-0024).
- **Road.** A chain whose playbook lists `extensions` calls `propose_extension` with the path of a built binary. The platform stages it,
  runs `Describe`, and raises a change item with the hash and what the plugin says it is. Only the owner's answer applies it: the binary is
  copied to `<ext>/<id>/<sha12>-<file>`, the hash is checked again, the manifest is stored and the source exists. `remove_extension` is the same
  road in reverse. A customer supplies the binary the same way; a chain that writes Go source for the platform to build is the second review
  path and is not built yet.
- **Bundled plugins.** A source of the core can be a plugin that ships with the server. `internal/bundled` holds them, written with `sdk`;
  the server starts its own binary as `remit plugin <id>` to run one, so it is a process like any other, over the same contract and
  through the same gate, with no file of its own to pin. Its manifest is read from the plugin in-process (`sdk.Describe`) and is never
  stored; a Chair cannot propose its id. Amplitude is the first. A source whose script form (`Inject`) agents use stays in the core until
  a plugin can carry it.
- **Enable and disable.** The owner can disable any installed plugin, bundled or applied, and enable it again: a record per id in
  `disabled_extensions` (who and when), so it outlasts a restart. A disabled plugin is not a source and its process ends; it stays
  installed. These are the owner's rail, not an agent's: a button on the Plugins page and an RPC pair (`extensionDisable`,
  `extensionEnable`) that the Assistant can pull on his words. Installing is asking the Assistant, as ever (`propose_extension`, then the
  inbox); the page's Install button starts that sentence.
- **Migrations 60 and 61.** 60 deletes the script extensions and the open proposals for them; the owner has the Chair propose each again as
  a plugin. 61 creates `disabled_extensions`.
- **Frontend.** `ui.entry` names a Module Federation remote embedded in the plugin file (`Asset`); `ui.slots` name where its components mount
  (slot, id, title, exposed module). The remotes are trusted, same-origin and share the app's one Vue instance (a spike confirmed this under
  `script-src 'self'`). The web side that loads them is the next change.

## Consequences

- A new kind of extension is a plugin, not an engine release: the contract grows by a message or a service, never by a per-kind manifest.
- A plugin is arbitrary code running as the server's OS user, with the credential of the connection it is called through. The owner reads
  the hash and the declared tools before it exists and decides.
- Credentials travel to the plugin per call, so a plugin keeps no state about a connection.

## Revisit when

Each item has its seam today and its control not built:

- **Confinement of the process** (filesystem, network, user): the launch is one function (`Host.use`); a runner goes there.
- **Host services** (a plugin starting chains, emitting events, using AI and threads): they come through the application layer with the
  plugin as the actor, over the go-plugin broker; the manifest gains `capabilities` and one policy function answers each request.
- **Build from source**: a chain proposes Go source and the platform builds it against `sdk`; `propose_extension` takes a source path then.
- **Declared effect versus real effect**: a manifest cannot be checked against what the plugin does.
- **Plugin signing and provenance**, **resource limits**, **protocol compatibility** (`sdk.ProtocolVersion` is the seam).

Enforced by: TestABundledPluginIsASourceFromTheStartAndItsToolRunsInAProcessOfTheServersBinary, TestTheOwnerDisablesAndEnablesAnExtension, TestADisabledExtensionIsKeptByIDAndEnablingForgetsIt, TestAPluginDescribesItselfToTheOwnerBeforeItIsApplied, TestACallReachesTheToolWithTheConnectionsVariables, TestAProgramThatIsNotTheOneAppliedDoesNotRun, TestAPluginThatDiedIsStartedAgainByTheNextCall, TestAChairProposesAPluginAndNothingIsInstalledUntilTheOwnerApplies, TestTheOwnersApplyInstallsThePluginPinnedAndTheSourceExists, TestAProposalTheServerCannotAcceptIsRefusedToTheChairAtOnce, TestAnAppliedPluginAnswersAnAgentWithTheGrant, TestAPolledPluginFeedsAnEventSignalAndItsFirstReadReplaysNothing, TestMigration60DropsScriptExtensionsAndKeepsTheRest.
