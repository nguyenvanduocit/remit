# client-web

The client core in the browser (ADR-0072 decision 10, P5): `replica` over `link`, bound with wasm-bindgen. One `Core`
serves every tab of the browser from a SharedWorker (ADR-0095 decision 4). It takes each tab's asks (`Ask`), steps
`Replica::handle` with `Date.now()`, carries out transport ops on the browser's WebTransport (moq-dev's `web-transport`
0.13.9) and its one timer on `setTimeout`, and tells each tab a `Told[]` per step (the models it shows and their row
changes, state, answers, notices). Both cross as plain objects through `serde-wasm-bindgen`'s `json_compatible`
serializer (decision 5); `useModel` in `web/src/core/client.ts` keeps each model's rows. Inputs that arrive during a step
queue behind it, so the core is never entered twice.

- `worker.rs`, `Tabs`: the SharedWorker's entry. `web/src/core/worker.ts` hands it every port that connects. What a
  tab says on its port is `FromTab` (`port.rs`, generated for the page into `web/src/core/generated/client_web` by
  `make gen`): its hello (`start`, read only for the first tab; the credential the next dial carries; the name of the
  Web Lock it holds while open), then its asks, then `bye`. A tab that goes without saying so releases its lock,
  and the worker, waiting for that lock, lets the tab go (`Input::Left`). The page's side is `overPort` in
  `web/src/core/client.ts`, which sends the hello once it holds its lock.
- `shell.rs`, `Core` and `Tab`: the core and one tab's handle on it; dropping a `Tab` lets the tab go. A tab is told
  through its port, or through a function when the core runs in the page itself (`Core.fake(start, server).tab(sink)`,
  the tests).

With the `fake` feature the module also carries `FakeServer`: a real `link::Conn` accepting the client's, admitting it to
the app contract and answering calls from a JS function. vitest runs the real core with it in the page; the verify pages
run it in their own SharedWorker (`Tabs.fake`, `web/verify/worker.ts`), so two tabs of a verify page share one fake
session as two tabs of the app share one WebTransport.

Builds: `bind.sh` (wasm32 release, then `wasm-bindgen` into `web/src/core/pkg/{web,node,fake}`, printing the gzipped
size). WebTransport and Web Locks in web-sys need `--cfg=web_sys_unstable_apis`, which `.cargo/config.toml` sets for
wasm32. A native build without `fake` holds only `FromTab`. The crate compiles under the workspace's
`forbid(unsafe_code)`.

## Tests

- `web/src/core/client.test.ts`: the core built for Node with the fake server, one tab in the page. A snapshot and then a
  push reach the rows of a model the page draws (a stale copy is dropped, nothing reaches it once hidden), and a
  changing call is answered carrying the core's request id.
- Two tabs sharing the core are tested in `replica` (`two_tabs_open_two_chains_and_each_is_told_only_its_own`).
