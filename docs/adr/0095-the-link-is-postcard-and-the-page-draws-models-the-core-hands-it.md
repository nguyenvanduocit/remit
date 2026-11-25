# 0095. The link is postcard, the core runs in a Worker, and a page draws the model the core hands it

Status: accepted, 2026-11-13; decision 3 amended by ADR-0107 (`model`'s records become link types). Amends ADR-0072 (frames were length-prefixed JSON, decision 10's client core on the UI
thread handing the page JSON) and ADR-0094 (the outbox stays in the core; only where the core runs moves).
Source: the owner, 2026-11-13 (translated): "WebTransport carries binary, so use binary"; his choice of postcard over
MessagePack; and the path he set: **server ⇄ postcard ⇄ WASM core in a Worker ⇄ model objects by structured clone ⇄
`useModel` (shallowRef) → Vue component**, with no second store in the page ("drop Pinia: it would only duplicate the
core").

## Context

An answer was decoded three times, all on the UI thread: a JSON frame into `serde_json::Value` (`link::envelope`), the
`Value` into its type (`from_value`), the type back to a JSON string for the page (`client-web` `shell.rs`), then
`JSON.parse` and a patch loop into reactive Maps (`web/src/core/client.ts`, `stores.ts`). The page so held a second copy
of the core's records. Measured on 10,000 records of the dev database (a probe outside the repo, decoding into generic
values, so approximate): MessagePack is 94–97% of JSON's bytes and decodes 1.5–3× faster; CBOR is no faster. The bytes
are mostly text, so the gain is in decoding once, not in size.

Both ends of every session are one build: `rpc` refuses a client of another bundle (`crates/rpc/src/serve.rs`,
`NewBundle`), and a plugin of another contract version (`crates/plugin/src/start.rs`, `admission`). A format that names
no field costs nothing here.

## Options

1. **Keep JSON.** No work; three decodes and a JSON copy in the page stay.
2. **MessagePack.** Self-describing, so `wire` stays as it is; it keeps field names on the wire, a half step a typed
   format would throw away. Built and passing for `link`, then dropped for 3.
3. **Postcard.** Smallest, decodes straight into the type; not self-describing, so a type on the link has no `flatten`,
   no `skip_serializing_if`, no internally or adjacently tagged enum and no `serde_json::Value`. **Chosen.**

For the page: (a) keep the JSON batch and the page's Maps; (b) the core keeps the data and hands each page part the model
it draws, as a structured-clone object. **(b), the owner's call.**

## Decision

1. **Frames are postcard** behind the 4-byte length. `Frame` is externally tagged with every field written.
2. **A body is `link::Payload`**, postcard bytes the link carries unread. A call is its `method` (a push its `name`) in
   the frame beside its **arguments as one postcard tuple, in declared order**; `contract!` writes `encode_args` and
   `decode_args` per call and push, so `Call` and `Push` keep their JSON shape for whatever reads them as JSON, and the
   link never depends on it. `serve(handler, method, args)` decodes the arguments of that method only. An answer is the
   answer type in postcard. `Failure.detail` is a `Payload` of the contract's `FailureDetail`, read with `detail_as`.
   `link` no longer depends on `serde_json`.
3. **Types on the link are postcard-safe, and their JSON does not move.** The same types are read as JSON by the page,
   the store and the CLI, so each keeps its JSON and gains a postcard form:
   - a field `skip_serializing_if` leaves out is written on the link as postcard's empty value (`link::written`, a
     serializer wrapper that writes what `skip_field` skips): it reads back as `None` or `""`; JSON keeps leaving it out;
   - an internally tagged enum is externally tagged for postcard and keeps its tag field in JSON (`wire::tagged`,
     `json_tagged!`, branching on `is_human_readable`); the generated TypeScript is unchanged. Its JSON passes through
     `serde_json::Value`, whose keys sort (the workspace does not enable `preserve_order`), so the tag is no longer
     the first key: readers look keys up by name, and the order is the same on every run. The one reader of the text
     itself is the request hash of an idempotent act (`orch/src/client_tools.rs:666`, `orch/src/worker.rs:364`): an
     act asked before this build and replayed after it with one of these enums in its arguments hashes differently,
     and is refused as another action under the same id (`orch/src/request.rs:231`), never done twice;
   - `runtime::SessionEntry` holds its body as a field, not `flatten`;
   - a `serde_json::Value` on the link is `wire::ForeignJson`, JSON another system owns (a plugin's JSON Schema, a tool's
     arguments), carried as text on the link and as the same JSON elsewhere.
   A test encodes one value of each such type with postcard and reads it back equal (`crates/wire/tests/round_trip.rs`),
   so a type that breaks it fails in `make test`, not in a browser. A predicate other than `Option::is_none` or
   `String::is_empty` would read back changed: the test catches it.
   A hello that does not decode is a peer of another build (a page left open across this change, a plugin built before
   it): the acceptor closes with `new_bundle`, which no frame format hides, so the page loads itself again and a plugin is
   told to be built again. `PLUGIN_CONTRACT` is 2.
