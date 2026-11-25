# rpc

The owner's channels' adapter onto the orchestrator (ADR-0072 decisions 2, 3 and 12). A session on the link whose hello
presents the owner key, and runs the bundle this server serves, is granted `wire::app`: `Api` answers its calls as that
owner through the orchestrator's application layer, and `Broadcast`, the orchestrator's `Notifier`, pushes it every change
from the moment it came in, each on its home's stream (`wire::app::Push::home`).

## Shape

- `Api` (`src/api.rs`): `wire::app::Handler` over `orch::Orchestrator`, as one signed-in owner. A changing call goes to
  the orchestrator's action that takes an `orch::Request` (each runs through `act`/`once`, so a replay acts once); a
  record or view crosses as itself, typed on both sides (ADR-0107); an `orch::Error` crosses as its `ErrorKind`.
- `Broadcast` (`src/broadcast.rs`): one queue per session that holds each record's last unsent push (`src/pending.rs`),
  and what streams in a home only for the sessions showing it (`homes_shown`), each home named marked on its stream
  (`home_shown`) for the client to read it from. A session with `BEHIND_BY` records
  waiting is closed, and its client reconnects and reloads its snapshot. How fast a peer reads is the link's to watch
  (`behind_bytes`).
- `Server` (`src/serve.rs`): `serve(hello, peer)` for each `link_quic` session whose hello asks for `wire::app`. It
  refuses `bad token` (no gate, no credential, a wrong one) or `new bundle`, else subscribes, admits and serves until the
  session closes. Wiring the listener into the `remit` binary is the binary's.

Shared with every channel on the link: `session::run` (one admitted session's calls answered and pushes sent until it ends)
and `convert` (`returned`, `failed`, `done`: what orch returned as the answer, a failure as its kind's code with its
`model::FailureDetail`). Every event answered or pushed is `wire::app::EventRead`, built by `orch::event_read` with what
the server types it as.
