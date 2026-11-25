# tsgen

Writes the TypeScript of one module of a crate from its source (ADR-0072 decision 2):

    tsgen <src dir> <module> <out dir> [<src dir of a crate it reaches>...]

writes `types.generated.ts` (every type and constant the module declares, and every type they reach in another module
or in one of the crates given after) and, when the module has a contract, `api.generated.ts` (per contract: `CallArgs`
and `CallAnswers` by method name on the wire, the `Send` the core implements, `createApi(send)` with one async function
per call, minus the `Request` the core adds, and `PushArgs`). A crate is named as its directory is, `-` written `_`; a
path into a crate not given fails as not found. A path follows `use` items, globs (`pub use account::*`) included.
A contract in a submodule takes its path as a prefix: `runtime::to_core` writes `ToCoreCallArgs`, `createToCoreApi`.

Its runs, each reading `crates/model`, whose records the contracts carry (ADR-0107):

    tsgen crates/wire/src app web/src/core/generated crates/model/src
    tsgen crates/replica/src shell web/src/core/generated/replica crates/link/src crates/wire/src crates/model/src
    tsgen crates/client-web/src port web/src/core/generated/client_web \
        crates/replica/src crates/link/src crates/wire/src crates/model/src

The second is the client core's vocabulary for the page (`Patch`, `State`, `Notice`, `Ask`, `Told`); `make check-gen`
fails when the web app's copy is not what it writes.

It reads serde's attributes within a closed set (`rename`, `rename_all`, `rename_all_fields`, `tag`, `content`,
`untagged`, `into` with `from` or `try_from` naming the same type, `skip_serializing_if = "Option::is_none"`, `skip`,
`default`, `flatten`); a container's `rename` is its TypeScript name. It reads `model`'s `text_enum!`, `json_tagged!` and
`json_paired!`. Any other attribute, another macro, a shape JSON cannot say plainly, or a hand-written `Serialize` or
`Deserialize` of a type it writes, in whatever crate it is reached, fails with its `file:line`; the one hand-written serde
it writes is a newtype over `serde_json::Value` (`model::ForeignJson`), `unknown` whatever form it takes on the link. A
module is read only when it is asked for or reached, so `plugin` (host and plugin only) is never written.

`Option` with `skip_serializing_if` is an optional field (`x?: T`); without it, `T | null`. An enum of unit variants is a
union of strings; with `tag`, a discriminated union. A `text_enum!` is the union of its texts. A struct's enum field
`json_paired!` writes as two keys is an intersection with the union of the pairs (`model::Event` is its fields `&
EventStatus`, each kind with the statuses of its own). A type with `into = "T"` is
written as `T`: `model::Duration` (`into = "i64"`) is `number`.
