# 0024. Everything that comes in is a reading of an event signal

Status: accepted, 2026-11-01; partly superseded by ADR-0047: the signal's pull is a sensor that writes it, a poll belonging to the sensor and not to the signal; its preconditions, prompt, Dispatchers and placements are replaced by ADR-0036; by ADR-0064, `Emit` is the event signal's adapter over `ingest`, the one write path of every reading; its waits on the owner, but a secret act, are an agent's (ADR-0087). Supersedes the webhook and delivery model of ADR-0008, ADR-0011 and ADR-0020 (their burst, builtin
and forge rules stand; their owner changes) and the `delivery` source and Deliveries page of ADR-0014 and ADR-0019. Source: the
owner's requests of 2026-11-01 (a GitHub issue or a Datadog error as a signal; "not everything has a webhook"; whatever the way
in, a webhook, a CLI or a poll, it is a signal and there is no webhook concept coming in), `docs/design.md`, ADR-0014.

## Context

ADR-0014 made a delivery a reading, but only a pointer: the payload, the handling (status, Dispatcher card, burst, outcome) and
the rules (a webhook's preconditions and prompt) stayed in two records of their own, `Delivery` and `Webhook`, and the only way
to write one was an HTTP post to the door with a key. A system with no webhook to point at Remit (a GitHub issue opened, a
Datadog monitor that alerts) cannot be heard, and a CLI cannot hand Remit a fact without posting HTTP. The same fact then has
two owners: the reading that says it arrived and the delivery that says what it was.

## Options

1. Keep Delivery and Webhook; add a poller that posts into `Receive`. Smallest change; leaves the webhook as the concept every
   other way in must imitate.
2. A second source of readings beside deliveries (`feed:`), with its own placement. Two pipelines for one job.
3. **One event signal. Every way in is a writer of readings of `event:<name>` through one application call; what arrived is
   the reading; what became of it is a placement; how it is placed is the signal's own settings.** Chosen.

## Decision

- **A signal `event:<name>` is what the outside says.** Its readings are what arrived, whichever way it came. The source `event`
  replaces `delivery`; the key shape is `event:<name>`, `<name>` lowercase letters, digits and dashes, at most 40.
- **One write path, `Orchestrator.Emit`.** Every writer calls it: the HTTP door (`POST /hooks/<name>`, kept as an address so
  senders already pointed at it keep working), the CLI (`remit emit`, a client of that door), the poller (a signal that
  pulls), and an agent's tool. `Emit` refuses a name no one made and a signal that is paused, bounds the body, makes a retry the
  same event (the sender's own id, or a hash of name and body), stores the reading and its placement in one step, refuses
  past `maxPendingEvents` waiting, and records who wrote it (`Cause.Actor`, the channel in `InboundEvent.Via`).
- **A reading's value is `{event, found}`.** `event` (`model.InboundEvent`; `model.Event` is a card's event, what its session
  streamed) is what arrived, as the sender wrote it and bounded: `via`
  (`door`, `cli`, `poll`, `agent`), `header`, `query`, `body` (text, at most 64 KiB, `bodyCut` when longer). `found` is Remit's
  own reading of it at that instant, never the sender's words: the chains it names (`names`) and what the first rule that held
  does (`does`). Conditions read `value.found`; routing never reads `value.event`.
- **A placement is what became of a reading** (`model.Placement`, id `pl_…`, one for each reading of an event signal, stored
  with it; one made of a stored delivery keeps the delivery's id, so the home of its Dispatcher stays where it was): status (pending, working, handled, failed, stopped), the Dispatcher card, burst, chains told or started, why it waits,
  its outcome, the rule that settled it, what the door recorded of the merge requests it is about. It carries none of what
  arrived. The Dispatcher's card lives in the home of the first placement of its burst.
- **The settings of an event signal** (`model.EventSignal`, the record that was a webhook, table `event_signals`) are its note,
  its prompt, its preconditions (the rules that turn an event into `found`), whether writers are let in (`paused`, the rail, held
  in `Emit` for every writer), whether it ships with Remit (`builtin`, `edited`), and `pull`, how Remit itself
  fills it. The door's key stays sealed beside it and opens the door only; the owner shows it from the signal's page.
- **A signal that pulls** (`EventSignal.Pull`: source, connection, query, every, cursor). The poller runs on the scheduler's tick
  in the background like the rollup poller, reads the connection through the gate with Remit as the caller (read effect only,
  grants as for any call), turns each new or changed item into an event and calls `Emit` with the item's `id@version` as its
  sender id, so a replay acts once. The first read sets the cursor and emits nothing older than the signal; one read emits at
  most `pullItemsMax`. A source that fails three reads in a row is silent, and the signal's page says so. A provider that can be
  pulled implements `source.Pollable`; GitHub issues and Datadog monitors do. The gate lets the poller (caller `event-pull`, in
  no chain) read exactly the source and connection a signal's pull names. A pull that reads a source, a connection or a query
  the signal did not read before is a standing use of a credential: set by anyone but the owner (the Assistant included), it
  waits for his Apply in his inbox, as a write that moves a rail does.
- **The owner tells the Assistant.** Event signals are made, changed, paused and pulled by the Assistant's tools
  (`create_event_signal`, `update_event_signal`, `pause_event_signal`, `resume_event_signal`, `rotate_event_signal_key`,
  `delete_event_signal`, `reset_event_signal`, `list_event_signals`, `list_events`, `read_event_fields`); no page has a form. The
  tools of the webhook are gone. A chain whose playbook lists the `events` capability gives its Chair `emit_event`.
- **The CLI.** `remit emit -name <signal> [-url <server>] [-key-file <path>] [-sender-id <id>] [file | -]` posts to the
  signal's door, saying it is the CLI (`X-Remit-Via: cli`); the key comes from a file only its owner can read, or from
  `REMIT_EVENT_KEY`, never from an argument.
- **The pages.** The Signals tab shows the `event` source as one column of event signals; a signal's page shows its settings
  (rules, prompt, pull, address, key, pause) and its readings with their placements (`ReadingLine.placementId`). The Events tab
  shows what came in and what became of it, with the flow of a placement: the contract's `Placement` (pushed by
  `placementUpdated`), `EventBoard` and, for one open, `EventView` (`eventGet`), which holds the `InboundEvent`. The Subscriptions
  tab lists an event signal's skip, tell and dispatch subscriptions as before (`event_signal:<name>`).
- **Chains** started by a Dispatcher have origin `event` and name the placement that started them.
- **Rails do not move.** Authentication of the door, pause, the quota of chains a day, the Dispatcher limit, merging and sign-in
  stay deterministic code; an event is evidence, never an instruction.

## Threats

| Threat | Control |
|---|---|
| A forged or flooding write at the door | Unchanged: sender key or HMAC, one refusal for every cause, size and pending limits, the quota. `Emit` repeats the paused, size and pending checks for every writer |
| Text in an event steers a Chair or a Dispatcher (OWASP LLM01); a public GitHub issue is written by strangers | An event reaches an agent only fenced, as evidence; rules read `found`, which Remit computes; a poll reads with a read-only effect and a credential no agent sees |
| A poll floods (an old issue list, a monitor that flaps) | The first read emits nothing older than the signal; at most `pullItemsMax` items a read; the sender id `id@version` makes a replay one event; the Dispatcher quota of chains a day holds |
| A poll or an agent's tool writes an event that wakes its own chain without end | A reading caused by the subscriber's own chain is skipped (`signal.Evaluate`); an agent's `emit_event` carries its chain in `Cause` |
| An injected Assistant or agent sets a pull that reads a connection the owner never meant Remit to read, and so feeds its data to the Dispatchers | A pull that reads anything new waits for the owner's Apply (`holdEventSignal`); the gate lets the poller read only a pulled (source, connection), read only, by a typed call (`pullVerdict`) |
| A CLI user's key leaks through the process list or a shared file | `remit emit` takes no key as an argument and refuses a key file others can read |
| A paused signal still hears a writer other than the door | `Emit` checks `paused` for every writer |
| A pulled credential reaches the signal's page or an agent | The poll goes through the gate; the page and the readings show no credential; `redact` applies to every item |
| A restart loses or repeats an event | The reading and its placement are one transaction; the migration is numbered and idempotent; a replayed `Emit` returns the stored event |

## Consequences

- A GitHub issue, a Datadog alert, a file a CLI posts and a GitLab post are the same kind of thing and take the same path; a new
  way in is a writer, not a concept.
- `Delivery`, `Webhook`, the `delivery` source, the `deliveries` and `webhooks` tables, `Receive`, the Deliveries/Webhook RPCs
  and the webhook tools go in the same change as their replacements, with a numbered migration that turns stored deliveries into
  readings and placements and a webhook into an event signal. Chain origin `webhook` becomes `event`.
- The rules (preconditions) stay with the signal, not with subscriptions: they turn an event into facts and cannot be one
  subscription's condition without moving the chair-finding code into every one.

## Revisit when

A second kind of rule needs a field that only one source reads (a flag in disguise); preconditions are wanted on a signal that is
not an event signal; or readings of events outgrow the owner's database.

Enforced by: TestAnEventSignalMustExistBeforeAnyoneWritesIt, TestAStandingSensorWritesItsItemsAsEventsAndStoresItsCursor, TestAReplayedPollIsOneEvent, TestASilentSensorIsSaidOnItsSignalsLine, TestMigrationTurnsDeliveriesIntoEventsAndPlacements, TestEmitStoresOneReadingAndARetryIsTheSameEvent, TestAPausedSignalTakesNoEvent