# model

The records Remit stores and sends to its channels: chains, cards, events, decisions, signals and subscriptions,
objectives, employees, the library, the inbox. A leaf crate: it depends on no workspace crate and does no I/O.

Every record is exported from the crate root (`ChainStatus`, `Chain::open_mrs`, the variant `ChainStatus::Running`).

A record writes every field, and a field that may be absent is an `Option`, left out when it is none (ADR-0107): a time
that may be unset is `Option<Time>`. A key a document lacks reads as the field's default. A document is written by
`serde_json`, and one an earlier build wrote (Go's escapes and float form included) reads. `stored_json` holds the forms a
value takes in every document and contract: `Time` (RFC 3339 text, offset kept), `Duration` (signed nanoseconds as an
integer) and `ForeignJson` (JSON another system defines; postcard carries it as text). A string enum
(`text_enum!`) reads only the values it names, and a field that may have none is an `Option` of it: a record holding a
value this build does not name fails to read, and the store leaves it out, untouched, and counts it (ADR-0107).
`tests/stored_json.rs` reads synthetic samples (`tests/fixtures/stored_records.json`) and checks each is written back
with every value it held.
