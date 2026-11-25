# Codex vs Remit: the contract, the link and the channels

Scope: what OpenAI Codex's Rust app-server protocol (`references/codex/codex-rs`, Apache-2.0) does that Remit's `wire`,
`link`, `link-quic`, `tsgen`, `replica`, `client-web`, `rpc`, `workerrpc` and `remit-client` could learn from. This is a
read-only study: nothing was built or run. Paths starting `codex:` are under `references/codex/codex-rs/`. Every other
path is in the Remit repository.

Coverage: I read Codex's `app-server-protocol` (rpc, common macros, experimental gate, schema fixtures),
`app-server-transport` (transport mod, connection_auth, websocket_compression, unix_socket excerpts),
`app-server/src/request_serialization.rs` and `error_code.rs`, `app-server/README.md` (first 260 of 951 lines),
`app-server-client/README.md`, `app-server-daemon/README.md` (first 120 of 195 lines), `docs/protocol_v1.md`, `uds`,
`stdio-to-uds`, `test-binary-support`, `config-schema`, the workspace lints and `clippy.toml`. I skimmed `protocol/` (34.7k
lines) and the 217k lines of `app-server/src`, and read only the parts named below.

For the Codex-first pass I also read:
- `app-server-daemon` (its whole README, `lib.rs`, `thread_recovery.rs`, the `update_loop.rs` outline)
- the subcommands of `app-server-test-client`
- `cli/src/doctor.rs`, plus the file list of `cli/src`
- the `tui` file list and how it uses `insta`
- the heads of `codex-home`, `websocket-client` and `tcp-tunnel`

I did not read the `tui` or `cli` sources beyond those parts, or the `remote_control` transport.

## Ranking

