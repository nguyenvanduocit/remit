# 0094. The page stays readable while the link is down, and the owner's sends wait in the client core

Status: accepted, 2026-11-13. Amends ADR-0072 decision 10 (the client core) and the offline view of `Shell.vue`.
Source: the study of agentes.chat (`docs/research-agentes-chat/report.md`, idea 4 and finding D6), which the owner asked to
build ("implement every idea you just suggested"), and his words of 2026-11-13 (translated): "when offline, or in any
other connection state, there must be good state, notice and feedback."

## Context

While the link was down the web app drew no page, only a centered line saying what the connection was doing
(`Shell.vue`), and every send failed at once with "Not connected" (`replica::core`, `Input::Send`). The owner had chosen
the blank page earlier: nothing shown could be current and nothing he did would be sent. While a message went, its
composer was disabled (`CardComposer.vue`), so a slow link held the box. agentes.chat keeps the last view readable with a
banner, queues what is sent offline, and shows a sent message at once. The page already holds a replica it may drop at any
moment; a lost link is one more reason it is not current, not a reason to hide it.

## Options

1. **Keep the blank page, fail sends offline** (as it was). Nothing stale is ever shown, but the owner loses his place on
   every blip and must type again what did not go.
2. **Queue sends in the page (Vue) and keep it readable.** Rejected: the request id and the replay rule are the core's
   (ADR-0072 decision 10); a queue in Vue would be a second owner of "a call replayed after a reconnect does not act twice"
   and would not exist for the next channel.
3. **Keep the page readable with a banner; sends wait in the client core, in an outbox, with their request id.**
   **Chosen.**

## Decision

- **The page stays.** Once a snapshot has been had, the app draws whatever the link does. While it is down the connection
  banner says since when what shows dates, how many tries have failed and when the next is, with **Try now**
  (`Ask::Connect`); a page of another build says to reload (Reload); a link back says so for three seconds. Each change of
  state is announced once in a polite live region, the countdown not. Before the first snapshot only the centered line
  shows, with Try now. A refused key keeps showing the sign-in.
- **The core words it.** What the banner says, its tone, its one action, the count of messages that wait and what a screen
  reader is told are the core's `State.banner` (`replica::banner`), and why an action cannot be taken now is
  `State.blocked`; the core wakes each second of the countdown and when "connected again" ends. A shell renders them and
  writes only the time the link went (`{since}`) in the owner's clock, so every channel says the same.
- **Sends wait in the core** (`crates/replica/src/outbox.rs`). `Ask::Send` adds a row to the `Outbox` store at once
  (`Pending`: where it shows, its words, its drafts, its status) and gives it a request id then. With no session it is
  `waiting` ("goes out when the connection is back", and the banner counts them); with one it saves its drafts and is
  asked (`going`), `slow` after 500 ms (the only time "Sending…" shows), `sent` for 2.5 s once taken, then gone.
- **Replayed once.** A try whose answer never came (the session went) waits for the next session and goes with the same
  request id, so the server, which answers a request id it has seen as it did the first time (`orch::request`), acts once.
  A timeout fails the row but keeps its id for Try again; only a failure the server answered gets a new id.
- **Kept across a reload.** The core tells the shell what to keep (`Told::Keep`: each waiting or failed send, its call,
  its request id and the words typed in its drafts) whenever a row changes, and a page loaded again gives it back
  (`Ask::Restore`) before it dials. The web shell keeps it in the tab's `sessionStorage` (`keepIn`), so a page that reloads
  for a new build sends what waited, with the same request id; each tab keeps its own, so two tabs never write over each
  other's. A page turned away as another build
  reloads at once (`main.ts`); while it cannot, the banner says how many messages go once it does.
- **One way out.** A failed row says why, with **Try again** (`Ask::Resend`) and **Edit** (`Ask::Unsend`: the row goes, the
  words show in their box again). Sending the same drafts again takes the failed row's place.
- **The box never holds.** The words leave the box for a pending row above it; the owner types the next message at once.
  A box's new draft is not saved while a send is held, since the draft being sent may still be in that box, which takes one
  draft (`Drafts::tick`).
- **Actions say why.** With no session, an action that changes something (in the composer: Stop, Interrupt, Restart) is
  `aria-disabled` and says why on hover and focus; a message still goes. Every other action is refused by the core with
  words that say so (`OFFLINE`).

## Consequences

- The owner keeps reading and writing through a dropped link; what he sent goes once, by itself, when the link is back.
- The core owns one more store and three asks; every channel built on `replica` gets the outbox.
- `Ask::Send` lost `place`: the outbox gives the request id, so the page no longer names a place to retry a send.
- What shows offline may be old; the banner says from when.
- A tab closed with sends waiting loses them (their drafts stay on the server, in their box).

## Revisit when

- Sends should outlive a closed tab: keep them in `localStorage`, one entry per tab that a page loaded later adopts.
- Every action that changes something (beyond the composer's) should be disabled with its reason while offline: a shared
  `offlineWhy` on the Button the actions use, done by the web-ui wave that restyles these screens.
