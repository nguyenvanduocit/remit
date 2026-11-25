# 0128. The desktop app is a native shell of the client core

Status: accepted, 2026-11-20. Source: the owner, 2026-11-20 (translated): "is it time to build the complete desktop app
for Remit?", then "do it".

## Context

The owner reaches Remit through one channel, the web app: a Vue page over the client core (`crates/replica`, ADR-0072
decision 10, ADR-0095), which keeps the replica of what the server pushes, its freshness, the outbox of his sends and the
reconnects, with no I/O. The browser's shell (`crates/client-web`) carries the core's link on WebTransport and hands
each tab what the core tells it.

`remit-desktop` (`crates/desktop`) draws with shadcn-rs (`gui` and `shadcn`, a native GUI and shadcn/ui's components in
Rust, with input methods and screen readers, its ADR-0009), and shows a fixed sample. The owner wants the whole app on
the desktop.

The core speaks raw streams (`Output::Open`, `Send`, `Finish`, `Reset`; `Input::Received`, `Written`, `Closed`): it owns
its `link::Conn`. `link-quic`, Remit's native transport, offers sessions only through a driver that owns a `Conn` of its
own (`Peer`, `Link::call`), so nothing native can carry the core today.

## Options

1. **The web app in a webview** (wry/Tauri). The fastest way to a window, and the same screens. But it puts a second
   browser engine under the app, leaves every screen in Vue and gains nothing a browser tab lacks; shadcn-rs would draw
   nothing of Remit.
2. **A native shell of the client core**, its screens in `gui` and `shadcn`. The same core as the web, so freshness,
   outbox, reconnect and what waits for the owner are not written again; the screens are Rust drawing the records the
   server sends.
3. **A native client of high-level calls** (as `remit-client` calls and watches). No core: the desktop would keep its own
   replica, freshness and outbox, a second owner of what `replica` owns.

## Decision

Option 2.

1. **One stream pump for QUIC.** `link-quic` gains `Streams`: a QUIC session's streams as a `link::Transport` (open,
   send, finish, reset, close) and the inputs they give (read, written, a peer's stream numbered per kind, closed).
   `link-quic`'s own driver is rebuilt on it, and the desktop shell uses it; `client-web/src/browser.rs` is its
   WebTransport twin.
2. **The shell runs the core off the UI thread.** A thread with a current-thread tokio runtime owns the `Replica`, the
   `Streams` and the core's one timer, and steps the core one input at a time, as `client-web/src/shell.rs` does. The UI
   thread hands it `Ask`s; what the core tells the window (`Output::Tell`) crosses to the UI thread through a `gui`
   mailbox that wakes the window. The window is one tab of the core.
3. **The way in is the web's.** `remit-desktop <link>` takes the link the server prints (`http://host:port/#key=…`,
   ADR-0026) or a mentor's code. The shell finds the link through `/link.json` and the trust it names, with
   `remit-client`'s `dial` (one owner of that), and keeps the credential in Remit's directory under the user's home
   (`~/.remit/desktop.json`), readable by its user alone, as it keeps the owner's sends the server has not taken
   (`Told::Keep`, `~/.remit/desktop-outbox.json`). With none kept, the window opens on the way in, where the owner pastes
   the link: typing a credential is his.
4. **The screens draw records, not rules.** A `Store` on the UI thread applies each `Told` (a model's rows, its
   freshness, the core's state) to signals; a screen decodes the rows it shows into the `model` and `wire` types the
   server sent and draws them. What waits, groups, labels and status come laid out by the server, as for the web.
5. **Built in the order the owner works.** The frame (sidebar of the web's areas, the link banner), the Board, an agent's
   screen (its live events; answer, pause, stop), the Assistant's panel, then the other tabs of the top bar.
6. **Looked at headless.** `remit-desktop --trace` drives the screens against a fake server, as the web's verify pages do,
   and a frame's PNG and report are what an agent reads.

## Consequences

- The desktop and the web run the same core: a fix to freshness, the outbox or reconnects reaches both.
- Every screen is written twice, once in Vue and once in Rust, until one channel is retired; the records they draw are
  one definition (`crates/model`, `crates/wire`).
- `link-quic`'s driver changes shape inside; its public API (`dial`, `Peer`, `Link`) does not.

## Revisit when

- The credential should leave the disk: the macOS Keychain (and the platform stores elsewhere) behind the same `Kept`
  seam the shell reads it through.
- The owner should be told outside the window what waits for him: platform notifications from the core's `Notice`.
- The app ships to someone else's machine: an `.app` bundle, signing and notarisation, and updates.
- One channel can carry every screen well: retire the other.
