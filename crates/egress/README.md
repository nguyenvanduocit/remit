# egress

Decides WHERE a program may connect through Remit, and relays the connections that pass (ADR-0018, forms T1 and T2).
It listens on a port of 127.0.0.1 that the program's proxy variables name; a program can go around it (ADR-0138), so it
records calls and adds credentials rather than walling a program in.

- `Gate` (`egress`, `forward`, `tunnel`): an HTTP forward proxy on a loopback port (CONNECT for HTTPS, an absolute URI for
  plain HTTP) that connects a program only to the `host:port` pairs its caller listed, refuses names that resolve to this
  machine or its network, and bounds every tunnel by a deadline, a byte cap and a cap on tunnels open at once.
- `Injector` (`inject`, `inject_cert`, `inject_exchange`): the Gate of form T1. It serves the connections of one run (an
  `Upstream` each, picked by the host and port a request names), ends the program's TLS with a certificate made in the run
  for that connection, admits reads, or writes where the caller grants them, adds that connection's credential and
  scrubs it from every answer.
- The decisions are pure and apart from the I/O: `policy` (targets, the allowlist, `routable`), `inject_rules` (what a
  request may be), `denied` (the bounded record of refusals).

The shell runs on tokio and hyper (server and client over HTTP/1.1), rustls with ring (TLS both ways), rcgen (the run's
name-constrained authority) and rustls-native-certs (the system's roots when the caller gives none).