4. **The core runs in one SharedWorker per browser** (Chrome and Firefox; Safari is not a target, owner 2026-11-13). `client-web` exports a shared-worker entry; WASM, WebTransport, the
   timer and the replica live there once, however many tabs are open: one session, one decode, one outbox (ADR-0094).
   Each tab is a port with its own shown models and its own `document.hidden`; the session stays while any tab shows
   something and closes with the last. Proven by P-SW below.
   As built (step 5.2): `replica` serves tabs (`Input::Ask { tab, ask }`, `Input::Left { tab }`, `Output::Tell { tab,
   told }`). Shared: the session, the catalog, the Assistant, drafts and typed words, the outbox. Each tab's own: the
   chain it opens and its window on each card's events (`opened.rs`), its open event and archived pages (`core.rs`
   `Page`), the models it shows, its places' retries and whether the owner sees it (`Ask::Hidden`). A chain's view and
   its events are held once however many tabs show it (`held.rs`) and told to every tab that shows that chain: a tab
   opening a chain another tab shows is handed what is held, with no read; a chain held by none is read once for every
   tab that opens it meanwhile; it is let go when no tab shows or opens it. What is held is handed on only while it is
   live, pushed to the session since its read: a chain that left every tab's homes, a reconnect and the home's reset
   read it again. A tab's card windows start on the chain's events held and grow back on their own: paging back in one
   tab leaves another's as it was. A model is kept in one of three slots (`models.rs` `Owner`): once for every tab,
   once per chain for the tabs that show it, or in a tab's own and told to it alone. The homes named to the server are
   the union over tabs. An answer goes only to the tab that asked; a notice about what a tab asked (a chain, a stream, an event,
   the archived pages) goes to that tab, where duels stand to every tab (a page reads them again), and the rest (a chain
   done, a turn finished, an inbox item, a draft not saved) to the tabs the owner sees, or to every tab when he sees none.
   What to keep across a reload goes to every tab, each keeping it in its `sessionStorage`; a tab that joins is told it
   too, and a restore of rows held already is dropped, so two tabs restoring one send send it once. A tab is served from
   its first ask; the session closes when the last leaves, and no tab left means no dial.
   `client-web` exports `Tabs` (`worker.rs`), the SharedWorker's entry: `web/src/core/worker.ts` hands it each port
   that connects. What a tab says on its port is `FromTab` (`port.rs`, generated for the page): its hello (the
   page's start, read for the first tab only; the credential the next dial carries; the name of a Web Lock it holds),
   then its asks, then `bye`. The page sends the hello once it holds its lock (`client.ts` `overPort`): the worker
   waits for that lock and lets the tab go when it is released, so a closed, reloaded or crashed tab goes as surely as
   one that says `bye`; a tab gone, its port is closed and nothing in the worker holds it. The verify pages run the
   same `Tabs` over the fake server (`Tabs.fake`, `web/verify/worker.ts`), one worker per verify page, whose seed
   answers come from the oldest tab still open over its port; tests run the core in the page (`Core.fake(start,
   server).tab(sink)`).
5. **The page draws models.** The core keeps the records (not only the facts its rules read) and hands each tab the
   models it shows, made by a pure function in Rust from the replica and the tab's local state (window, open item).
   Rules and derived views stay the server's (AGENTS.md, *Server-driven*): a model selects and shapes what the server
   laid out, it decides nothing the server owns. Page → core: `Ask` (show a model with its local state, change the local
   state, hide it, act); core → page: `Told` (a model's rows, an act's answer or failure, the connection state). Both
   cross as structured-clone objects (`serde-wasm-bindgen`), never JSON strings.
   A model crosses as rows keyed by record id: whole when its tab shows it, then only what changed (a row put or
   dropped, its place, text appended to a row), and nothing to a tab that does not show it. A transcript streaming
   text so re-renders the one row that grew, and no clone copies the rows that did not change: a model sent whole on
   each change would clone the open chain's 600 events on every chunk and hand Vue 600 new objects to diff.
   As built (step 5.1): a model is `replica::Model`, one per kind of record the core keeps (the open chain's events,
   one card's events, the board, …), named in the page as `{ model: 'chains' }` or `{ model: 'cardEvents', card }`.
   The rules still say each change as a `Patch` to their `Index`; `replica` `models.rs` applies it to the records it
   keeps (by model, in order, an `IndexMap`) and turns it into what a shown model is told: `Told::Model` whole on
   `Ask::Show` and when a model is read again (a reset, a card's stream forgotten on reopen), `Told::Rows` with
   `Change::Put` (a whole row, `before` only for a new one), `Append` (the chunk alone) and `Drop`, and `Told::Status`
   when its freshness moves. `client-web` crosses `Told[]` and `Ask` with `serde-wasm-bindgen`'s `json_compatible`
   serializer, so the page gets plain objects, numbers and `null` as the JSON it read before; an integer past 2^53 in a
   record fails the batch, which JSON had rounded silently. Measured in the verify app: a streamed chunk changes one
   feed run of 102 and every run keeps its DOM node.

### Long sessions

The owner stays in the app for days, and a chain's transcript grows without bound: the busiest chain on the dev database
holds 1,343 events and 1.7 MB of text, and opening a chain reads up to 20,000 events in one answer
(`crates/orch/src/views.rs:158-161`, `EVENTS_SHOWN`), with older ones out of reach (`EventPage.truncated`, no cursor). The
owner's session is pushed every change of every chain, every streamed text delta included
(`crates/rpc/src/broadcast.rs:1-3`), and a session 512 changes behind is closed and reloads its whole snapshot
(`BEHIND_BY`, `broadcast.rs:26`). So what is sent, held and verified is bounded by what is on screen, not by how long the
owner has been there.

6. **A transcript is a window, paged by cursor.** Opening a chain answers its newest `CHAIN_EVENTS_SHOWN` (600)
   events, what its Live feed shows; an agent's stream answers pages of `CARD_EVENTS_PAGE` (1,000) and reads the page
   before with `card_events(card, before)`, its oldest event's id as the cursor (both in `wire::app`). The chain's own
   answer has no cursor: no page reads a chain's past across its agents, so none is built until one does. The core
   keeps the open chain's newest `CHAIN_EVENTS_SHOWN` as more are pushed, and each agent's stream one page over the
   pages the owner read back (`replica/src/held.rs`, `opened.rs`, `Index::keep_last`); an agent whose oldest were let go offers
   its earlier page again. A feed counts what arrived while the owner reads above by event id, not by length
   (`useStickToBottom`), so a window that drops its oldest still counts. The core's memory is so bounded by the open
   chains' windows and the pages asked for, not by the transcript's length. A window lets go of its oldest under an
   owner reading at its top; keeping what a tab shows is step 5's, where each model carries its tab's window.
7. **The server pushes a home's stream only to the sessions that show it.** A home is what a push stream carries (a
   chain, the Assistant). The core names the homes it shows (`homes_shown(homes, seq)`, the whole set, so a replay
   sets the same; `seq` counts the session's tellings): the chain open or being opened and the Assistant's home. The
   pushes that stream while agents work (`event_*`, `card_shown`) go only to the sessions showing their home; every
   other push (the board, the inbox, where each chain stands) goes to every session its audience reaches, and a session
   that never named a home (`remit-client watch`) is pushed them all. The rule is `Broadcast`'s (`Shown`,
   `Push::streamed_home` in `wire`), beside the mentor's audience, and `Broadcast` still turns each change into its
   push once.
   A session naming its homes lets go of what waits for the homes it no longer shows (`Pending::keep_homes`), then
   pushes each home named `home_shown { home, seq }`, on that home's stream after everything sent for it before
   (`rpc` `broadcast.rs` `Showing::set`). The core reads a home it names anew once that mark of its telling (or a
   later one) comes, and drops what streams in the home before it (`replica` `core.rs` `show`, `home_shown`, `held.rs` `read`): a push
   sent before the server learned that the session had let the home go would land on the read and undo what came
   after it (a record deleted meanwhile put back), and a home's stream is in order (decision 9), so what follows the
   mark is current and none of it is missed. A home it named already is read at once. The mark goes to every home
   named, not only those new to the server, so the core alone decides which it waits for; a telling that fails brings
   none, and the homes waiting for it are read then (the session was never narrowed, or is gone). The seeded test
   found both windows: a push held for a home let go, and one already on its way. A session's calls are served at once, so
   the server applies no telling older than the last it applied, and the core's `seq` goes on across sessions; a new
   session has been told nothing, so the core names every home anew on it and reads each from its mark.
8. **Conflation, no queue that grows.** A session holds at most one unsent push per record (`rpc/src/pending.rs`): a
   newer one replaces it and goes to the end, so what is sent is each record's last state in the order of the last
   changes, and text an event streams merges into what waits for that event (a chunk that does not follow is left, as
   the client leaves it). A merged push can so start inside text the core read meanwhile: the core keeps, of two
   copies of one revision, the longer, and of a chunk, what passes the text it holds (`replica` `events.rs`). A slow or
   hidden session gets fewer, larger pushes; the server's memory per session is bounded by the records that changed,
   never by time. A session with `BEHIND_BY` (512) records waiting is still closed
   whole; with repair per home (decision 10) it repairs that home alone.
9. **Within a session a home has no gaps by construction; versions wait for resume.** The link sends a home's
   pushes in order on that home's stream, and a push that cannot be sent closes the session (`rpc` `session.rs`):
   nothing is lost inside a session, and a new session reads every home it shows again. So pushes carry no `epoch`,
   `version` or `digest` today: a home shown again is read from its `home_shown` mark (decision 7), which order on the
   stream makes enough. They earn their place when a reconnect resumes a home from the version it holds instead
   of reading it again; that resume, the per-home digest that checks it (the wrapping sum of
   `xxh3(id ‖ postcard(record))` over the home's view records, the same bytes on both sides) and the check that each
   push follows the last are its seam, and the seeded test (step 6) compares the core's records with the server's
   directly meanwhile.
10. **Repair is per home.** A session whose unsent records pass `BEHIND_BY` lets go of what streams in the home with
    the most of them and is pushed `home_reset { home }` in their place (`rpc` `pending.rs`); its core reads that home
    again at once (it names it already) and applies what came meanwhile over the answer. A `home_shown` waiting for the
    home is kept ahead of the reset, so a core waiting for it reads the home from it, then again on the reset. Other
    homes and the board are untouched. Only when what waits is not a home's stream (the board, the inbox) is the session closed whole.
11. **Freshness is part of every model.** A model's `status` is `Live { as_of }`, `Stale { since, reason }` (offline,
    behind) or `Repairing`, never a flag beside it; the page draws each the way ADR-0094's banner says it, per model.
    As built (step 5.1, `replica` `fresh.rs`): a model's freshness is its home's, by what it is read with (the snapshot;
    the open chain; the Assistant's stream; the open event; the core's own `typed` and `outbox`, always live). No
    session is `Stale { reason: offline }` and a failed reread `Stale { reason: behind }`, both since the link went
    when it did (else since the step it began); a snapshot on its way, before the first one or after a reconnect, is
    `Repairing`, and so is a home being read while the rest is live (a chain opening or reset, the Assistant's stream
    read again, an event opening). `Live { as_of }` dates from the step it went live. `ModelStatus.vue` draws it above
    the chain's Live feed and an agent's activity: nothing while live, "Bringing this up to date…" while repairing,
    "Shown as of {time}" while stale.
12. **What happens on the link is counted.** The core counts, per minute, each answer's and push's bytes and decode
    time by name (timed on the clock its shell gives it, `performance.now()`), the homes read again by reason (`reset`:
    the server let go of the stream; `decode`: a push this build could not read) and how long after an event's
    `created_at` the core held it, and sends that as `link_report` (`replica` `tally.rs`), an idempotent call named by the
    first tab's salt and the minute. The server keeps it in its telemetry database as `remit.link.frame`, `remit.link.repair`
    and `remit.link.delay` events, once per page and minute, with the delays moved onto its own clock by the gap between
    its clock and the core's when the report was sent (`rpc` `link_report.rs`). Repairs by `epoch`, `gap` and `digest`
    come with resume (decision 9). As built (step 5.2), the delay ends when the core hands a tab the event's row (the end of
    the step, on the core's clock), once per event however many tabs show it; an event no tab is handed in the step it
    arrives (no tab shows its stream, or its home is being read) is not counted, nor is the port's crossing to the page.

### Vue

13. **Vue.** `useModel(name, local)` returns the model's rows (a `shallowReactive` map by record id, in the model's
    order) and its `status`; `act(change)` returns the change's answer. `web/src/core/stores.ts` and every
    per-component copy of server records go: the rows `useModel` keeps are the only copy in the page, and there is no
    store library in it. `tsgen` generates `Ask`, `Told` and the models; a record type no page sees gets no TypeScript.
    As built (step 5.1, `web/src/core/client.ts`): `useModel(model)` counts its users by effect scope, asks `show` for
    the first and `hide` once the last scope is disposed; used at a module's top (the catalog, the open chain's and the
    Assistant's view, the drafts) it is shown for the page's life, and a card's stream (`useCardEvents`) is shown while
    its drawer shows that card. A new core shows every model in use again. `inOrder` and `oneOf` read a model as a
    list or as its one record. As built (step 5.2), a tab's local state is what it asks (`openChain`, `openEvent`,
    `readArchived`), kept for that tab in the core, so `useModel` takes a model's name alone; a call's answer is the
    promise of `api`, settled by the ticket the core answers that tab alone. `act(change)` is not built: no page needs
    more than `api` gives it.

