# wire

The one definition of what crosses the wire (ADR-0072 decision 2), compiled into the server and every client core. A
type `model` declares (a record such as `Chain`, `Card`, `Edge`, `Decision`, `Event`, `InboxItem`, `Draft`, `Landing`,
`Release`, and each string enum a record holds) crosses as itself, never as a copy here (ADR-0107); a value the server
works out for a record rides on a read type that holds it (`app::ChainRead`, `app::DecisionRead`, `app::EventRead`,
`app::LandingRead`), never flattened into it:

- `app`: the owner's channels and the server.
- `runtime::to_core` and `runtime::to_runtime`: the AI runtime and the core, both ways.
- `worker`: a mentor's device and the core.

Each is a `link::contract!`: its calls (`reads` with why a replay is safe, `changes` taking a `Request`, `idempotent` with
why their own ids make a replay safe) and its pushes. Payloads serialize with serde to the field names the Vue shell and
the stored records use (camelCase; `Option` fields of `?` are left out when absent, those of `| null` are sent as
null). The TypeScript generator reads this crate's source.
