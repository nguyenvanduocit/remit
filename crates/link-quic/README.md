# link-quic

The link's driver over WebTransport (ADR-0072 decision 3): `web-transport-quinn` on tokio, for the server (`Listener`)
and every native peer (`dial`, `client(Trust)`). Sessions open at `/link`. Each session runs one task that owns its
`link::Conn`, and each stream runs one task that reads it and writes what the conn hands it, reporting flushed bytes so the conn can
tell a peer that falls behind. `Link::call` awaits a typed answer, `Link::push` pushes on a home's stream, and `Peer::events` carries
hello, welcome, the peer's calls (answered with `Link::reply`), pushes and the closing. The close code carries why
(`done`, `behind`, `broken`, `bad_credential`, `new_bundle`), so the peer learns it even when the frame that said so was
lost with the connection. Crypto is `ring`, as in `egress`.