## Order of work

1. `link`, `link-quic`: decisions 1–2 and their tests. Done (`feaee83`).
2. `wire` postcard-safe (decision 3) and every caller of the link. Done (`feaee83`).
3. The transcript window and its cursor (decision 6). Done.
4. Interest, conflation, repair per home, telemetry (decisions 7–12), server and core together. Done.
5. The page draws models (decisions 5, 11, 13), then the core moves into a SharedWorker (decision 4):
   1. the core keeps the records and hands the models a tab shows, whole on show and then as row changes, with their
      freshness, as structured clone; `useModel` replaces `stores.ts` and its readers. Done;
   2. the open chain, open event and archived pages are each tab's own in the core; `client-web` exports the
      SharedWorker entry and the page talks to it over its port. Done.
6. The server's half is in (`rpc` `broadcast.rs`, `every_session_converges_on_the_homes_it_shows_whatever_the_order_of_faults`):
   sessions that change what they show, drain at their own pace and fall behind settle on the server's records for
   every home they show. One test drives the core and `Broadcast` (sans-IO, seeded, inside `make test`'s minute): pushes reordered, duplicated,
   dropped by a reconnect, streams reset, epochs changed, interest changed mid-stream, tabs shown and closed. After each
   step a home not `Repairing` equals the server's (same digest, same records); once faults stop, every home converges
   within one reopen; no home is pushed to a session that does not show it. Done: `rpc` `converge.rs`,
   `the_core_shows_the_servers_records_whatever_the_order_of_faults`, the real core over a real `link::Conn` to
   `Broadcast`, its frames read in any order across streams, four seeds of 2,000 steps in about a second. After every
   step that leaves the link quiet each tab's open chain is live and holds the server's records (no digest: decision 9
   defers it, the records are compared directly); once faults stop, every tab converges with at most one read of its
   chain; a session that named its homes is never sent another's stream; a chain never fails to open because the link
   went. It asserts each fault was reached. Epochs are not exercised: none exist (decision 9). It passed 450 more seeds
   (300 at its rates, 150 with the server sending twice as fast). It found six defects, each fixed at its root:
   1. a push the server merged a streamed chunk into, or a merged chunk, straddling text the core read meanwhile was
      dropped, and the event's text stood still until its next update (`events.rs`, decision 8);
   2. the owner opening another chain, or a home's reset, while a reconnect's sync waited for a tab's chain left the sync
      waiting 20 s, every model `Repairing`, then loading the snapshot again (`opened.rs` `open`);
   3. a call asked while a closing session's events were read was never answered: a chain opening then was never read
      (`calls.rs` `ask`);
   4. an opening asked with no session, or cut by one going, was let go; the next session, pushed every home until it
      names its own, named its homes without it and then with it once the snapshot opened it again, and a push sent
      before landed on its read: a record deleted meanwhile put back (`core.rs` `show`, `opened.rs` `answered`,
      `reload`). An opening now waits for the next session, which names it first.
   5. a chain shown again while a push of it sent before the server learned it was let go was on its way, or held by
      the core while its snapshot loaded, read first and then had that push land on the read: a record deleted
      meanwhile put back. The core now reads such a home from its `home_shown` (decision 7); the test reopens chains
      inside that window (`Reached::reshown`);
   6. the core forgot what it had told only once a new session welcomed it, after naming its homes on that session:
      a home named on the session before counted as named, was read at once, and the current pushes after that read
      were then dropped as older than the mark it waited for. What was told is forgotten when a session is dialled
      (`core.rs` `Input::Opened`).7. The report of this change reads the telemetry of decision 12 on the busiest chain, before and after. Done
   2026-11-13: two throwaway instances on one copy of the dev database (`ch_f5bf8d7fcffa`, 1,343 events), each with
   its own key and no agents, `a32c1b0` (step 4 done) and `db10808` (step 5 done); in Chrome, two tabs open the chain
   8 s apart and the minute's reports are read from each instance's telemetry database:

   | | sessions | bytes received | decode | answers and pushes |
   |---|---|---|---|---|
   | before | 2 | 5,585,795 | 79.0 ms | 70 |
   | after | 1 | 4,283,963 | 46.8 ms | 55 |

   The snapshot is read once instead of once per tab (1.25 MB, 21.6 ms saved per tab beyond the first), and each push
   is received and decoded once. What is left is each tab reading the chain it opens: two tabs on one chain read
   `chain_events` (1.0 MB) and `chain_get` (0.2 MB) twice. No event streamed (no agent ran), so the delay was not
   measured here; it is read from real pages' reports.
8. Tabs showing one chain share its view and events in the core, each keeping its own card windows (decision 4). Done
   2026-11-13: in Chrome, two tabs of the verify app on the busy chain (20,000 events, card pages of 50) in one
   SharedWorker; the first tab's opening asked `chain_get` and `chain_events` once each, the second asked neither (its
   only calls were its own card window, `card_events`, and its seen mark) and was handed the 600 events held. A
   streamed chunk reached both tabs' feeds and card windows; paging back the engineer's card in the first tab took its
   window from 51 to 101 events while the second's stayed at 51; with the first tab closed, the second went on taking
   pushes with no read. The seeded test opens a chain another tab shows half the time (`shared`) and converges with at
   most one read of each chain shown, however many tabs show it; it passed 300 seeds.

