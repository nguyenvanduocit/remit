# 0056. A person works from Paseo, through the Work plugin

Status: superseded by ADR-0089 (a mentor works from the web app), 2026-11-12. Source: `docs/superpowers/specs/2026-11-04-employee-runtime-design.md` (*`/worker`*, *A person's card*,
*What this replaces*), ADR-0053, ADR-0051, ADR-0026.

## Context

A person the owner hires works from Paseo, on a phone, a laptop or a pod, and Paseo is the app they already keep open. Remit does
not depend on Paseo (AGENTS.md): the server speaks one channel to a person's device, `/worker` (ADR-0053 signs the device in, ADR-0051's
*The protocol* says what crosses it), and names no app. Something on the device has to hold the key, keep the socket, show each task
where the person looks, raise the phone's notification while the app is closed, and carry the person's acts back: accept with a due
time, decline, answer a question, ask the lead. Paseo runs a plugin's server entry in a subprocess of its daemon and its client entry
in every connected app; a plugin's provider turns a conversation into a Paseo tab, and a tab that finishes a turn raises Paseo's
attention and notification.

## Options

1. **A plugin with a daemon shim that tags callers.** The shim told the plugin which calls came from the person's own app and which
   from a local agent, and recorded presence from the daemon's client heartbeat. It patches the daemon, and the spec drops the rule
   it served (*What this replaces*: no caller guard).
2. **A plugin alone.** Everything lives in `paseo/`: the server entry holds the key and the socket, the client entry reports each app's
   presence itself, and a provider makes each task a tab. **Chosen.**
3. **A separate app.** A second app on the person's phone, with its own notifications and sign-in: a second place to look, and
   everything Paseo already gives (tabs, attention, push) built again.

## Decision

Option 2, the Work plugin (`paseo/`, plugin id `work`).

- **One host serves one person, and the key is the host's.** The server entry signs the host in (`signInStart` with the host's id,
  made once in `$PASEO_HOME/work/host.json`, then `signInWait`), keeps the key in `key.json` (mode 0600, written whole) and holds one
  socket while signed in. There is no caller guard: every app connected to the host is the person's. `bad key` from the server signs
  the host out.
- **The feed.** Events are handled once, in id order: an id at or below the cursor (`cursor.json`) is answered and skipped, and the
  cursor moves past an event only after its tab has it. A fresh key starts at the feed's head its sign-in answered (`head`), so old
  events never become new popups; every (re)connect reads the person's tasks again.
- **Where the person sees it.** The host owns every rule and every word (`shared/copy.ts`, which names no product): the app's sidebar
  row counts what waits, the first popup shows over whatever the person looks at (an offer, a question, news on a question), and the
  Work screen lists the tasks. A popup's news is marked seen when it shows.
- **A task is a tab.** The provider `work` gives each task a Paseo tab (`work/<card id>`, labelled `work-card`). Each tab sits in
  a workspace of its own on `~/Work`, titled with the task (made once per card, its title kept to the task's), so the sidebar
  names each task; a tab found elsewhere is made again there and the workspace it leaves is archived once empty. A question's
  timeline is the task's thread and its composer asks the lead; `/answer`, `/accept` and `/decline` act on the task. A live event
  for a question's tab runs as a turn (started, the item, completed), reloading a closed tab first, so Paseo raises its attention
  and notifies the person's apps, the closed ones included.
- **A work task's tab is the person's hosted session** (ADR-0059). The session runs in the AI runtime; the tab only relays, over
  `/worker`: `attach` reads the session a page at a time and follows it from its first call, so every entry stored or changed is
  pushed (`timeline`) and an id reused updates its item in place; the composer prompts the session (`sessionPrompt`, its request
  id Paseo's `clientMessageId`), steers a running turn (`sessionSteer`) and stops it (`sessionCancel`); a turn runs from a prompt
  taken to the next turn end; a lead's message is drawn as the lead's and marked seen when an open tab shows it. A permission prompt
  shows as *Waiting for an approval* until the decision is the person's, then as Paseo's prompt (`permissionAnswer`). Each
  reconnect attaches every open tab again, from before its oldest entry still in flight.
- **Acts act once.** Each press makes its request id once and sends it again with every retry; the server acts once per id.
- **Presence** comes from each app's client entry: its own id, its device type, whether it is in front of the person and when they
  last used it, on every change and each minute while visible; the host forwards the person's apps to the server.
- **Shipping.** The release carries `work-paseo.tar.gz` (`make work-paseo`); a host image unpacks it, runs `npm ci` and the
  typecheck, and registers it. The server's address is the plugin's host setting, which the person sets on its settings screen
  (Settings → Plugins → Work); until it is set every call answers that Work is not set up and no socket opens, and a new address
  closes the old server's socket. `key.json` records the server that issued the key, so a key is never sent to another address.

## Consequences

- The person needs Paseo and nothing else; their phone is notified of a task's news through Paseo's own attention.
- Remit stays free of Paseo: the plugin is a client of `/worker` like any other, generated from the same contract.
- Paseo hands a plugin its API only inside a call, so the tabs work from the first call of an app on this host: a task offered
  after a daemon restart, before any app opened, gets its tab when one opens.
- A host shared by two people would show one person's work to the other: one host is one person's.

## Revisit when

- **A host is shared by several people:** the key moves from the host to the app (an app signs in), and the plugin tells callers
  apart again.
- **A key should sign the person's acts** (a key pair per host), so a copied `key.json` alone acts as no one.
- **The person wants the task's files or a terminal beside the session:** the provider gains Paseo's file and terminal
  surfaces, read through `/worker` as the session is.
