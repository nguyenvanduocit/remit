# link

The protocol every Remit peer speaks (ADR-0072 decisions 2, 3 and 12): the envelope (`envelope`), `contract!`, and `Conn`,
one session's sans-IO state for either half. Pure: no runtime, no clock, no socket; builds for `wasm32-unknown-unknown`
and natively. `memory::Pair` wires two halves back to back for tests and fake shells.

## Shape

- `Conn::dial(cfg, hello)` / `Conn::accept(cfg, node)`; the acceptor gets `Event::Hello` and answers `admit(granted)` or
  `refuse(refusal)`.
- In: `receive(stream, bytes)`, `written(stream, n)`, `closed(closing)`, `tick(now)`; calls with `call(now, outgoing)`,
  pushes with `push(now, home, &push)`, answers with `reply(incoming, result)`.
- Out: `drive(&mut transport)` hands the `Transport` what to open, send, finish, reset or close, and returns the `Event`s.
  `next_deadline()` says when to `tick` again.

## How a driver maps WebTransport onto it

| link | `web-transport` |
|---|---|
| `StreamId { opener, kind: Bi, n: 0 }` of the dialler | the first `open_bi` of the dialling peer: the control stream |
| `Transport::open` with `kind: Bi` | `session.open_bi()`; remember the pair under the id |
| `Transport::open` with `kind: Uni` | `session.open_uni()` (one per push home, plus the feed) |
| `Transport::send` / `finish` / `reset` | write the bytes / finish the send side / reset the stream |
| `Transport::close(closing)` | `session.close(code, reason)`; `Closing::Refused` and `Closing::Behind` each get a code the other side maps back |
| a stream from `accept_bi` / `accept_uni` | the next `StreamId { opener: <peer>, kind, n }`, counted per peer and kind in accept order |
| bytes read on a stream | `receive(stream, bytes)`; bytes flushed by a write: `written(stream, n)` |
| `session.closed()` resolved | `closed(<the closing its code names>)` |

Frames are a 4-byte big-endian length and JSON. Datagrams are not used. Stream numbers are the link's own: a driver keeps
its map from them to its stream handles, so both peers name a stream the same whatever the transport numbers it.

## The guarantees

1. Every call is answered or fails typed: `TIMEOUT` at its deadline (a late answer is dropped), `DISCONNECTED` when the session
   ends or there is none, `INVALID_ARGUMENT` for what does not decode, the handler's own code otherwise. What a caller acts on
   beyond the code is the failure's `detail`, typed by its contract (`model::FailureDetail` for `wire::app`, read with `detail_as`).
2. A changing call carries a `Request` by type; retried after a reconnect with the same one, the callee acts once.
3. A peer's calls of one line run one at a time in the order sent (`seq` per line), whatever order their streams arrive in: a
   line is the changes naming one entity (`order = <arg>` in `contract!`: a chain's pause, resume, finish), else one method.
4. A push is written in order on its home's stream or the session ends `Behind`: never skipped.
5. The acceptor reads nothing but `hello` until it admits; a refusal says why; a call outside the grant is `FORBIDDEN`.
