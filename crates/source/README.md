# source

How an agent reads the outside world: one Provider per source, each with its own read-only guard, and one gate (ADR-0018)
every call passes: resolve the connection, ask the Policy, act under a timeout, remove secrets, name the connection, cut
to `OUTPUT_MAX`, tell the Recorder once.

## Sources

| Source | Module | Read-only guard | Driver seam |
|---|---|---|---|
| StarRocks | `starrocks.rs` | `sqlguard::read_only_sql` on every statement | `RowsProvider` |
| MongoDB | `mongo.rs`, `mongo/` | `mongoguard` on filter, projection and pipeline | `MongoClient` |
| Bytebase | `bytebase.rs`, `bytebase/` | `bytebase/guard.rs` (per engine), `bytebase/mongoshell.rs` | HTTP |
| Grafana | `grafana.rs`, `grafana/` | GET-only API, reads through the proxy | HTTP |
| Coroot | `coroot.rs`, `coroot/` | GET-only API | HTTP |
| CloudWatch Logs | `cloudwatch.rs` | read actions only; the `aws` program form is checked by `ProgramSpec` | `LogsApi` |
| Datadog | `datadog.rs`, `datadog/` | read endpoints only | HTTP |
| Extensions | `extension.rs` | the manifest the owner applied, pinned by sha256 | `Plugins` (call, poll) |

## Dependencies

| Crate | Version | Features | Why |
|---|---|---|---|
| grant | path | | the decision over a chain's grants |
| tokio | 1.54.1 | rt, time, sync, macros | async (ADR-0072 decision 5) |
| reqwest | 0.13.5 | rustls-no-provider | REST sources; the rustls config is built here |
| rustls | 0.23.46 | ring, std, tls12 | TLS provider for reqwest and the roots below |
| rustls-native-certs | 0.8.5 | | the system's root certificates |
| mysql_async | 0.37.1 | minimal-rust (no TLS) | StarRocks speaks the MySQL protocol; connections are plain (no TLS) |
| mongodb | 3.10.0 | compat-3-3-0, bson-3, rustls-tls, dns-resolver | MongoDB (`mongodb+srv` needs the resolver) |
| bson | 3.1.0 | serde, serde_json-1 | extended JSON, parsed with key order kept (sort and pipeline stages) |
| aws-sigv4 | 1.6.5 | sign-http, http1 | signs CloudWatch Logs calls and the `sigv4` extension scheme |
| aws-credential-types | 1.3.1 | | the key the signer takes |
| aws-smithy-runtime-api | 1.21.0 | client | the signer's identity type |
| hmac | 0.12.1 | | the `hmac-query` extension scheme |
| sha2 | 0.10.9 | | script ids, the hmac schemes |
| base64 | 0.22.1 | | Basic auth |
| hex | 0.4.3 | | hash text |
| http | 1.5.0 | | request parts the signers edit |
| url | 2.5.8 | | base URLs and paths |
| jiff | 0.2.42 | | time, RFC 3339, rounding |
| schemars | 1.2.3 | | tool input schemas |
| serde / serde_json | 1.0.229 / 1.0.154 | derive | the JSON shapes of each source's answers |
| regex | 1.13.1 | | token shapes, names |
| tempfile | 3.27.0 | | the scratch directory of a program or script |
| thiserror | 2.0.21 | | error types |

## Design notes

- **CloudWatch Logs without the AWS SDK.** The calls are JSON 1.1 requests (`X-Amz-Target`, `application/x-amz-json-1.1`)
  over reqwest, signed by aws-sigv4. `aws-sdk-cloudwatchlogs` (ADR-0072 P12) builds a large dependency tree on a disk
  that was close to full; the five actions this source calls (DescribeLogGroups, FilterLogEvents, StartQuery,
  GetQueryResults, StopQuery) do not need it. What the SDK added is kept: a throttled request, a 5xx or a broken
  connection is sent again (3 attempts, jittered doubling wait), and an Insights query whose call is dropped is stopped
  on the runtime. Revisit when another crate pulls in the SDK anyway.
- **No MCP server here.** `Set::serve(caps)` returns `ServedTool`s (name, description,
  input schema with the `connection` property, a `call`), and orch registers them in rmcp.
- **The Caller is a parameter**, not a value carried in a context.
- **Policy answers `grant::Decision`** (Allow, Deny, Ask); there is no verdict without a kind.
- **A Provider closes when dropped**, so a connection the cache lets go is closed with it.
- **Plugins** are reached through the crate's narrow `Plugins` port (call, poll). The host implements it
  over `plugin-sdk`'s `Plugin` trait.
- **Lists:** `null` is accepted on read and `[]` is written for an empty list.
- **Numbers:** serde_json without `arbitrary_precision` (feature unification would change other crates). A whole float is
  written as an integer; an integer above 2^53 inside a decoded document keeps its digits as an
  `i64`/`u64`. A document Bytebase returns is rewritten through `f64`, so `1.10` reads `1.1` and an integer beyond 64 bits
  loses digits.
- **JSON null reads as the zero value** through one deserializer (`null_json::null_as_zero`) on every field
  of a decoded answer.
- **StarRocks: one statement per call is held by the guard alone.** mysql_async
  0.37.1 always offers `CLIENT_MULTI_STATEMENTS` (`opts/mod.rs:1158`) and exposes no way to clear it or to
  send `COM_SET_OPTION`, so `read_only_sql` (which refuses anything after a `;` but comments) is the one check. Revisit
  when the driver lets the flag go, or move StarRocks to a driver that does.
- **MongoDB collection lists** are asked for in full. The driver sends `nameOnly` only from `list_collection_names`,
  which drops the type that tells a view from a collection.
- **Coroot sign-in** follows a redirect to the same host; the password never leaves that host.
- **Bytebase's MongoDB shell parser is strict**: `db[` needs a real quote, `\u` takes no sign, and text after a NUL
  character is refused.
- **Quoting in error texts** is Rust's `{:?}`. Labels an agent reads in an answer use `text::go_quote`, which escapes a
  control character as `\xNN` and a non-printing one as `\uNNNN`.
