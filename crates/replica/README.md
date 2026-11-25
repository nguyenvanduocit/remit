# replica

The client core (ADR-0072 decision 10): one link session to the server and the replica of what it pushes, sans-IO.
`Replica::handle(now, Input) -> Vec<Output>`. A shell feeds it what its transport saw, the time and each tab's asks
(`Input::Ask { tab, ask }`, `Input::Left { tab }`). It gets back transport ops, one timer, and what each tab is told
(`Output::Tell { tab, told }`): the models it shows (whole, then their row changes and freshness), the answers to its
calls, `State` and `Notice`s. It depends only on `link`, `wire`, the records `wire` carries (`model`, ADR-0107) and
`indexmap`, never on a rule crate, and builds for `wasm32-unknown-unknown`.

## Shape

- Tabs (ADR-0095 decision 4): one core serves every tab of the browser. The session, the catalog, the Assistant, the
  drafts and the outbox are shared; the chain a tab opens and its window on each card's events (`opened.rs`), its open
  event and archived pages (`Page` in `core.rs`), the models it shows, its places' retries and whether the owner sees it
  (`Ask::Hidden`) are its own. A chain's view and events are held once however many tabs show it (`held.rs`): a tab
  opening a chain another shows is handed it with no read, and one read serves every tab opening a chain meanwhile. A
  model is kept in one slot (`models.rs` `Owner`): once for every tab, once per chain for the tabs that show it, or a
  tab's own, told to it alone. The homes named to the server (`homes_shown`) are the union over tabs. An answer goes to the tab that
  asked; a notice about what a tab asked goes to it, where duels stand to every tab, the rest to the tabs the owner
  sees (every tab when he sees none). A tab gone (`Input::Left`) takes what it opened with it, and the last closes the
  session.
- Models (`Model`, ADR-0095 decision 5): the catalog's, the open chain's (`CardEvents(card)` per card), the Assistant's,
  the open event, the archived pages read, `Typed` (what the owner typed that the server does not have yet) and `Outbox`
  (the owner's sends the server has not taken yet, as `Pending` rows). A model of one record keeps it under the empty key.
- The rules keep each record's key, order and the few facts they read (`Index`) and say every change as a `Patch`
  (`reset`, `put` in place or before a key keeping named fields, `merge`, `append` a streamed chunk with offsets in UTF-16
  units as the server counts them, `drop`, `forget`). `models.rs` applies it to the records it keeps, as the server's
  JSON, and tells a shell that shows the model (`Ask::Show`, until `Ask::Hide`): `Told::Model` whole on show and when
  it is read again, `Told::Rows` (`put`, `append`, `drop`) after, nothing for a model not shown.
- Freshness (`fresh.rs`, decision 11): each shown model's `Live`, `Stale` (offline, behind) or `Repairing`, by the home it
  is read with, told when it moves.
- Computed reads stay in the shell for now (`archivedChains`, `liveEvents`, `waitingOn`, `textOf`, `comments`, the box's
  draft, `prefill`).
- Request ids are the shell's random `salt` and a count. Draft ids come from the same counter.
- Drafts: typed words are saved on the revision of the server's copy they were typed over (`draft_save`'s `base_rev`). A save
  refused `CONFLICT` (another page saved since) carries the draft as it is; the words go on from it, keeping both pages'.
- Outbox (`outbox.rs`, ADR-0094): a send is a row from the moment it is asked, with its request id; with no session it
  waits and goes when one is up. A try whose answer never came goes again with the same id, so the server acts once; only
  a failure the server answered takes a new id on Try again (`Ask::Resend`). `Ask::Unsend` takes a row back to its box.
- State says when the link went (`lost_at`), the failed dials in a row (`tries`) and the next (`next_try`).
