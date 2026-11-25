# 0072. The backend and every client's core are Rust, cut over in one release on the same database

Status: accepted, 2026-11-10; decision 6's rule that documents keep Go's JSON and decision 4's rule that `replica`
depends on no `model` superseded by ADR-0107. Supersedes, at the cutover release, the mechanism of ADR-0034 (`hashicorp/go-plugin`, `plugin.proto`
and the Go `sdk`: a plugin is now a `link` peer, decision 7) and ADR-0058's transport (the runtime contract rides
the link below, not Socket.IO); every other record's decision stands and is carried over as behaviour. Source: the owner's goal of
2026-11-10, "rewrite the backend to rust", and his decisions of the same day: the browser's client side is a Rust core compiled to
WASM ("all frontend state is managed in the WASM, it interacts with Vue and Vue's state management, and it packages the
connection to the server"); the channels are the browser, iOS and macOS apps that wrap the web app in a web view, a CLI, a
daemon and more to come; with Rust on every side
socketrpc-gen is no longer followed; the transport is WebTransport through `moq-dev/web-transport` ("use it, do not care much
about the old; if TypeScript must be generated we write that ourselves"); the server is a general server where a UDP port can be
opened; the Rust backend runs on the development Mac first; and "do it in one shot, no rollback needed, no need to test too thoroughly". And the house rule in `~/projects/CLAUDE.md` (*Language Choice*: Rust
for new backend work; clippy `unwrap_used`, `expect_used`, `panic` = deny, `#![forbid(unsafe_code)]`, no `clone()` or
`Arc<Mutex>` added only to satisfy the borrow checker). Read against the working tree of 2026-11-10, while another session changes
the plugin protocol and accounts (ADR-0071). Proofs of concept named below ran on 2026-11-10 in `/tmp/remit-rust-poc/`
(coordinator's runs, re-checked by him).

## Context

The customer is the owner. What he gets is the same Remit, every chain, card and record he has today, run by a backend whose
compiler rules out the technical errors (null, ignored errors, data races, wrong shapes) that a Go backend written by agents can
still ship, and one client core that every channel he uses runs (the Vue shell in a browser or an app's web view, the CLI, the daemon), so a new channel is a shell,
not a second client. Product behaviour does not change; the domain is not redesigned.

What exists (line counts are non-test / test Go, measured this session):

- **Size.** About 95k lines of Go outside tests and 31k of tests. `internal/orch` 44101/17032 is the application layer and
  shell; `internal/source` 10118/3009; `internal/store` 5207/2597; `internal/bundled` 4025/1138; `internal/model` 2831;
  `cmd/remit` 1756; `internal/jail` 1639/1034; `internal/egress` 1606/768; generated: `internal/wire` 6910, `sdk/pluginpb` 2655,
  the runtime and worker wire and client packages about 1.2k to 1.5k each.
- **Process boundaries.** One binary, `remit`, runs as the core, as the AI runtime (`remit runtime`, its child, ADR-0058,
  `cmd/remit/main.go:52`), as each bundled plugin (`remit plugin <name>`, `internal/bundled/bundled.go:15`,
  `cmd/remit/main.go:59`), as `remit emit` and `remit export` (`main.go:45`, `:66`). The server also reads the bundled plugins
  in-process: their manifests (`internal/orch/plugins.go:67-68`, `extensions.go:332`), the signals and agents they ship
  (`plugin_ships.go:24-55`), their subscriptions (`plugin_pipeline.go:314`), the merge-request settings types
  (`forge_service.go:29-44`, `merge_request_sense.go:45-59`). The worker contract's client is TypeScript run by the Paseo plugin
  in Node (`paseo/server/worker/`). memoryd starts the owner's memory binary, a separate program (`internal/memoryd/daemon.go`);
  only its client is Remit's.
- **Import graph and fitness rules.** `internal/fitness/imports_test.go:13-71` is the allowlist of edges with a reason each;
  `TestThePureCoreDoesNoIO` (`imports_test.go:156`), `TestOnlyStepSetsAChainsStatus` (`writers_test.go:126`),
  `TestOnlyTheChainsWritersStoreAChain` (`writers_test.go:46`), `TestEveryCallThatChangesAnythingTakesARequest`
  (`rpc_test.go:76`, with its list of read-only calls and why, `contractReads`), `TestEveryActionOfTheOrchestratorChecksWhoAsks`
  and `TestTheClientDoorCallsNoMethodThatTakesAnActor` (`client_test.go:99`, `:169`), `TestNoBehaviourBranchesOnAChainsOrigin`
  (`origin_test.go:82`), `TestOnlyIngestWritesReadings` (`readings_test.go:42`), `TestOnlyTheRecordStageAddsFirings`
  (`firings_test.go:43`), `TestOnlySubscribeMakesASubscription` (`subscriptions_test.go:61`): eleven rules read from the AST.
- **Contracts.** `web/src/rpc/define.ts` (2,378 lines of types; `ServerFunctions` at `:2237` with 98 calls, `ClientFunctions` at
  `:2329` with 45 pushes), `web/src/rpc/runtime/define.ts` (core ↔ AI runtime, both ends Go) and `web/src/rpc/worker/define.ts`
  (core ↔ a person's device, its TS client copied into `paseo/server/worker`). `bunx socketrpc-gen@10.0.0 --server go` writes
  the Go servers and clients and the TS clients (`Makefile:11-20`); `make check-gen` diffs them (`Makefile:36`). The transport is
  Socket.IO v4 through the owner's `socketrpc-gen/go/socketio` (its README). The browser connects with `transports: ['websocket']`
  only (`web/src/rpc/index.ts:17`).
- **The guarantees the contract gives today**, partly from socketrpc-gen and Socket.IO:
  1. *Every call is answered or times out, with a typed error.* A call waits at most 60 s unless it asks for longer
     (`Makefile:10`, `--timeout 60000`). Transport codes `TIMEOUT`, `DISPOSED`, `DISCONNECTED`, `ABORTED`, `INTERNAL_ERROR`,
     `INVALID_ARGUMENT` (`internal/wire/types.generated.go:16-23`); domain codes `NOT_FOUND`, `ENDED`, `UNAVAILABLE`, `FAILED`
     (`define.ts:1883-1889`, `internal/orch/errkind.go`).
  2. *A changing call replayed after a reconnect acts once.* It carries a `Request` id (`act.ts:37-41`); the server does it once
     per id and remembers the answer seven days in `requests` (`internal/orch/request.go:20-60`, `:29`;
     `internal/store/migrations.go:165`); `internal/rpc/replay_test.go` covers each changing handler.
  3. *A connection's calls of one method are answered in arrival order* (a queue per event, `internal/wire/server.generated.go:56-110`).
  4. *A client that falls behind is never silently skipped*: the server waits until the connection has written what it was given
     (`internal/rpc/broadcast.go:280-297`); every (re)connect loads a snapshot and applies the pushes that came meanwhile by each
     record's revision (`web/src/sync.ts:1-60`, `rev.ts`).
  5. *Authentication and the bundle are checked when the connection opens*: the owner key, `bad token` final, `new bundle` sends
     the page to reload (`internal/rpc/rpc.go:66-99`). No action is asked while the socket is down (`act.ts:61-69`).
- **The browser's state.** No Pinia: plain Vue `reactive`/`shallowRef` in 107 view modules (5513 lines under `web/src/*.ts`):
  `catalog.ts`, `openChain.ts`, `assistant.ts`, `openEvent.ts`, `homes.ts` hold the replica ("nothing here is decided in the
  browser", `catalog.ts:4`); `sync.ts` applies snapshot and pushes; `act.ts` makes request ids and retries; `drafts.ts` holds the
  owner's typed words until the server has them (`drafts.ts:1-7`); `stream.ts` folds a card's events into blocks for display
  (`stream.ts:1-2`). 196 files under `web/src` import from `rpc/`. `web/verify/fakeClient.ts` fakes the socket client (35 lines).
- **Storage.** One SQLite file, WAL, `busy_timeout(5000)`, `foreign_keys(on)`, `_txlock=immediate`, one write connection and
  `NumCPU` read connections (`internal/store/store.go:121-137`), a `flock` on `<db>.lock` (`lock.go:20`);
  `Store.Update(ctx, func(Tx) error)` is the one write transaction (`store.go:74`), called from 75 places in orch. Records are
  JSON documents in `doc TEXT` columns. Migrations are SQL strings in order (`migrations.go:747`), some built by code
  (`migrations_ships.go:21`); a database holds how many it ran in `PRAGMA user_version` and the SHA-256 of each text in
  `schema_migrations`, and refuses a newer database or an edited migration (`migrate.go:18-99`). The highest is 96 (ADR-0071).
- **Plugins.** `hashicorp/go-plugin` v1.8.0, gRPC only, magic cookie `REMIT_PLUGIN`, `ProtocolVersion = 1`
  (`sdk/transport.go:12-21`), the binary pinned by SHA-256, host environment withheld, no AutoMTLS
  (`internal/plugin/host.go:118-147`); a process is dropped only on `Unavailable` (`host.go:107`); what Remit lends a plugin for
  one call is served on a `GRPCBroker` connection (`host.go:52-60`, `:251-258`; `sdk/forge.go:36-40`, ADR-0057). Service `Plugin`:
  `Describe`, `Call`, `Poll`, `Sense` (stream), `Process`, `Asset`; service `Forge` (`sdk/pluginpb/plugin.proto:9-28`); ADR-0071
  adds `Account`. The Go module is `remit` (`go.mod:1`), so `sdk` cannot be fetched by an outside author.
- **Agent tools.** 183 `mcp.AddTool(` sites outside tests; a Chair sees 138 tools, the Assistant 94; served over streamable HTTP
  (`internal/orch/tools.go:317`, `internal/clientmcp/handler.go:36`).
- **Concurrency.** Background work goes through `o.background` onto a `WaitGroup` with the work's context
  (`internal/orch/recover.go:327-335`); 26 `sync.Mutex`/`RWMutex` fields in orch; `tickMu` serialises the scheduler's Tick and is
  held while firings run (`orch.go:137`, `schedule.go:66`, `deliver.go:17`); every clock read goes through `o.now`
  (`orch.go:76`); seven orch test files use `testing/synctest`. An action runs to its end whatever happens to its channel
  (`request.go`, comment on `act`).
- **Platform code.** The Linux jail starts its program with clone flags and a one-uid map (`internal/jail/namespaces_linux.go:75-76`),
  re-execs the binary as its helper (`init_linux.go:15`) and makes 52 `syscall` calls (mount, bind, remount read-only, capset,
  prctl, prlimit, an ioctl to bring the loopback up). macOS runs the helper under `sandbox-exec` (`plan_darwin.go:4-14`).
- **Dependencies to replace** (`go.mod`): `modernc.org/sqlite`, `hashicorp/go-plugin` + grpc + protobuf, the MCP go-sdk,
  `google/cel-go` (subscription conditions, compiled and type-checked with a cost limit, `internal/signal/probe.go:28-42`,
  `condition.go:49-67`), `google/jsonschema-go` (a tool's input schema from its Go type, `sdk/sdk.go:68`), aws cloudwatchlogs,
  mysql (StarRocks), mongo v2 (with its own query parser, `internal/source/mongoquery.go`), x/oauth2 (Google Analytics).
- **Release and deployment.** One linux-amd64 binary with the web app embedded, built on `ubuntu-latest`
  (`.github/workflows/release.yml:2,16,45`); `scripts/update.sh` keeps `current` and `previous` releases as links
  (`update.sh:4-5,51-56`). The live binary listens on loopback behind a TCP hop, and the public URL answers HTTP/2 with no
  `alt-svc` (coordinator's measurement, 2026-11-10): today nothing carries UDP to the binary.
- **The tree moves.** Other sessions edit Go while this runs; on 2026-11-10 one is changing `sdk/`, `plugin.proto`,
  `internal/plugin/host.go`, accounts, `internal/agent`, `internal/airuntime` and both `define.ts` files (ADR-0071).

## Options and decisions

### 1. Cutover shape

1. **Strangle along process boundaries**: port the AI runtime, then the bundled plugins, then the core, shipping Go and Rust
   together in between. The boundaries that exist (runtime contract, plugin protocol) hold under 15% of the code; the core
   (orch, store, source, rpc: over 65k lines) has none inside it, so strangling it means new internal process or FFI seams that
   exist only to be removed, and every intermediate release ships two toolchains and two implementations of the request-once
   rule. Rejected: it is exactly "built beside the old to be removed later".
2. **Package by package through FFI** (Rust libraries called from Go over cgo). `unsafe` at every seam, cgo in the build, and the
   same scaffolding. Rejected.
3. **One cutover release, built in a Rust workspace beside the Go tree in the same repository and never shipped beside it
   (chosen).** No release serves the Rust tree until the cutover change, which deletes the Go tree, its generators and its tests
   in the same change. Nothing runs both in one deployment; no flag chooses.

The strongest counter-argument is the big-bang rewrite's record (Netscape; Spolsky's "single worst strategic mistake", in
`.claude/skills/aio-architect-reference/volume-03-cloud-design-patterns/25-strangler-fig.md:11`): the new system never quite
matches, the old keeps moving, and the new one is "almost ready" for years. The answers, as the owner set them:

- *It never matches* → the Rust server has to work on the existing data and the paths the owner uses, not to match Go test for
  test. It reads the same SQLite file (decision 6), and each crate carries the tests of the path it is used through (AGENTS.md:
  "a new feature carries only the tests of the path it is used through"); hardening tests wait until asked. What differs and
  matters is found while porting and in Milestone 1, and fixed forward.
- *The old keeps moving* → each unit is ported from the Go tree as it is on the day it is ported, and its crate's `README.md` names
  that Go commit (`ported-from:`). At cutover one pass ports `git diff <ported-from>..HEAD` over each unit's Go paths, repeated
  until it is empty. No freeze: Go keeps moving and product work goes on in Go until the cutover change lands.
- *Years* → the port is planned for agents in parallel (decision 11), each unit with its done-check.
- *No way back* → none is kept, by the owner's choice: the cutover deletes Go and goes forward; a defect found after it is fixed
  in Rust.

"We do it all at once" holds: what the owner receives is one release, whole; the Rust tree before it is work in progress, not a
second system in service.

### 2. The contract: one Rust crate, three modules

Settled by the owner (with Rust on every side socketrpc-gen is no longer followed). Considered:

1. **Keep `define.ts` and add Rust targets to socketrpc-gen.** The definition would be TypeScript whose only TypeScript reader is
   the Vue shell, while both ends are Rust; the generator, in another repository, would need a Rust server, a Rust client and a
   WASM binding. Lost: the source of truth would sit in the one language neither end is written in.
2. **Keep Socket.IO through `socketioxide`.** Proven: axum 0.8.9 with `socketioxide` 0.18.9 served the unmodified
   `client.generated.ts` and `reconnect.ts` through twelve cases (refusal, call, error envelope, array answer, push, replay once,
   invalid argument, timeout, `__rpc:error__`, reconnect, server restart). Lost: with no JS client left it needs a Socket.IO
   client for `wasm32` and native Rust, and Engine.IO's packet rules, for guarantees the link below gives directly; the
   PoC also found five behaviours Go has that `socketioxide` does not give for free (arrival order, `INVALID_ARGUMENT` on a decode
   failure, an answer on a handler's panic, one-tuple answers, a full 128-packet buffer dropping a push where Go waits).
3. **Protobuf for these contracts** (as the plugin contract is). No tagged unions with data (`oneof` only), and its JSON mapping is
   not the JSON the documents are stored in. Lost for these three; the plugin contract stays protobuf over gRPC (decision 6),
   because authors in any language read it.
4. **A Rust crate `wire` is the one definition, compiled into the server and every client core (chosen).** Its modules are the
   three contracts, each a capability set of the one protocol (decision 12): `wire::app` (browser and every owner channel),
   `wire::runtime` (core ↔ AI runtime), `wire::worker` (core ↔ a person's device), beside `wire::envelope` (frames and the error envelope) and `wire::contract` (the macro below). Payloads are
   serde JSON in the field names and shapes `define.ts` has today, so the Vue code reads the same shapes and the stored documents
   and the wire share their rules.

**How a call is expressed.** Two shapes were weighed:

- *One request enum and one answer enum per contract, dispatched by an exhaustive `match`*: the compiler catches a call with no
  handler, but the pairing of a call with its answer type is lost (any answer variant type-checks for any call), and the client
  stub must match on the answer at runtime.
- *A trait per contract with one method per call, written by a declarative `contract!` macro (chosen).* One list per contract,
  read like `define.ts`:

  ```rust
  contract! { app;
      read  chain_get(id: ChainId) -> ChainView, why = "reads one chain's view";
      change chain_pause(id: ChainId, why: String) -> Ack;
      change health_check() -> Health, timeout = 300s;
      push chain_updated(Chain);
  }
  ```

  It writes: the `Call` enum carried in a frame (serde-tagged by method), a `Handler` trait with one typed method per call, a
  `dispatch` whose `match` over `Call` is exhaustive and serialises each method's own answer type, a typed client stub per call
  in the client core, the `Push` enum, the constant list of calls with their kind, and the description our TypeScript generator
  reads. The exhaustive `match` lives inside the macro output, so both properties hold: no unhandled call, no mismatched answer.

**"Every call that changes anything takes a Request" becomes a type fact.** The macro has two arms. `change` puts
`request: Request` first in the handler's signature and the client stub, so a changing call without one cannot be written; the
client core fills it (a new id per action, the same id on a retry). `read` takes no `Request` and must carry `why = "..."`, which is
`contractReads`' reason moved to where the call is declared. The rpc adapter reaches the orchestrator's actions only through
`act`/`once`, which take a `Request` by type. What the compiler cannot decide is whether a call marked `read` really changes
nothing; that rests on the `why` its author writes and the reviewer reads. The replay test iterates the macro's list of `change` calls and fails for one without its
replay case.

**TypeScript for the Vue shell: our own generator** (`crates/tsgen`, a binary over `syn`), the owner's call. It reads the `wire`
crate's source: the `contract!` lists and every type they reach, with a closed set of serde attributes (`rename`, `rename_all`,
`tag`, `content`, `untagged`, `skip_serializing_if`, `default`, `flatten`); an attribute or a type shape outside the set fails the
generator, so an unsupported shape cannot pass silently. It emits `web/src/core/types.generated.ts` (every type of `wire::app`
the Vue shell reads, internally tagged enums as discriminated unions, `Option` as optional) and `web/src/core/api.generated.ts`
(the core's API: one async function per call with its typed arguments and answer, minus the `Request`, which the core adds; the
patch vocabulary and its store names, decision 10). Drift is caught three ways: `make check-gen` regenerates into a scratch
directory and diffs; `vue-tsc` runs over the shell in `make test`; and a Rust test writes one sample of every emitted type to JSON,
which a vitest checks against the generated declarations' field lists. Third-party generators (`tsify`, `ts-rs`, `specta`) were
not chosen for their own sake: each brings its own attribute dialect and its own wasm-bindgen coupling, and the generator needed is
small because the contract's shapes are few and closed.

**What happens to each file at cutover.** Deleted: `web/src/rpc/define.ts`, `client.generated.ts`, `types.generated.ts`,
`reconnect.ts`, `index.ts`, `reconnect.test.ts`; `web/src/rpc/runtime/define.ts`; `web/src/rpc/worker/{define,client.generated,types.generated}.ts`
and their copies in `paseo/server/worker/`; `internal/wire`, `runtimewire`, `runtimeclient`, `workerwire`, `workerclient`; the
socketrpc-gen lines of the Makefile (`gen_browser`, `gen_runtime`, `gen_worker` and their `check-gen` diffs); `socket.io-client`
from `web/package.json`. `make gen` runs `tsgen` and `buf generate`; `make check-gen` diffs both. The 196 files that import
`rpc/` move to `web/src/core/` in the same cutover. socketrpc-gen stays the owner's tool, unused by Remit.

### 3. Transport: WebTransport, with the five guarantees ours

Considered:

1. **Socket.IO** (option 2 above). Lost.
2. **WebSocket with the link's own frames** (axum server, `tokio-tungstenite` natively, `web-sys` in the browser). Works through
   the TCP hop and every browser today, but one ordered byte stream per connection: one chain's streamed turn and a snapshot sit in
   front of every other push. Not chosen; it is the designed second driver (*Revisit when*).
3. **WebTransport through `moq-dev/web-transport` (chosen, the owner's decision).** `web-transport` 0.13.9 is one API over
   `web-transport-quinn` natively and the browser's WebTransport in wasm (`Client`, `ClientBuilder::with_server_certificate_hashes`,
   `with_system_roots`; `Session::open_bi`, `accept_bi`, `open_uni`, `accept_uni`, datagrams, `close`, `closed`; a native-only
   `Server`; docs.rs read 2026-11-10, MEDIUM). Independent streams remove head-of-line blocking between calls and between chains.

**Streams.** One WebTransport session per pair of peers, at one path, `/link` (decision 12):

- **Control stream**: the first bidirectional stream, opened by the dialling peer. `hello {node, credential, capabilities,
  bundle}` → `welcome {node, capabilities}` or the session closed with the refusal; then `ping`/`pong` both ways, and `behind`
  from the pushing side (guarantee 4). Until `welcome` the accepting peer opens no stream, accepts no other stream and reads
  nothing else.
- **One bidirectional stream per call**, opened by whichever peer calls (the core calls the AI runtime, the runtime calls the core,
  as their contract already runs both ways, `web/src/rpc/runtime/define.ts:184,203`): the caller writes one `call {seq, method,
  args, home?}` frame and finishes its side; the callee writes one `ok {value}` or `err {code, message}` frame and finishes. A
  timeout resets the stream.
- **One server-opened unidirectional stream per home** (a chain's id or the Assistant's) for the pushes whose record has that home
  (cards, events, deltas, drafts), and one **feed** stream for every other push. Within a stream, pushes keep their order; across
  streams no order is needed, because each record's revision decides (the rule `sync.ts` applies today) and homes are independent.
  The set of pushes a session receives is today's; only the stream they ride changes.
- Frames are length-prefixed JSON (`wire::envelope`); datagrams are not used.

**The five guarantees, as requirements with a test each** (tests named here are written with the `link` and `rpc` crates; the
cutover cannot land without them):

| # | Requirement | How it holds over streams | Test |
|---|---|---|---|
| 1 | Every call is answered or fails within its timeout, with a typed error | The client core arms a timer per call (60 s, or the call's `timeout =`); on expiry it resets the stream and fails the call `TIMEOUT`; a session lost fails every pending call `DISCONNECTED`; a call aborted by its caller is `ABORTED`, by a closed core `DISPOSED`. The server answers an undecodable frame `INVALID_ARGUMENT`, a handler whose task ended abnormally `INTERNAL_ERROR`, and a domain failure with its `ErrorKind`. One `ErrorCode` enum in `wire::envelope` holds these ten codes and decision 12's `FORBIDDEN` and `NOT_HERE` | `link::every_call_is_answered_or_fails_typed`: no answer, a late answer after the timeout (ignored), a session loss with three calls out, a malformed frame, a panicking handler, each domain code |
| 2 | A changing call replayed after a reconnect acts once | Unchanged on the server (`requests`); the client core keeps an unsettled action's `Request` and sends it again on retry (`retrying` of `act.ts`), only after `TIMEOUT` or `DISCONNECTED` | `rpc::a_changing_call_replayed_after_reconnect_acts_once`, for every `change` call of the macro's list: the session is closed after the server acted and before the answer, the client reconnects and retries, one effect, the first answer returned |
| 3 | A session's calls of one method are answered in arrival order | Each call frame carries `seq`, the client's order of sending; the server runs one session's calls of one method one after another in `seq` order, as Go's queue per event does, whatever order their streams arrive in | `link::calls_of_one_method_run_in_the_order_sent`: three calls of one method on streams delivered in reverse; handler invocations in `seq` order; calls of other methods not held |
| 4 | A client that falls behind is never silently skipped | A push is written in order on its stream or the session ends: when a stream's unsent bytes pass the bound or a write waits past the call timeout, the server sends `behind` on the control stream and closes the session; the client reconnects and re-reads its snapshot | `link::a_client_that_falls_behind_is_disconnected_and_resyncs`: a client that stops reading; the server never drops a push mid-stream; after reconnect the replica equals the server's |
| 5 | Authentication and the bundle are checked when the session opens | `hello` first; `bad token` closes with a final code (the shell shows the way in), `new bundle` with the reload code; any stream opened before `welcome` is refused | `rpc::a_session_is_admitted_only_by_hello`: a call before `hello`, a wrong key, a stale bundle, the dev bundle (none), a good key |

No action is asked while the session is down: the core refuses it locally with the words `act.ts` says today.

**What the transport needs, and what is unproven.** HTTP/3 over UDP with TLS terminated at the binary. In development the binary
makes a self-signed ECDSA P-256 certificate valid less than two weeks (MDN, WebTransport constructor: the limit, no RSA, and
`allowPooling` false; read 2026-11-10, HIGH), makes a new one before the old expires, and serves its hash to the page over the same
origin the page came from, which connects with `with_server_certificate_hashes`; the AI runtime gets the hash with its key on stdin (ADR-0058). In production a certificate for
the public name sits at the binary and a UDP port is open to it; the owner runs the server where one can be opened (his answer of
2026-11-10), but on the pod today the URL answers HTTP/2 with no `alt-svc` behind a TCP hop, so this is unverified there (P15).
Browsers: Chrome 97+, Firefox 114+, Safari desktop and iOS only from 26.4, about 92% of users (caniuse, read 2026-11-10, MEDIUM);
every Safari before 26.4 has no WebTransport. The crates are 0.x; the wasm crate needs `--cfg=web_sys_unstable_apis` in the final
build's `.cargo/config.toml`; its CI runs headless Chromium only, and "clean closure has not been reproduced through a real
WebTransport session" (its README). `qmux`, its stream layer over WebSocket, states no browser client. Node has no WebTransport
global, which decides the worker's shell (decision 10).

The transport is a trait in the sans-IO core (`link::Transport`: sessions, streams, frames), so a WebSocket driver carrying the
same frames with a stream id in each is a second implementation, and the contract, the guarantees' tests and every shell do not
change when it is added.

### 4. Workspace and crate map, and how the architecture rules hold

A Cargo workspace at the repository root (`Cargo.toml`, `crates/*`), `[workspace.lints]` setting `unsafe_code = "forbid"` and
clippy `unwrap_used`, `expect_used`, `panic` = deny, `await_holding_lock` = deny; every crate has `lints.workspace = true`, and
`clippy.toml` sets `allow-unwrap-in-tests`. One crate per Go package, named as the package:

| Crate | From | Kind |
|---|---|---|
| `model` | `internal/model` | leaf: records, serde |
| `grant`, `signal`, `outcomes`, `rollout`, `replay`, `library`, `skills`, `childenv`, `egress`, `provider`, `secrets`, `auth` | same names | pure core |
| `builtin`, `importer` | same | records and skills |
| `store` (+ feature `testing` for `storetest`) | `internal/store` | shell: SQLite |
| `procgroup`, `jail`, `claudecode`, `agent`, `forge`, `memoryd` | same | shell: processes |
| `source` | `internal/source` | shell: outside systems |
| `plugin-sdk` | `sdk` | author API: `serve(impl Plugin)` over `link`'s stdio transport |
| `plugin` | `internal/plugin` | host |
| `bundled` | `internal/bundled` | plugins, served as `remit plugin <name>` |
| `wire` | the three `define.ts` and `plugin.proto` | the contract: `app`, `runtime`, `worker`, `plugin`, `envelope`, `contract!` |
| `tsgen` | new | the TypeScript generator (a binary; build-time only) |
| `link` | new: the sans-IO session (client and server halves), the five guarantees, `Transport` | pure core, builds for wasm32 |
| `link-quic` | new: `Transport` over `web-transport-quinn` (server and native client) on tokio | shell |
| `replica` | `web/src/{sync,rev,homes}.ts` and the mutation side of `catalog`, `openChain`, `assistant`, `openEvent`, `archived`, `drafts`, `act`, `session` | client core, sans-IO, builds for wasm32 |
| `client-web` | new: the wasm-bindgen driver (`web-transport`'s wasm side, timers) and the exported API | binding, browser |
| `remit-client` | new: the CLI and daemon shell, and the Paseo worker's child (decision 10) | binary |
| `orch` | `internal/orch` | application layer and shell, one crate |
| `rpc`, `workerrpc`, `runtimelink`, `airuntime`, `door`, `clientmcp` | same | channels |
| `remit` (bin) | `cmd/remit` + `web` (the bundle embedded with `rust-embed`) | binary |

`orch` stays one crate with one module per Go file group: splitting it by domain would be the redesign this record does not do,
and its parts share `Orchestrator`. Its compile time is the cost (*Revisit when*).

The rules, by who enforces them:

- **Allowed edges: the compiler.** A crate can only name the crates in its `[dependencies]`, so a new edge is a line in a
  `Cargo.toml` that the diff shows and the reviewer reads.
- **The pure core does no I/O: dependencies and clippy.** A pure crate depends on no `tokio`, `rusqlite`, `reqwest` or driver,
  and its `clippy.toml` lists `std::fs`, `std::net`, `std::process`, `std::time::SystemTime::now`, `std::env` under
  `disallowed-methods`/`disallowed-types`; every pure crate's `clippy.toml` holds the same lines.
- **No `unsafe`: the compiler.** `forbid(unsafe_code)` everywhere, through `[lints] workspace = true` in every crate. An exception is
  written in that crate's `Cargo.toml` with its reason; the only candidate is the binding `client-web`, if P5 shows
  wasm-bindgen's generated code does not compile under `forbid` (then that crate alone is `deny`, holds no hand-written `unsafe`,
  and only macro output is allowed). The core it binds stays `forbid`.
- **Single writers and the rest: visibility and review.** Where visibility can carry a rule it does: a chain's status field is
  private to `model` behind a setter that only orch's `step` module calls, a store writer is reachable only from its owner's
  module, and a changing call takes a `Request` by the `contract!` arm (decision 2). What visibility cannot hold (one module of
  another crate, a branch on a chain's origin) the reviewer holds against AGENTS.md.
- **The client core decides nothing:** `replica` may depend only on `link` and `wire` (no `model`, `signal`, `library` or any rule
  crate) in its `Cargo.toml`, so it cannot become a second owner of a rule. (Superseded by ADR-0107 decision 1: `replica`
  depends on `link`, `wire` and `model`, whose records hold no rule that weighs one record against another; never on a rule
  crate.)

### 5. Async model and concurrency

1. **`async-std` or `smol`.** axum, tonic, rmcp, quinn, the AWS SDK and `mongodb` are tokio crates. Rejected.
2. **Threads only.** Every outside call blocks a thread; streams and timeouts by hand. Rejected.
3. **tokio, multi-threaded runtime, translated shape for shape (chosen).** No concurrency is redesigned:

- `o.background` + `WaitGroup` + `workCtx` → `tokio_util::task::TaskTracker` + a `CancellationToken`; `Shutdown` closes the
  tracker and waits, as today.
- `context.Context` as first parameter → the caller's cancellation is the future being dropped; deadlines are
  `tokio::time::timeout`; work that must outlive its caller takes the token explicitly.
- **An action runs to its end whatever happens to its channel.** In Go a closed socket only cancels a context; in Rust, dropping a
  handler's future cancels the work inside it. So `act` and `once` spawn the action on the tracker and await its handle: a dropped
  call leaves the action running to its answer, which `requests` records. A test drops the call mid-action and asserts the action
  finished once.
- A `sync.Mutex` held for a short section with no I/O → `std::sync::Mutex` (`await_holding_lock` denies holding it across an
  await). `tickMu`, held while firings run, → `tokio::sync::Mutex`. A goroutine that owns state and serves it over channels
  (`views.run`, `orch.go:188`) → a task that owns it, with `mpsc` in and `watch`/`broadcast` out. Shared state is `Arc<...>` where
  Go shares a pointer; no `Arc<Mutex>` or `clone()` is added where Go has no shared state.
- The clock stays one port (`o.now`). `testing/synctest` → `#[tokio::test(start_paused = true)]` with the clock port (P10).
- **The store's single writer.** One `rusqlite::Connection` for writes, owned by a dedicated thread; `Store::update(|tx| ...)`
  sends the closure to it and awaits the result over a oneshot. The closure is synchronous (`FnOnce(&Tx) -> Result<T> + Send`),
  so nothing awaits inside a transaction: an orch call that does outside I/O inside `Update` today is split into compute outside,
  write inside (the 75 call sites are read in unit U4.1). Reads use a fixed set of `NumCPU` connections opened `query_only`, on
  blocking threads. `BEGIN IMMEDIATE` for writes, as `_txlock=immediate`; the same `flock`.

### 6. Storage: the same file, the same schema, zero data migration

1. **Export and import into a fresh Rust schema.** A migration of the owner's data at cutover and no road back. Rejected.
2. **A new store (another engine or an ORM's schema).** Same, plus a redesign. Rejected.
3. **The Rust binary opens the same file and schema (chosen)**, proven by P3 on the dev database.

- `rusqlite` 0.41.1 with the `bundled` SQLite 3.54.1, the version modernc carries; the same pragmas, connection roles and
  `flock` as `store.go:121-137` and `lock.go:20`. The bundled build lacks `ENABLE_MATH_FUNCTIONS`; no store SQL uses one.
- The 96 migration texts (and any Go adds before cutover) keep their numbers and are generated from Go, not copied from the source
  files, because some are built by code (`migrations_ships.go:21`): a `go run` at the baseline writes
  `crates/store/migrations/NNNN.sql` from the `migrations` slice, compiled in with `include_str!`. The migrator is `migrate.go`'s
  algorithm (`user_version`, refuse a newer database, the SHA-256 of each text against `schema_migrations`, `INSERT OR IGNORE`
  of the sums), so a database at 96 opened by Rust runs nothing and older backups still open.
- Migrations after the cutover are appended in Rust, numbered on from the last Go one.
- Documents keep their JSON, encoded as Go encodes them (each rule measured by P3), so every stored document and sealed secret
  stays readable: serde field names are the Go `json` tags,
  `omitempty` → `skip_serializing_if` (`internal/model/model.go:43`); times in documents are RFC 3339 with the local offset and
  the fraction trimmed (`model.go:65`), in columns Unix milliseconds (`store.go:144`); a zero time is `0001-01-01T00:00:00Z` where
  the field has no `omitzero` (`model.go:329`); `<`, `>`, `&` are escaped as Go escapes them (`store.go:194`); a `Duration` is
  integer nanoseconds (`internal/model/signal.go:324`); map keys sorted; bytes as padded base64. Key order inside a stored
  document is not stable (revisions are counted in SQL with `json_set`, which appends, `store.go:210-213`), so no code compares or
  hashes document bytes. A revision is a compare-and-set on `$.rev` in SQL with `RETURNING` (`store.go:260`), ported as SQL.
  (The rule that documents keep Go's encoding is superseded by ADR-0107 decision 2: documents are written by `serde_json`,
  an absent value is an `Option` left out, and migrations 0109 and 0110 removed the empty enum texts, zero times and `null`
  lists earlier builds wrote; times keep their text and a `Duration` its integer nanoseconds.)
  Events: `by_kind = ''` is no actor (`events.go:38`), empty data is `NULL` (`events.go:41,66`), a packed event is raw DEFLATE in a
  BLOB (`pack.go:41,94`); old shapes are converted when read, as `scanEvent` does.
- Snapshot before migrating, export, restore and event packing are ported with the store.

### 7. Plugins

The plugin's port is already narrow: `source.Set` passes the one gate (`internal/source/lend.go:30-50`, `grant::decide` in
`crates/grant/src/lib.rs:171`) before it reaches `Plugins.Call` or `Plugins.Poll` (`internal/source/extension.go:324-330`), so how
a plugin runs sits behind the gate. go-plugin's stdout handshake, magic cookie, broker and `GRPCStdio` answer Go's need to spawn a
process for every plugin, bundled ones included (`internal/bundled/bundled.go:1-3`); the owner said plugins need not follow Go.

1. **Keep Go plugins and a Go host; only the core is Rust.** Two toolchains forever. Rejected.
2. **protobuf over gRPC with go-plugin's v1 handshake, host and SDK in Rust.** Proven (P2), but a second protocol and codec beside
   `link`, the broker and cookie carried over, and no plugin on another node. Its one gain, applied extensions running unchanged,
   protects nothing on the dev database (`extensions` = 0). Rejected.
3. **A WebAssembly component runner (wasmtime, WIT, WASI 0.3).** Confined by construction, the lent imports being the grant;
   but `script` runs host programs WASI cannot (`internal/bundled/script.go:200`), and `wasmtime-wasi-http` calls itself
   experimental. Kept as the confinement runner (*Revisit when*).
4. **A plugin is a `link` peer granted the `plugin` capability; the contract is a fourth module of `wire` (chosen).**
   `wire::plugin` holds `describe`, `call`, `poll`, `process`, `asset`, `account` as calls the host makes, `sense` as pushes the
   plugin makes under a sensor id, and `forge.standing` as a call the plugin makes when its `welcome` grants `forge.read`
   (ADR-0057's lend becomes a grant; no broker). The author API is one async trait, `Plugin`, in `crates/plugin-sdk`, and the
   host holds `Arc<dyn Plugin>` behind `source`'s plugin port, so every call has passed the gate before it reaches one: a bundled
   plugin is a direct implementation called in the server's process, with no codec between; an applied extension is
   `RemotePlugin`, the same trait over `link` on a child process's stdio (`serve(impl Plugin)` is the child's side), pinned by
   SHA-256, the host environment withheld, started again by the next call after it dies. `wire::plugin` is the remote form;
   schemas come from `schemars` in Go's shape. Protocol versions are `link`'s and `wire`'s. An extension built against the Go `sdk` stops
   working at cutover; the pod's `extensions` table is read, with the owner's go-ahead, before cutover.

### 8. Third-party replacements, and the PoCs that gate the port

Crate versions are those `cargo search` listed on 2026-11-10. Only a PoC whose failure would stop the server from working at all
gates the port: **P1** (CEL), **P5** (the WASM core over WebTransport, P15 folded in) and **P4** (the jail under
`forbid(unsafe_code)`). Every other row is **found while porting**: its unit meets it, takes the fallback named here if the first
choice fails, and says so in its `README.md`. Ranked by risk:

| # | Replaces | With | State | PoC, and what refutes it |
|---|---|---|---|---|
| P1 | `cel-go` (declared variables, type check that the output is bool or dyn, cost limit, interrupt, `ext.Strings`) | `cel-interpreter` 0.10 | **Gating; pending, high risk**: stored conditions are data, and the interpreter is not known to type-check or to bound cost | Evaluate every condition stored in a copy of the owner's database and every one in fixtures and tests, against recorded readings, in Go and Rust; run `Check` on ill-typed and costly expressions. Refuted if any result differs or an expression Go refuses is accepted. Fallbacks in order: our own checker over the interpreter's AST for the six declared variables; a binding to cel-cpp (`unsafe` stays in the dependency) |
| P5 | the TS RPC client | `client-web` over `link`, `replica`, `wire`, `web-transport` wasm, and `tsgen`'s output | **Gating; pending, high risk** | Build for `wasm32-unknown-unknown` (`rustup target add`, `wasm-bindgen-cli` matching the crate, `--cfg=web_sys_unstable_apis`), load it in the app against Milestone 1's server. Measure the gzipped wasm, the time from page load to first snapshot applied against today's on the same machine, and whether the crate compiles under `forbid(unsafe_code)`; `vue-tsc` passes over today's components with only import paths changed. Refuted if the first snapshot is more than 100 ms later on a cold load, or a type needs a hand edit |
| P15 | Socket.IO over the TCP hop | WebTransport over HTTP/3 to the binary | **Gating, with P5; pending** | On the development Mac: Chrome and Firefox reach the binary with a certificate hash; Safari 26.4+ with a trusted certificate. On the server: a UDP port open to the binary, TLS at the binary, `alt-svc` advertised, `curl --http3` from another machine gets `/healthz`, and the app opens a session through it. Refuted by any of these failing; then the WebSocket driver ships first (*Revisit when*) |
| P2 | go-plugin host + Go SDK | own host and SDK over `link` (decision 7) | **Found while porting** | The Go `testplugin` ported to the Rust SDK runs as a child over stdio: `describe`, `call`, six `sense` pushes, a `forge.standing` call under its grant, clean exit, no process left |
| P3 | `encoding/json` over stored documents | `serde_json` | **Done; confirmed on the dev database**: user_version 96, 96/96 checksums equal, the 96 texts run by rusqlite give the same 81 schema objects, chains 92/92, subscriptions 34/34, events 140/140 decode, 13,356 documents in 34 tables re-encode (13,355 byte for byte with a Go-compatible formatter), Go reads back what Rust wrote | Before the server runs on the pod: the same read on a backup of the pod's database, which holds packed BLOB events the dev database lacks |
| P4 | Linux jail under `forbid(unsafe_code)` | the existing helper re-exec, then `nix` (`unshare`, `mount`, `pivot_root`, `setrlimit`, `prctl`), `caps`, and netlink (`rtnetlink`) for the loopback | **Gating; pending, medium-high** | On linux/amd64 the ported `run_linux_test.go`, `hidden_linux_test.go`, `probe_linux_test.go`, `readonly_test.go` pass with no `unsafe` in the crate. Refuted by one call with no safe wrapper; then that call alone moves to a crate `jail-sys` whose `Cargo.toml` gives its reason |
| P6 | Socket.IO's acks, heartbeat, reconnect | `link` over `Transport` | **Found while porting** | The five guarantee tests of decision 3, first against an in-memory transport, then over `link-quic` on loopback. Refuted by any case behaving otherwise than `act.ts`, `sync.ts` and `reconnect.test.ts` assert today |
| P14 | the worker's TS client in Node | `remit-client` as the Paseo plugin's child, JSON lines on stdio | **Found while porting; confirmed short of Milestone 1**: Node has no WebTransport global, so the wasm core cannot open a session there. The plugin's own code starts the child, enters a code, accepts an offered help and hands it in against `workerrpc` wired as the binary wires it (`crates/remit-client/tests/worker/paseo.rs`, run with `--ignored`); against the binary the child finds the link, dials and is turned away with `bad key`. A code on Milestone 1's server is made by the Assistant, so the accept there waits for stage b | The Paseo plugin starts the child, a person's device connects to Milestone 1's server, accepts a card and hands it in. Refuted by a step that fails; fallback: a napi-rs addon of the same core (`unsafe` inside the binding crate, its reason in that crate's `Cargo.toml`) |
| P13 | (new) the iOS and macOS apps, which wrap the web app | the Vue shell and `client-web` in the system web view (WKWebView) | **Found while porting**: WebTransport there is the OS's WebKit, Safari and iOS Safari only from 26.4 (caniuse, MEDIUM) | A minimal wrapper app (a plain WKWebView first, then Tauri 2's) on the latest iOS and macOS release, the only versions the apps support (the owner, 2026-11-10), loads the app from Milestone 1's server, opens a WebTransport session with the certificate path the production server will use, applies a snapshot and a push, and makes one changing call twice with one request id. Refuted if the session does not open there; then the WebSocket driver (*Revisit when*) is built |
| P7 | MCP go-sdk streamable HTTP | `rmcp` 3.9.0 on axum | **Done; confirmed**: three real tools at `/mcp`, bearer auth as axum middleware, a per-card tool list by building a `ToolRouter` per request from its `Parts`; a real Claude Code session saw the same tools from Go and Rust | Binding on `orch` and `clientmcp`: a transform of about 60 lines over `schemars` output makes schemas byte-identical to Go's (drop `$schema`/`$defs`, `["null","array"]`, no `"default":""`), and arguments are parsed once at the door into the tool's closed (`deny_unknown_fields`) input type, the same type `schemars` derives the client's schema from, so the schema and the check cannot disagree and a bad argument never reaches the tool as loose JSON (as `plugin-sdk/src/sense.rs` does) |
| P8 | mongo v2 with `mongoquery.go`'s extended JSON | `mongodb` 3.10 + `bson` | Found while porting | The ported `mongoquery` and `mongo` tests, then a read-only query per stored Mongo connection on the dev copy. Refuted by a different document set |
| P9 | `CGO_ENABLED=0` static build | `x86_64-unknown-linux-musl` with bundled SQLite (C) and quinn's TLS | Found while porting | The release workflow builds the musl binary on `ubuntu-latest`, `ldd` says static, and it starts on the pod's OS copy. Refuted by a dynamic link or a start failure; then `x86_64-unknown-linux-gnu` against the pod's glibc |
| P10 | `testing/synctest` | `tokio` paused time + the clock port | Found while porting | The seven synctest files ported pass 50 runs in a row; SQLite on blocking threads does not let paused time jump mid-statement. Refuted by one hang or flake; then those tests use a manual clock with real time for the store |
| P11 | StarRocks over go-sql-driver | `mysql_async` 0.37 (text protocol `query`) | Found while porting | Ported `starrocks` tests and a read on the dev copy's connection |
| P12 | x/oauth2 JWT for GA, cloudwatchlogs, jsonschema-go | `jsonwebtoken` + `reqwest`; CloudWatch Logs' five actions as JSON 1.1 calls signed with `aws-sigv4` (the signer reproduces AWS's documented example signature; the SDK's retry is 3 attempts on throttling, 5xx or a broken connection, `crates/source/src/cloudwatch.rs`); `schemars` | Found while porting | Their ported tests; the GA reading on the dev copy; every bundled tool's schema equal to Go's |

### 9. What proves it works, and no rollback

1. **The Go behaviour as an oracle**: every Go test ported, differential runs of the pure core, the wire compared call by call, a
   rollback release kept and drilled. The owner declined it: "do it in one shot, no rollback needed, no need to test too
   thoroughly".
2. **Shadow production traffic.** The core acts on the world (it runs agents, merges MRs, posts comments): a shadow would act twice.
   Rejected.
3. **It works on the existing data and on the paths it is used through (chosen).**

- **Data.** The Rust server opens the owner's SQLite file as it is: migrations 1–96 carried over with their checksums, every stored
  document and sealed secret readable (decision 6; P3 showed it on the dev database, and runs once on a backup of the pod's
  before the server runs there).
- **Paths.** Each crate carries the tests of the path it is used through, written as its caller uses it; the five guarantees of
  decision 3 are among them. Hardening tests (stress, race, edge sweeps) wait until asked.
- **What the owner sees.** The pages in `web/verify/` render on the WASM core over a fake transport, looked at with
  `node web/verify/shot.mjs`; then Milestone 1 (decision 11).

No rollback is built: the cutover deletes the Go tree and no Go release is kept as a fallback. The database backup *Go live* takes
before every release stays, as it does for any release.

### 10. The client core and its shells

**What "all frontend state in the WASM" means under *Server-driven*.** The server keeps owning every piece of state, every rule and
every derived view, drafts included. The core holds only what a channel holds today, and decides every change to it: the session
and its state (connecting, syncing, live, stale, refused); the pending calls with their request ids, `seq` and timeouts
(`act.ts`); the replica's index (each record's id, home and revision) and the order in which a snapshot and early pushes apply
(`sync.ts`, `rev.ts`, `homes.ts`); the owner's typed words the server does not have yet, and the order their saves go in
(`drafts.ts`). It is dropped with the page, as today. View state stays in the URL. A rule or a derived view never enters the core
(`replica` cannot depend on a rule crate, decision 4); one found in a client moves to the server.

**Core shape.** `link` and `replica` are sans-IO state machines: `handle(Input) -> Vec<Output>`, the inputs being a frame on a
stream, a session opened or closed, a timer fired, a shell's call; the outputs being frames to send, streams to open or reset,
timers to arm, patches for the shell, a call's answer. They run natively in `cargo test` with no browser and no runtime, in
`wasm32` behind `client-web`, and natively behind `remit-client`.

**How the core feeds Vue.**

1. **Snapshots**: the whole replica after each change. Copies the state per push. Rejected.
2. **Vue reads through WASM memory** (getter proxies per field). No copy, but every render crosses the boundary and reactivity
   needs signals by hand. Rejected.
3. **Granular patches into Vue stores (chosen).** The core emits batches of `reset {store, records}`, `put {store, id, record}`,
   `drop {store, id}`, and `append {card, delta}` for a streaming turn. `web/src/core/` applies them: one `shallowReactive(Map)` per
   store, keyed by the server's id; a `put` replaces one record, built once as a JS object from that record alone; an `append`
   pushes the delta onto the card's event list and triggers it, so a streamed turn copies its delta, not its chain. Record bodies
   live once, in the shell's stores; the core keeps the index its rules need, not a second copy. Components read the stores as
   they read `catalog` and `opened` today.

**What moves into the core, what stays.** Into `replica`/`link`: `sync.ts`, `rev.ts`, `homes.ts`, `rpc/reconnect.ts`,
`rpc/index.ts`, the request id, `retrying`, `online` and `unanswered` of `act.ts`, the mutation actions of `catalog.ts`,
`openChain.ts`, `assistant.ts`, `openEvent.ts`, `archived.ts`, the overlay and save order of `drafts.ts`, the link state of
`session.ts`. Stays in the Vue shell: components, the router, toasts and their words, and the presentation modules (`stream.ts`'s
folding, `ui.ts`, `board.ts`), unchanged because this record changes no behaviour; that the folding and labels are derived views
the server should own is its own change (*Revisit when*).

**The shells.** Each holds a rendered copy of the records the core hands it, its own view state and its platform's I/O, and
nothing that decides. The patch vocabulary is the one interface every shell implements; a shell that needs something else asks the
server for it, as a view type in `wire::app`.

| Shell | Built from | Transport | Holds |
|---|---|---|---|
| Browser (Vue) | `client-web` | the browser's WebTransport through `web-transport` | Vue stores filled by patches; view state in the URL |
| iOS and macOS apps | the same Vue shell and `client-web`, in the system web view of a wrapper app | the web view's WebTransport (the OS's WebKit) | what the browser holds; the wrapper holds no state of Remit's |
| CLI, daemon (Rust) | `remit-client` over `link`, `replica`, `wire`, `link-quic` | `link-quic` | what one command or one run needs; nothing between runs but the credential |
| The Paseo worker (Node) | `remit-client worker`, its child, JSON lines on stdio whose TypeScript `tsgen` emits from `wire::worker` | `link-quic` in the child | what the Paseo plugin shows today |

**Costs, each with its check.** Bundle size and startup: P5. Rebuild: a change to a client crate is a `wasm-pack build` and a full
page reload (Vite reloads the package); Vue components keep hot reload; `scripts/dev.sh` watches `crates/{link,replica,wire,client-web}`
and rebuilds. Debugging: the core is tested natively, logs through `tracing` to the console in dev builds, and every frame is JSON.
`web/verify`: `fakeClient.ts` becomes a fake transport that answers the real core's frames from the page's seed, so the verify
pages run the replica's real logic. Tests: vitest loads the core built for Node with the fake transport (no WebTransport needed).

### 11. Port order: units for independent agents, and where it first runs

Each unit is one crate or a slice of `orch`, owned by the project agent of that area (`.claude/agents/`), worked in parallel with
every unit of its wave. A unit is done when its crate builds for its targets, clippy finds nothing, and its main-path tests pass; its `README.md` names
`ported-from:`.

- **Wave 0 (settles the risks).** U0.1 workspace, lints, `clippy.toml` (done). Then the gating PoCs P1, P4 and P5 (with P15), each in a scratch crate. A refuted PoC
  takes its named fallback; a refuted fallback gets a record of its own before Wave 1 builds on it.
- **Wave 1 (leaves, all parallel).** `model`; `grant`, `signal` (with P1's evaluator), `outcomes`, `rollout`, `replay`, `library`,
  `skills`, `childenv`, `egress`, `provider`, `secrets`, `auth`, `procgroup`; `wire` with `contract!` and `tsgen` (2,378 lines of
  types translated once); `link` with the five guarantee tests.
- **Wave 2.** `store` (the generated migrations, `testing` feature); `builtin`, `importer`; `jail` then `claudecode`; `forge`;
  `source`; `plugin-sdk`, `plugin` (after `wire::plugin`); `memoryd`; `link-quic`; `replica`. `store` first within it.
- **Wave 3.** `agent`; `bundled` (each plugin a sub-unit: GitHub, GitLab, Intercom, Amplitude, Google Analytics, script, forge);
  `runtimelink`, `airuntime`; `client-web` and `web/src/core/`; `remit-client`.
- **Wave 4: `orch`.** U4.1 first and alone: the skeleton (`Orchestrator`, its ports, `background`, `act`/`once` and `requests`,
  `o.now`, `Store::update` call sites split where they do I/O inside a transaction, `step`, the MCP server with the schema
  transform). Then in parallel, by the agents' file groups: chain runtime; agent tools and capabilities (183 tool sites); signals
  and subscriptions; objectives and crew; review and ship; sources, grants and plugins in orch; rails (client keys, entrance keys).
  Done per slice: its main-path tests; for the crate, the ported AST rules (`crates/fitness`, the eleven of *Context* read from `crates/`).
- **Wave 5.** `rpc`, `workerrpc`, `door`, `clientmcp`; the `remit` binary with every subcommand and the embedded bundle; the Vue
  shell moved to `web/src/core` in all 196 importing files; `web/verify` on the WASM core; the Paseo plugin on `remit-client`.
- **Milestone 1: the Rust server runs on the development Mac.** `remit -addr 127.0.0.1:4330 -data ~/.remit-rust-dev`, its UDP port
  beside it, with a second Vite on 5183, next to the Go dev instance on 4320 and 5173, which keeps running. Stage a:
  `~/.remit-rust-dev` is a copy of `~/.remit-dev` taken with `sqlite3 .backup` (its own file and `flock`), and the binary runs with
  `-agent`, `-gh` and `-glab` pointing at stubs that refuse, no `-source-env` and no `-vault`, so the chains it recovers from the
  copy cannot act outside a second time; checked: every page in a real browser, `Describe` of every bundled plugin, P13 and P14 against it. Stage b: a fresh `~/.remit-rust-dev` with the real `claude`, `gh`
  and `glab`, where the Assistant starts one chain that runs to its judge, a sensor fires, and the server restarts mid-turn. Claude
  Code's login is keyed by its config dir's path, so stage b signs its accounts in under its own data dir.
- **Milestones 2 and 3**, each only with the owner's go-ahead: macmini staging, then the server through *Go live*, after P15 holds
  there.
- **Wave 6: cutover.** The last `git diff <ported-from>..HEAD` pass of every unit, repeated until empty; the one cutover change: delete the Go tree,
  `go.mod`, `go.sum`, the socketrpc-gen lines and every file listed in decision 2; `make build`, `make gen`, `make check-gen`,
  `make test`, `make test-changed`, `scripts/test_changed.sh`, `scripts/lint_changed.sh`, `scripts/dev.sh`, the release workflow
  and the hooks on cargo; `AGENTS.md`, `docs/design.md` and the agents' definitions say Rust; then *Go live*, with the owner's go-ahead.

### 12. One protocol for a network of nodes: the seams now, the network later

The owner's direction (2026-11-10): usage like Paseo, a network of daemons that talk to each other, the owner joining it through
his GUI clients, one protocol for all; today one cluster on one machine, later one or many AI clusters on several machines talking
directly over WebTransport. Nothing of the network is built here. What would be expensive to add later is fixed now, because it
sits in the handshake, the envelope and the contract, which every peer and every stored client copy would otherwise have to change
at once.

Weighed, pattern by pattern:

1. **`link` is a symmetric peer protocol; what a peer may do is its capabilities, not its kind. Kept, as a seam.** It costs little:
   a WebTransport session already lets either side open streams, the runtime contract already runs both ways
   (`runtime/define.ts:184,203`), and the frames above have no client or server in them. So both halves of `link` can call and
   push; a `contract!` call names the capability it belongs to (`app`, `runtime`, `worker`, the three modules of `wire`); `hello`
   asks for capabilities and `welcome` grants a set, and each side refuses a call outside what it granted the other
   (`INVALID_ARGUMENT` is wrong for that: a new code, `FORBIDDEN`, joins the envelope). One path, `/link`, serves every peer. The
   strongest counter: symmetry invites any peer to call anything. Answered by the grant: the capability set, decided by today's
   keys, is the only door, and a browser still cannot be dialled (only peers that listen can).
2. **Every node has an identity in the handshake. Kept, as a seam; the control is not built.** `hello` and `welcome` carry
   `node {id, public_key}` (Ed25519; the id is the key's hash), each node making its key pair at first start in its data dir.
   Admission stays what it is today (owner key, runtime key, worker key, decision 3's guarantee 5); the node key is recorded, not
   yet proven or trusted (*Revisit when*: proof of possession over the session and grants signed by the owner, which the owner key
   and client keys grow into). This is a rail by AGENTS.md's terms, so the missing control is named.
3. **One home node per entity, others hold replicas and send commands to it. Kept as the model, with a routing field, not new ids.**
   It is *Server-driven* restated: the home node drives; replicas are snapshot plus ordered patches by revision, exactly what
   `replica` already does; commands carry a `Request` and act once. Putting the home node into every entity id was weighed and
   rejected now: it rewrites every stored id and every reference to one, a data migration the cutover forbids (decision 6), for a
   network of one node. Instead a call frame carries an optional `home` (a node id), absent meaning the callee's own node, and a
   node answers a call whose `home` is not itself with `NOT_HERE` (a code reserved now, never sent while there is one node). Where an
   entity's home is recorded, when there are several, is the routing decision that waits.
4. **Actors inside a node: one task with a mailbox per chain, session and connection. Not now.** It is a better concurrency shape for
   many nodes and many chains, but adopting it during the port changes orch's concurrency and the language at once, and doubles what a
   port unit must get right. Decision 5 translates shape for shape; a single writer per chain
   already holds (only `step` sets a chain's status). *Revisit when* contention or a second node asks for it.
5. **Functional core, ports and adapters, sans-IO protocol core.** Already decided (decisions 4, 5, 10).
6. **Peering is explicit (an address and a key); a reachable daemon is the gateway for browsers. The model, not built.** Two risks
   stay open, each with a PoC before the network is built: daemons behind NAT (`web-transport-iroh` "implements the
   `web-transport-trait` traits for iroh connections and streams"; its README says nothing of browsers, NAT traversal or relays,
   which would come from iroh itself, unread: LOW), and browser trust of a daemon's certificate (by hash only for certificates of
   less than two weeks, rotated and handed to the browser through a trusted path; a long-lived public daemon wants a certificate
   from a CA).

Fixed now, then: node identity in `hello`/`welcome`; the symmetric envelope (either peer calls and pushes); capability sets granted
per session in place of three endpoints; the optional `home` routing field and the `FORBIDDEN` and `NOT_HERE` codes. Each has a
test with `link`: `link::either_peer_may_call_within_what_it_was_granted` (a call outside the granted set is `FORBIDDEN`, a call
from the accepting side runs) and `link::a_call_for_another_home_is_not_here`. Waiting: routing between nodes, replication between
daemons, discovery, NAT traversal, signed grants. Milestone 1 stays one node on the development Mac.

## Threats

The entrances let in what they let in today: on `/link`, the owner key grants the `app` capabilities, the runtime key the
`runtime` ones (ADR-0058), a worker key the `worker` ones; the door's signatures, the client door's keys, plugin binaries pinned by hash. What changes is who checks them, and that
TLS now ends at the binary.

| Threat | Control now |
|---|---|
| The credential moves to the first frame of the control stream; a server that serves a stream before `hello` is checked lets anyone in | The server half of `link` accepts no other stream until `welcome` (guarantee 5's test) |
| A development certificate hash is reused or leaked | It is made at each start, valid less than two weeks, rotated before it expires, and only for the loopback name; production uses a certificate for the public name |
| The Rust plugin host accepts a binary whose hash differs | The pin is checked before exec; P2 runs a binary with one byte changed and expects a refusal |
| A rule ported wrong opens a rail (pause, stop, authentication, merge) | Each rail carries the tests of the path it is used through, ported in its unit (authentication, pause and stop, merge) |
| The jail loses an isolation step silently | `probe_linux_test.go` and `hidden_linux_test.go` ported; P4 |
| An MCP call skips its input schema | Arguments validated against the schema in `call_tool` (P7) |

Threat modelling (`aio-threat-models`) runs when the owner asks for hardening.

## Consequences

- One language for the backend, the AI runtime, the plugins Remit ships and every client's core; Go leaves the repository at
  cutover, and `socket.io-client` leaves `web/package.json`.
- The contract is Rust: adding a call is one line in a `contract!` list, and the TypeScript Vue reads is regenerated by our own
  generator. A new channel is a shell over the same core.
- Plugins implement one trait: compiled into the server for bundled ones, served over stdio for applied ones, or written in any
  language speaking `link` over stdio; all of them sit behind the same gate.
- The server terminates TLS on a UDP port: deployment needs a certificate at the binary and UDP reachability, which the pod does
  not give today (P15). Until a WebSocket driver exists, a browser without WebTransport (any Safari before 26.4) cannot connect.
- Every GUI is the Vue shell on the WASM core, in a browser or an app's web view. The iOS and macOS apps support only the latest
  release of each OS (the owner, 2026-11-10), whose WebKit has WebTransport (26.4 and later), so they need no fallback (P13).
- Product work in Go goes on until the cutover change; each unit's last diff pass carries it over.
- No rollback exists: the cutover deletes Go and goes forward, and a defect found after it is fixed forward in Rust. Parity with
  Go is not proven test by test; what is proven is that the server works on the owner's data and on the paths he uses.
- A Rust build is slower than a Go build; `orch` as one crate is the slowest part. The browser downloads a WASM module before its
  first snapshot (P5 bounds it).
- Accepted records whose text names a Go file or package stay as written; `docs/design.md` names the crates after cutover.

## Revisit when

- **P15 fails on the server, or the owner needs a browser without WebTransport**: the WebSocket driver of `link::Transport`, the same
  frames with a stream id in each, offered when WebTransport cannot connect. The seam is the trait; nothing above it changes.
- **`web-transport` reaches 1.0, or its wasm side drops `web_sys_unstable_apis`**: remove the cfg from the build.
- **Frames grow large or many** (a snapshot measured in megabytes, a push rate the JSON parse shows in a profile): a binary encoding
  for the envelope, chosen in `wire::envelope`.
- **`orch`'s compile time slows agents' loops** (an incremental build over a minute): split it into crates along the agents' file
  groups, with the `Orchestrator`'s ports as traits.
- **The stream folding moves to the server** (`stream.ts` `blocks` and `foldedRuns`, the counted phrase of a group, `ui.ts`'s
  labels): each event already carries what it is (`wire::app::Activity`: a tool call's group, name, words and status, a message
  sent through a tool, a turn's end, a permission prompt decided, typed by `orch::activity`); the view types gain the folded
  blocks and every shell renders them.
- **A client tool sends server-initiated messages** (progress, sampling, elicitation): `/mcp/client` is stateless MCP, each
  request checked and served by its own key's server; such a tool needs sessions bound to the key that opened them.
- **A plugin's sensor outruns its reader**, or a plugin binary could be swapped between its hash check and its start:
  sensor channels get a bound with back-pressure (`plugin_sdk::Senses`), and the pinned binary is opened once, checked and
  run from that handle.
- **Confinement of a plugin is asked for**, or a plugin comes from outside the repository: a `wasmtime` component runner (WASI
  0.3, WIT imports = the granted capabilities, `wasi:http` routed through the egress gate) as a third implementation of `Plugin`.
- **A second node joins** (an AI cluster on another machine): proof of possession of the node key over the session (a signature
  over the TLS exporter), grants signed by the owner naming a node's capabilities (the seam: `node` and `capabilities` in `hello`),
  where each entity's home is recorded and how a call is routed to it, replication of replicas between daemons, discovery, and the
  two PoCs of decision 12 (NAT between daemons over `web-transport-iroh`; a browser reaching a daemon by a rotated certificate hash
  or a CA certificate).
- **Contention inside a node, or a second node**: actors, one task with a mailbox per chain, session and connection.
- **The app wrappers are built**: what the shell is. Tauri 2 (Rust-native, the same toolchain, a WKWebView on Apple platforms, its
  own updater and plugin model) against a plain WKWebView wrapper in Swift (least code, nothing between the page and WebKit); P13
  runs on both, and the choice gets a record of its own.
- **A peer that is gone must be noticed sooner than the transport does** (a runtime frozen mid-session, a laptop that sleeps with
  its tab open): each half pings every `link::Config::ping_every` only to keep QUIC's idle timeout from closing a quiet session,
  and reads the `pong` as nothing. The seam is `Conn::ping` and its `pinged_at`: a ping left unanswered past a bound closes the
  session.
- **Hardening is asked for**: a frame size limit and a rate limit before `hello`, certificate rotation at the binary, the jail's
  syscall filter (seccomp), and a fuzz target on the envelope codec, at `link`'s server half, `link-quic` and `jail`. The limits
  are one policy in `link::Conn`, so every driver has them: `link::Config` gains the largest frame (smaller before `welcome`; a
  larger one closes the session) and the most calls in flight (one past it is answered `UNAVAILABLE`), and the pipe driver's own
  64 MiB cap (`plugin_sdk::pipe`) goes. Today `envelope::Reader` buffers any length a frame's header names and `rpc::session`
  spawns every call unbounded.