| Proof | Refuted by | Then | Status |
|---|---|---|---|
| P-SW: a SharedWorker opens a WebTransport session to the dev binary in Chrome and Firefox, and two tabs show one session's pushes | either failing | a dedicated Worker per tab (one session per tab), recorded here | Passed 2026-11-13 in Chrome and in Firefox 150 (playwright's build): a SharedWorker dialled the dev binary's `/link` with its certificate hash, opened a bidirectional stream, and two tabs reported one worker with 2 ports. Safari is not a target (owner, 2026-11-13) |

Already in place, so not built here: pushes instead of polling (no timer in `web/src` polls the server), one push per
changed record and streamed text as deltas (`event_delta`), typed call arguments (`contract!`), one conversion per change
fanned out by audience (`Broadcast`).

Not taken from the desk's design (2026-11-12) it learns from: size and time budgets as tests (the owner's rule of
2026-11-12, no fitness tests: decision 12 reports them instead); one cut across several live sources (a Remit screen
does not total several pushed sources); topics per access scope (`Broadcast`'s audiences already scope what a session is
pushed).

## Consequences

- One decode per answer, off the UI thread; the page holds only what it draws, and the core only the windows near it.
- A long session costs what is on screen: pushes, memory and repairs are bounded per home, not by the session's age.
- The server keeps, per session and shown home, a version, a digest and its unsent records: memory in proportion to
  what is shown.
- Frames are not readable by eye: a debug view decodes them with the contract (`Revisit when`).
- Every contract change needs a new bundle and rebuilt plugins (already true: the bundle and contract checks refuse a
  mismatch).
- A page left open across a release reloads (it already does, `NewBundle`).

## Revisit when

- Debugging the link needs frames in the clear: a dev-only decoder in `link` that prints a frame through its contract.
- A peer of another build must talk to Remit (a plugin built elsewhere, kept across releases): a version field per
  contract and a per-version decode, or a self-describing format on that contract only.
- Step 5 shows structured clone of a large model slower than the JSON batch it replaced: hand that model as a transfer of
  its rows in pages.
- A brief drop reloads too much (telemetry shows reopens after reconnects dominating the bytes): resume a home from its
  version, with `epoch`, `version` and the per-home digest of decision 9 on each push.