| # | Idea | Value | Effort | Confidence of the gap |
|---|---|---|---|---|
| 1 | Typed agent-activity items: the server types each tool call, and the browser stops deriving from raw input | HIGH | L | HIGH |
| 2 | Order calls by the entity they act on, not by method | HIGH | M | HIGH (read orch's pause and resume path) |
| 3 | A checked-in contract schema, and a test that a breaking change bumps the contract's number | HIGH (when CLI, daemon or a second node ship), MED today | M | HIGH |
| 4 | Typed failure detail beside the code | MED | M | HIGH |
| 5 | Compare-and-set on what the owner writes (`base_rev` on `draft_save`) | MED | S–M (needs 4) | HIGH (orch's own test shows last write wins) |
| 6 | A frame-size cap and an in-flight cap in `link::Conn`, one policy for every driver | MED | S | HIGH |
| 7 | Local transports: move the pipe driver into its own crate and add a Unix-socket listener | MED | M | HIGH |
| 8 | What each session watches (homes), so not every session gets every push | MED (LOW today) | M | HIGH |
| 9 | Trace context in the call frame | LOW–MED | S | HIGH |
| 10 | Two more workspace lints: `await_holding_invalid_type` and `disallowed_methods` | LOW–MED | S | HIGH |
| 11 | Negotiated compression | LOW | S–M | HIGH |

---

## 1. Typed agent-activity items: the server types each tool call (HIGH)

**What Codex does.** Every item in a thread is a variant of a tagged union, and each variant has typed fields:
`ThreadItemPayload` (`codex:app-server-protocol/src/protocol/v2/item.rs:341`) has `CommandExecution`, `FileChange`,
`McpToolCall`, `DynamicToolCall`, `CollabAgentToolCall` (sender and receiver thread ids, prompt, agent states),
`SubAgentActivity`, `WebSearch`, `ImageView`, `ContextCompaction` and more (`item.rs:344-540`). Status fields are enums,
for example `CommandExecutionStatus`. The server parses a shell command into
`command_actions: Vec<CommandAction>` (`Read{path}`, `ListFiles`, `Search{query,path}`, `Unknown`;
`item.rs:134-152`), and `CommandExecution` documents this as "a best-effort parsing of the command" (`item.rs:~418`). Clients
render these items without parsing anything themselves. HIGH (read).

**What Remit does today.** `wire::app::Event` carries `tool_kind: Option<String>`, `status: Option<String>` and
`title: Option<String>`, plus `data.input: Option<Value>` (`crates/wire/src/app.rs:810-832`, `771-773`). The browser then
derives views from those raw fields:
- `toolGroup` maps the `toolKind` string to a group (`web/src/stream.ts:23-41`).
- `sentOf` reads `data.input.to_card` and `kind` and switches on `e.title === 'mcp__remit__route'` to find "a message this
  agent sent" (`stream.ts:57-66`).
- `stream.ts` also folds tool calls and builds the turn-end blocks.

This breaks AGENTS.md *Server-driven* ("the server owns every rule and derived view: grouping, labels, status"), and the
web-ui role definition repeats that rule. ADR-0072 already names this debt under *Revisit when* (`docs/adr/0072-…md:616`).
HIGH.

**Gap.** Every new channel (CLI, daemon) has to re-implement `stream.ts`. A rename of the Remit MCP tool silently breaks the
"sent" block. `status` is a string, so a misspelling compiles.

**Proposal (`wire::app`, then the orch conversion, `replica`, `web`).**
- Replace `tool_kind`, `status` and `data.input` on `Event` with `activity: Option<Activity>`. `Activity` is a tagged enum
  that the server fills in:
  - `Read{path}`, `Edit{path}`, `Execute{command, actions: Vec<CommandAction>}`, `Search{query}`, `Fetch{url}`
  - `Sent{to: Option<String>, kind, label, text}` (today's `sentOf`)
  - `Asked{question}` (`ask_chair`)
  - `Remit{tool}`
  - `Other{name, input: Value}`
- Add a status enum `ToolStatus { Pending, Running, Completed, Failed }`.
- Give the chain view the folded blocks (`Block::Tools{summary, running}`, `TurnEnd{ms}`), so `stream.ts` becomes a renderer.
- The mapping lives once, where an event is stored or converted (the orch side, owned by the chain-runtime area), and
  `tsgen` carries the union to TypeScript.

The stored event shape may need a numbered migration (store-and-migrations area). Effort is L: it crosses wire, orch,
replica and web, so it is several areas' work in one change.

## 2. Order calls by the entity they act on, not by method (HIGH)

**What Codex does.** Each request declares a serialization scope in its contract entry:
- `serialization: thread_id(params.thread_id)` on `turn/start` (`codex:app-server-protocol/src/protocol/common.rs:1107-1111`)
- `thread_or_path(...)` on resume and fork (`common.rs:581-592`)
- `None` for calls that must not block their thread, with the reason written beside it: "Waiting for a sample must not block
  requests on its parent thread" (`common.rs:593-598`)

The macro turns that declaration into `serialization_scope()` (`common.rs:141-205`, `260-272`). The server keeps one queue
per key (`codex:app-server/src/request_serialization.rs:24-55`). The `Thread` key is **not** scoped per connection, so two
clients acting on one thread are ordered as well. `Global` and `GlobalSharedRead` give exclusive and shared access
(`request_serialization.rs:53-56`). HIGH (read).

**What Remit does today.** Guarantee 3: "a peer's calls of **one method** run one at a time in the order sent (`seq` per
method)" (`crates/link/README.md`, `crates/link/src/conn.rs:130`, `179`, `373`). Calls of different methods are each
spawned (`crates/rpc/src/session.rs:41`). So `chain_pause(c1)` followed at once by `chain_resume(c1)` from one tab are two
methods, and the link does not order them. The same holds for the Mac and the phone acting on one chain. HIGH.

orch does not restore the order. Each change is atomic: `apply` holds the chain's lock around `step` and the commit
(`crates/orch/src/apply.rs:184-201`), so ADR-0072's "single writer per chain" holds. Which change goes first, though, is
whichever task takes that lock first, and the two paths reach it at different speeds:
- `pause_as` first awaits `lock_runs` and `paused_runs`, then calls `apply` (`crates/orch/src/lifecycle.rs:414-422`).
- `resume_as` calls `apply` at once (`lifecycle.rs:435-445`).

So a resume sent right after a pause can take the lock first. It is then `Unchanged`, because the chain is not paused yet,
and the pause applies after it. The chain ends **paused**, against the owner's last word, and both calls answer success.
HIGH that nothing orders them (read). That it happens in use needs two clicks close together; I did not reproduce it, and
the test below would.

**Gap.** Ordering is a property of the entity, but Remit declares it by method name.

**Proposal (`link::contract!`, `link::Conn`).**
- Add an optional `order = <arg>` to a `changes` or `idempotent` entry:
  `chain_pause(id: String, why: String) -> Ack, order = id;`.
- `Conn` counts `seq` per `(capability, order key)` and falls back to the method when no key is declared. This stays sans-IO
  and testable in `tests/guarantees.rs`.
- Cross-session ordering belongs to orch: one queue per chain, the same key. Write the test before the fix: a pause and a
  resume sent back to back, delivered in reverse, must end resumed.

## 3. A checked-in contract schema, and a test that a breaking change bumps the number (HIGH later, MED now)

**What Codex does.**
- It checks in the generated TypeScript (98 files) and JSON Schema (39 files) under
  `codex:app-server-protocol/schema/{typescript,json}`.
- Tests regenerate both and diff them against the checked-in fixtures (`schema_fixtures_tests.rs:17-41`). A protocol change
  therefore shows as a schema diff in review, for clients in any language.
- New surface is gated: `#[experimental("reason")]` on a method or field (`common.rs:529-600`, `experimental_api.rs:5-31`),
  served only to clients that opt in with `capabilities.experimentalApi` (`v1.rs:56-57`). This lets the stable schema change
  only additively.

HIGH (read).

**What Remit does today.**
- `wire::app` is lockstep: a page on another bundle is refused with `new_bundle` and reloads (`crates/rpc/src/serve.rs:86-93`).
  That is right for the web and for the iOS and macOS wrappers, which load the page from the server (ADR-0072 P13, line 408).
- `wire::worker` has a hand-kept number: "Additive changes keep it (a call, an optional field); any other change bumps it"
  (`crates/wire/src/worker.rs:12-13`, compared in `crates/workerrpc/src/serve.rs:26-35`). Nothing checks that rule.
- `make check-gen` catches drift between `wire` and the generated TypeScript (`Makefile:10-23`). It does not catch a
  breaking change to a contract whose peer is another build (the Paseo worker today; the CLI, daemon and other nodes next,
  ADR-0072 decisions 10 and 12).

HIGH.

**Proposal (`tsgen`, `wire`, `fitness`).**
1. `tsgen` also writes a canonical `crates/wire/contract/<module>.json`: each call with its kind, argument and answer shapes,
   and each push. `make gen` writes it and `check-gen` diffs it.
2. A test classifies the diff from the committed file. Adding a call, a push or an `Option` field is additive. Anything else
   is breaking, and the test fails unless the module's `CONTRACT` number moved.
3. Every contract gets the same number (`wire::app::CONTRACT`, `wire::runtime::CONTRACT`, `wire::plugin::CONTRACT`), and a
   native peer's hello names `capability@n`. One rule, one name.

Codex's experimental gate is not needed while Remit is lockstep. It would be a flag in disguise.

## 4. Typed failure detail beside the code (MED)

**What Codex does.** The error envelope has `{code, message, data?}` (`codex:app-server-protocol/src/rpc.rs:81-88`).
Examples of `data`:
- `{"reason":"serverShuttingDown"}` while the server drains (`codex:app-server/src/error_code.rs:9-13`;
  `app-server/README.md:66-68`)
- `{reason:"readStateConflict", readState:<current>}`, which hands the client the current record (`README.md:97-104`)
- turn errors carry the typed `CodexErrorInfo` enum (`TooManyDenials`, `UsageLimitExceeded`, and `HttpConnectionFailed{httpStatusCode}`;
  `v2/shared.rs:76-106`)

HIGH (read).

**What Remit does today.** `link::Failure { code: ErrorCode, message: String }` (`crates/link/src/envelope.rs:32-35`).
Anything beyond the code is prose, so a channel cannot act on "conflict, here is the current draft" or "the server is
restarting, reconnect now" without parsing words. HIGH.

**Proposal (`link`, `wire`).**
- Add `detail: Option<Value>` to `Failure` at the envelope (sans-IO, contract-agnostic).
- Each contract declares its detail type: `contract! { capability: "app"; failures: AppFailure; … }`. `serve` encodes it and
  `Pending::decode` returns it typed. `tsgen` emits it like any other type.
- First variants: `Conflict { current: … }` (idea 5), `ShuttingDown` (a close the replica answers by reconnecting at once
  rather than backing off), and `QuotaReached { until }`.
- No untyped `Value` crosses into a handler.

## 5. Compare-and-set on what the owner writes (MED)

**What Codex does.** `thread/readState/update` takes `{threadId, expectedRevision, operation}` and refuses a stale revision
with the current state attached. The README says "Do not blindly retry", and revisions are equality tokens, not clocks
(`app-server/README.md:97-112`). HIGH (read).

**What Remit does today.** `draft_save(draft: DraftInput) -> Draft` (`crates/wire/src/app.rs:2858`). `DraftInput` has no
revision (`app.rs:1009-1026`), and each save becomes "the same draft at its next revision" (`crates/orch/src/drafts.rs:198`).
In one replica, saves go in order, and an answer to an older save leaves newer typing held
(`crates/replica/src/drafts.rs:1-5`, `Typed.version` at `:46-50`). Nothing compares revisions between two pages or devices.
orch's own test pins last-write-wins as the behaviour: "A second page's first save in the same composer goes on the draft
already there". A second page's save replaces the text, and the test asserts the view holds only the second page's words
(`crates/orch/src/drafts.rs:198-228`). Nothing reports a conflict to either page. HIGH (read).

"One composer holds one draft" is deliberate. The gap is only that a page's words are overwritten without telling it.

**Proposal.** Add `base_rev: i64` to `DraftInput`. orch refuses a stale save with `Conflict { current: Draft }` (idea 4). The
replica keeps the owner's local text, shows that the draft moved, and saves again on top of `current`. Apply the same
pattern to any future change that overwrites text the owner wrote.

## 6. Caps on frame size and calls in flight, enforced in `link::Conn` (MED)

**What Codex does.**
- Incoming messages go through a bounded queue. When it is full, a request gets the retryable `OVERLOADED_ERROR_CODE -32001`
  "Server overloaded; retry later." (`codex:app-server-transport/src/transport/mod.rs:54`, `241-268`).
- WebSocket frame and message sizes are capped (`transport/unix_socket.rs:183-188`), and so is decompressed size
  (`websocket_compression.rs:117`).

HIGH (read).

**What Remit does today.**
- `envelope::Reader::next_frame` accepts any `u32` length and buffers until the frame is whole, so up to 4 GiB per stream,
  even before `hello` (`crates/link/src/envelope.rs:160-166`).
- `session::run` spawns every call with no bound (`crates/rpc/src/session.rs:41`).
- The pipe driver caps records at 64 MiB on its own (`crates/plugin-sdk/src/pipe.rs:29`), and `link-quic` has no cap, so
  each driver has its own policy.

ADR-0072 defers "a frame size limit and a rate limit before `hello`" to hardening (`docs/adr/0072-…md:634`). HIGH.

**Proposal (`link`).** Add `Config { max_frame, max_inflight }` and enforce both in `Conn`, so every driver gets them:
- A frame larger than `max_frame` closes the session `Broken`, and the limit is smaller before `welcome`.
- A call beyond `max_inflight` is answered `UNAVAILABLE` with retry detail.
- Delete `pipe.rs`'s own constant.

This is the seam AGENTS.md asks for now (the limits can start generous), and its numbers are recorded under *Revisit when*.

## 7. Local transports: a pipe crate and a Unix-socket listener (MED)

**What Codex does.**
- One message format runs over stdio, a Unix socket, a WebSocket and an in-process typed channel
  (`codex:app-server-transport/src/transport/{stdio,unix_socket,websocket}.rs`; `app-server-client/README.md` "Transport
  model": typed in process, JSON only at external boundaries).
- The local daemon socket lives in a 0700 directory with a 0600 socket, and it is verified before use: the directory's
  owner must be the current user (`transport/unix_socket.rs:34`, `325-338`; `codex:uds/src/lib.rs:18-24`).
- `stdio-to-uds` bridges any stdio client to it (`codex:stdio-to-uds/README.md`).
- The daemon is single-instance through a startup lock (`transport/mod.rs:57-59`), and every lifecycle command prints exactly
  one JSON object (`app-server-daemon/README.md` "Commands").

HIGH (read).

**What Remit does today.**
- `link-quic` (WebTransport) serves every network peer.
- A second, complete link driver over a byte pipe is buried in `crates/plugin-sdk/src/pipe.rs` (513 lines), and only plugins
  use it.
- Local peers that could use a socket instead (the AI runtime, the coming CLI and daemon on the same machine) go through QUIC
  with certificate hashes (ADR-0072 decision 3).

HIGH.

**Proposal.**
1. Move `pipe.rs` into a `link-pipe` crate: the driver for any `AsyncRead + AsyncWrite`. plugin-sdk and remit-client both
   depend on it.
2. Have `remit` listen on `<data_dir>/link.sock` (0700 directory, 0600 socket) with the same `hello`. Whether the file mode
   can stand in for the credential is a rail decision. It needs an ADR and the rails-and-sandbox area, and the default is to
   keep the credential.
3. `remit-client` dials the socket when the server is local and QUIC otherwise.

## 8. What each session watches (MED, LOW today)

**What Codex does.** A connection receives a thread's events only after it starts or resumes that thread, and stops
receiving them on `thread/unsubscribe` (`common.rs:616-619`; `README.md:107`). At initialize it can opt out of named
notifications (`opt_out_notification_methods`, `v1.rs:66-69`). HIGH (read).

**What Remit does today.** `Broadcast` queues every change for every session (`crates/rpc/src/broadcast.rs:1-4`,
`BEHIND_BY = 512` at `:21`). The per-home streams exist (`wire::app::Push::home`, `app.rs:2942-2965`), but every session is
on all of them. HIGH.

**Gap.** A `remit watch <chain>` CLI, a phone on cellular, or a peer node gets the whole firehose, and it risks `Behind` on
pushes it never shows.

**Proposal (`wire::app`, `rpc`, `replica`).** Add an `idempotent` call `watch(homes: Vec<String>)`. Its `why` is "sets the
session's whole watch set; twice is once". Broadcast filters by `Push::home`, and the feed stays on. The web keeps watching
everything (no behaviour change). The CLI watches one home, with a snapshot scoped to it.

## 9. Trace context in the call frame (LOW–MED)

**What Codex does.** Each request may carry `trace: Option<W3cTraceContext>` (`codex:app-server-protocol/src/rpc.rs:52-55`).
HIGH.

**What Remit does today.** `Frame::Call { seq, contract, home, body }` carries no trace (`crates/link/src/envelope.rs:119-126`),
so one action that crosses the browser, core, AI runtime, worker or plugin cannot be followed as one trace. HIGH.

**Proposal (`link`, `link-quic`, `link-pipe`).** Add `trace: Option<String>` (a `traceparent`) to `Frame::Call`. The driver
enters a `tracing` span with it, and the core sets it from the current span. The change is additive on the wire.

## 10. Two more workspace lints (LOW–MED)

**What Codex does.** Its `clippy.toml` (lines 3-7, 8-24) does two things:
- `await_holding_invalid_type` lists `tokio::sync::MutexGuard`, `RwLockReadGuard` and `RwLockWriteGuard`.
- `disallowed-methods` routes escape hatches through their one owner ("Create SQLite pools through codex-state's sqlite shim").

The workspace also denies `redundant_clone` and `needless_collect` (`codex:Cargo.toml:600-636`). HIGH.

**What Remit does today.** It already denies more than Codex: `panic`, `indexing_slicing`, `arithmetic_side_effects`,
`as_conversions` and `allow_attributes`, with `unsafe_code` forbidden (`Cargo.toml:14-33`). It has `await_holding_lock` but not
the tokio guard list, and it uses tokio locks in `orch`, `plugin` and `source` (rg: `orch/src/orch.rs`, `plugin/src/remote.rs`,
…). It has no `disallowed-methods`. HIGH.

**Proposal (workspace, release-and-ops or reviewer to own).** Add `await_holding_invalid_type` with the tokio guards.
Add `disallowed-methods` so that opening a `web_transport` session or stream outside `link-quic` is refused, and so is
`serde_json::from_value` on a frame body outside `link`. This keeps one path, mechanically.

## 11. Negotiated compression (LOW)

**What Codex does.** One persistent zstd stream per direction (level −7, window 2^20). It is negotiated by an upgrade header,
and plain JSON remains when the other side does not answer it (`codex:app-server-transport/src/websocket_compression.rs:1-29`).
HIGH.

**What Remit does today.** Frames are uncompressed JSON. ADR-0072 waits for a measurement before a binary encoding
(`docs/adr/0072-…md` *Revisit when*, "Frames grow large or many"). **Proposal.** Keep waiting for that measurement. If a
snapshot proves large, negotiate `encodings` in `hello`, applied per stream inside the driver. Value stays LOW until measured.

---

## Codex does better / Remit lacks (Codex-first pass)

This pass reads Codex on its own terms and asks one question: is this engineering better than what Remit has, whether or
not Remit has a slot for it? Ideas 1–11 above already cover what this pass also found:
- per-entity serialization (2)
- checked-in schemas and versioning (3)
- typed error detail and the drain reason (4)
- compare-and-set (5)
- overload and size caps (6)
- local transports (7)
- subscriptions (8)
- trace context (9)
- lints (10)
- compression (11)

What follows is new, ranked by value.

| # | Codex engineering | Value | Effort | Owner area in Remit |
|---|---|---|---|---|
| A | A protocol-level client that drives the real server from a shell | HIGH | M | rpc-contract (`remit-client`) |
| B | One command for the server's lifecycle: idempotent, serialized, one JSON object out, update included | HIGH | M | release-and-ops |
| C | A read-only `doctor` that writes redacted, serializable rows | MED | M | release-and-ops, with rpc-contract for the link probe |
| D | Generated reference for the binary's settings (`config-schema`) | MED | S | release-and-ops |
| E | Snapshot tests for generated and rendered output (`insta`) | LOW | S | rpc-contract (tsgen) |

### A. A protocol-level client for agents and tests (HIGH)

**Codex.** `codex-app-server-test-client` is a CLI over the real protocol
(`codex:app-server-test-client/src/lib.rs:157-215`):
- `Serve` starts the server on a WebSocket and logs to a known directory.
- `SendMessage` and `ThreadResume` drive a thread.
- `Watch` initializes and dumps every inbound message until interrupted (`lib.rs:196-199`).
- `TriggerCmdApproval` and `NoTriggerCmdApproval` start turns that should or should not ask for approval (`lib.rs:200-214`).

Anyone, human or agent, can exercise and observe the server's contract with no UI in between. Codex's own TUI and `exec` are
clients of the same app-server (`codex:app-server-client/README.md` "Purpose"; `tui/src/app_server_session.rs`). HIGH (read).

**Remit.** `remit-client` has one command, `worker` (`crates/remit-client/src/main.rs:13-18`). Nothing in a shell can call
`wire::app`, watch its pushes or replay a `Request`. AGENTS.md asks for verification "on the surface the user looks at (…
API: call it)", but for the owner's contract the only caller is the browser. HIGH.

**Proposal (`remit-client`, generic from the contract, no code per call).**
- `remit-client call <method> '<args json>' [--request <id>]`: the JSON is decoded as `wire::app::Call` (it is already
  serde-tagged `{method, args}`, `crates/link/src/contract.rs:182`). The CLI fills in a fresh `Request`, or the given one so
  a replay can be checked by hand. It prints the answer, or the `Failure`, as one JSON object.
- `remit-client watch [--home <id>]`: pushes as JSON lines.
- `remit-client snapshot`: prints the snapshot.
- The credential is the owner key, read from a file or the environment and never from an argument.

The `lines` relay already does this for `wire::worker` (`crates/remit-client/README.md`), so the design is to make
`lines` generic over a contract rather than to build something beside it. This also becomes the first CLI channel of
ADR-0072 decision 10.

### B. One command for the server's lifecycle (HIGH)

**Codex.** `codex app-server daemon start|restart|update|stop|version|bootstrap` (`codex:app-server-daemon/README.md:30-43`)
has these properties:
- Every command writes exactly one JSON object to stdout for machines (`:43-46`).
- `start` is idempotent and returns only once the server answers the protocol handshake on its socket, not merely once a
  port is open (`:161-165`).
- Mutating lifecycle commands are serialized by a lock, so concurrent ones do not race (`:184-187`).
- `stop` requests a graceful exit, then forces it after a bounded grace of 0–300 s, 60 by default (`:70-76`).
- The updater restarts the server on the new binary before it replaces itself, and an explicit pin stops automatic updates
  (`:126-145`).
- Interrupted turns are written to a recovery file and resumed after a planned restart
  (`codex:app-server-transport/src/daemon_recovery.rs:12-27`; `app-server/src/daemon_thread_recovery.rs:1-3`).

HIGH (read).

**Remit.** Going live is a runbook in AGENTS.md (*Tools and runtime*, "Go live", `AGENTS.md:329-335`): push,
`make release`, `gh run watch`, back up `remit.db` with `sqlite3 .backup`, run `update.sh`, `pm2 restart`, then check that
`/healthz` reports the commit and that the page serves the new asset hash. `scripts/update.sh` (58 lines) only installs the
binary and repoints `current` and `previous` (`scripts/update.sh:5-6`). The backup, the restart and the readiness check are
done by hand, in the step most likely to cause tomorrow's incident. Single-instance is already enforced by the database lock
(`crates/store/src/lock.rs:2`), which is good. HIGH.

**Proposal (`remit` subcommand, release-and-ops).** Add `remit update [--to <tag>]`, idempotent and serialized by a lock in
the data dir, printing one JSON result (`{"from","to","backup","healthy"}`). In order, it:
1. fetches and verifies the release, as `update.sh` does
2. backs up the database
3. asks the running server to drain (idea 4's `ShuttingDown`)
4. restarts it through the supervisor
5. waits until a `hello` on `/link` is welcomed and `/healthz` names the new commit, which is readiness through the
   protocol, as Codex does it
6. repoints back to `previous` and restarts if the wait fails

The runbook in AGENTS.md then shrinks to one command.

### C. A read-only `doctor` (MED)

**Codex.** `codex doctor` (`codex:cli/src/doctor.rs:1-12`) checks the installation, configuration, auth, paths and
bounded reachability. Every check returns a redacted, serializable row, so one data set backs both the human summary and
`--json`. It is read-only by rule, so it is safe to run before filing an issue, and it never executes what PATH selects.
Checks live one per file (`cli/src/doctor/{network,disk,security,updates,…}.rs`). HIGH (read).

**Remit.** It has `/healthz` and the `health_check` read (`crates/wire/src/app.rs:2806`), which probes providers. It has no
local diagnosis of what ADR-0072 leaves unverified: UDP reachability of `/link` and `alt-svc` (P15, `docs/adr/0072-…md:402`),
certificate expiry and hash, data-dir permissions, which process holds the database lock, and the running build against the
installed one. HIGH.

**Proposal.** `remit doctor [--json]`, read-only, with one row per check as a typed `Check { name, ok, detail, fix }`. The
link probe dials `/link` with `link-quic` and reports `welcomed`, `refused(code)` or `unreachable`.

### D. Generated reference for the binary's settings (MED)

**Codex.** `config-schema` writes `config.schema.json` from the config types (`codex:config-schema/src/main.rs:1-7`,
`20-22`), so the settings' reference has one owner, the code. HIGH (read).

**Remit.** The flags are defined once in code (`crates/remit/src/flags.rs:71-150`, `f(name, kind, default, usage)`) and
again by hand in README's table (`README.md:84-115`). This session I checked that the 32 names agree. The texts do not: for
`-data` the code says "database and chain working directories" (`flags.rs:93`), while README says "the database, its backups
and the chains' working directories" (`README.md:85`). That is one concept with two owners (AGENTS.md). HIGH.

**Proposal.** `make gen` writes README's flag table from `flags::defs` between markers, and `check-gen` diffs it, as it
does for the TypeScript.

### E. Snapshot tests for generated output (LOW)

**Codex.** The TUI is tested with `insta` (`codex:tui/Cargo.toml:188`, 1,520 `.snap` files), and protocol output is
compared as whole trees with readable diffs (`schema_fixtures_tests.rs`, `similar::TextDiff`). HIGH (counted).

**Remit.** `insta` is not in `Cargo.lock` (0 hits). `tsgen`'s tests assert the emitted TypeScript by hand
(`crates/tsgen/src/tests.rs`, 183 lines). HIGH (counted). The replica is already tested at the right level.
`crates/replica/tests/replica.rs` (564 lines, 39 asserts) drives the core as a shell does, against a real in-memory link
session, and asserts what the shell would apply: patches, phase, answers and notices (`replica.rs:1-2`). It asserts through
projections such as `puts(&shell.patches, &Store::Chains)` (`replica.rs:245-260`). Snapshots would add little there. HIGH
(read).

**Proposal.** Use `insta` snapshots for `tsgen` only, one per supported serde shape (the closed set in
`crates/tsgen/src/attrs.rs`), so that a change to the emitter reads as a diff. Leave the replica's tests as they are.

### Where Codex's engineering is not better after all

- **Transport abstraction.** Codex has a channel per transport around one JSON message type
  (`app-server-transport/src/transport/mod.rs`). Remit's `link::Transport` trait over a sans-IO `Conn`
  (`crates/link/src/conn.rs:108-115`) is the cleaner seam. It already has three drivers (`link-quic`, `plugin-sdk/pipe.rs`,
  `memory`) and runs in wasm.
- **The experimental-API macros** (`codex-experimental-api-macros`, 310 lines, plus an `inventory` registry,
  `experimental_api.rs:22-27`). This is well engineered for clients released on their own cadence. Remit's web and app
  wrappers are lockstep, and its native peers get a contract number (idea 3), so the gate would add a mode without a user.
- **`tcp-tunnel`'s versioned NDJSON diagnostics** (`codex:tcp-tunnel/README.md`). This is good practice for a helper's
  stderr. Remit's `lines` relay is already typed and generated by `tsgen`, which is stronger.

## Where Remit is already better

- **Replay safety is a type, on every changing call.** `contract!` writes `request: Request` into every `changes` call
  (`crates/link/src/contract.rs:185`, `213`, `242`). Orch remembers answers for 7 days and answers a call cut off by a
  restart as `INTERRUPTED` (`crates/orch/src/request.rs:17-35`). Codex's `RequestId` is per-connection correlation only
  (`codex:rpc.rs:17-21`). Idempotency keys exist ad hoc on two calls (`v2/account.rs:436`, `v2/project.rs:92`), and the
  README tells clients "Do not blindly retry" (`README.md:103`). HIGH.
- **Every call states its kind and why a replay is safe** (`reads`/`changes`/`idempotent`, `contract.rs:27-34`), and a
  fitness rule checks it (`crates/fitness/src/contract.rs:1-12`). Codex classifies nothing. HIGH.
- **Questions to the owner are server-owned records.** Remit raises inbox items, which any channel answers, live or later
  (`wire::app::InboxKind`, `app.rs:86-95`). Codex sends approvals as server→client requests. With no capable client
  connected, "the command is rejected" (`README.md:10-15`), and pending ones replay per connection
  (`app-server/src/outgoing_message.rs:525`). HIGH.
- **Sans-IO protocol core.** `link::Conn` has no runtime, clock or socket, builds for wasm32 and natively, and is tested
  through `memory::Pair` (`crates/link/README.md`). Codex's transports are tokio tasks around channels
  (`transport/mod.rs`). HIGH.
- **Code generation that costs no build time and refuses unknown shapes.** `tsgen` reads source with `syn` and fails on an
  unsupported serde attribute with its `file:line` (`crates/tsgen/src/lib.rs:1-5`). Codex derives `ts-rs` and `schemars`
  only under `cfg(test)`, behind no-op macros, to save compile time (`codex:app-server-protocol/src/lib.rs:54-61`), and ships
  zstd-compressed precomputed exports (`schema/precomputed/`). HIGH.
- **A slow reader is never skipped silently.** Pushes are ordered per home, and the session closes `Behind`, after which the
  client resyncs (guarantee 4; `broadcast.rs:1-4`). On overload, Codex drops even the overload answer when the outbound
  queue is full ("dropping overload response", `transport/mod.rs:259-265`). Its in-process consumer queue is unbounded
  (`app-server-client/README.md` "Backpressure"). HIGH.
- **One answer type per call, checked by the compiler.** `Outgoing<Call, A>` and `Pending<A>::decode`
  (`contract.rs:42-111`). Codex reaches the same result with a parallel `ClientResponse` enum, plus a v1 variant inside the
  v2 payload (`common.rs:356-372`). MEDIUM.
- **Seams for a network of nodes are already fixed.** Node identity, capability grants, `home` routing, and the `FORBIDDEN`
  and `NOT_HERE` codes are in place (`envelope.rs:64-90`, ADR-0072 decision 12). Codex has no peer model. HIGH.
- **Stricter lints** (idea 10). Codex's `test-binary-support` uses `unsafe { set_var }` and `panic!`
  (`codex:test-binary-support/lib.rs:46-60`), which Remit's workspace forbids. HIGH.

## Not worth taking

- **JSON-RPC framing.** Codex does not follow it either ("We do not do true JSON-RPC 2.0", `rpc.rs:1-2`).
- **v1 and v2 side by side, with legacy fields kept for older clients** (`common.rs:356-372`, `README.md` "Legacy
  compatibility field"). Remit's runtime code reads one shape (AGENTS.md *Leave nothing behind*).
- **Experimental gating per field.** Remit's peers are lockstep or carry a contract number (idea 3), so a gate would be a
  flag in disguise.
- **The SQ/EQ in-process protocol** (`docs/protocol_v1.md`). Remit's runtime contract already covers this, both ways
  (`crates/wire/src/runtime.rs:394-430`).
