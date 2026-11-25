# 0036. An event signal holds readings, and every rule about them is a subscription

Status: accepted, 2026-11-02; partly superseded by ADR-0064: a subscription's `Cooldown` is a named `Flow`, and only the Tick judges a reading (no inline `o.emit`). Source: the owner's requests of 2026-11-02 (a precondition belongs to the subscription that
wants the event, as its filter or its own handling; each GitHub rule becomes a subscription: update the chain's status, tell the
Chair, trigger a chain; the Dispatcher goes; a subscription is sent what it subscribed to and manages its own queue; burst
batching goes), `docs/design.md`, ADR-0014, ADR-0024, ADR-0035. Replaces ADR-0008 (burst) and, of ADR-0024, the preconditions, the
Dispatcher and the placement; of ADR-0014, the actions `dispatch` and `skip` and the claim order of an event key's subscriptions.

## Context

An event signal (`event:github`) carries eight preconditions: each a condition (`when`), what to do (`does`: `chair`, `live`,
`skip`, `dispatcher`) and what to read of the event (`marks`, `summary`, `quote`, `author`). The door runs them once, when the
event arrives, and writes `found.names` and `found.does` into the reading. Three fixed subscriptions then read `found`: a skip, a
tell for each named chain and a dispatch (`event_signal.go:75-88`). One decision is written twice, as a rule and as a route, and
the rule sits on the signal although only a subscriber wants it.

Around that sit a Dispatcher (an agent outside any chain with its own pool, `MaxDispatchers`, and `OriginDispatcher` special-cased
in about ten places), a burst rule that holds events for 20 seconds to 2 minutes (`tellQuiet`, `tellLongest`) and a `Placement`
per reading that tracks its Dispatcher, its burst and its wait. All of it exists to turn an event into an act, which is what a
subscription is.

## Options

1. Move the preconditions onto subscriptions and keep a catch-all that starts a Dispatcher. The Dispatcher stays a second kind of
   agent, with its pool, its card, its placement and its burst.
2. **The signal holds readings; a subscription is a `when` and an action; nothing in between.** The Dispatcher, the burst and the
   placement are gone. An owner who wants a triage agent subscribes a chain to the events he means. Chosen.

## Decision

- **An event signal is the door, the key, the pull, the pause, a note and its readings.** A reading's value is `{event}`, what
  arrived (`InboundEvent`). It has no preconditions, no prompt, no `found`.
- **A subscription to `event:<name>` is the subscription of ADR-0014, with nothing of its own for events.** Its `Expr` (the `when`)
  runs in the one environment every key's subscriptions use: `value` (for an event, `value.event`: its `body`, `header`, `query`),
  `now`, `chain` and `cause`; `body` and `header` are in that environment as short names for `value.event.body` and
  `value.event.header` when the reading is an event. A reading goes to every subscription whose `Expr` holds. The field
  `Position` and the claim order are deleted: no key has an order across its subscriptions. A subscription is sent what it
  subscribed to, once per reading, and the subscriber manages its own queue: a Chair's messages queue in its outbox
  (`outbox.go`), a chain waits in the queue of ADR-0001. The signal holds nothing back, batches nothing and orders nothing.
- **An action is a kind and its parameters (`With`), and a parameter is a CEL expression over the same reading, except where the table says literal.** The action kinds are
  the ones ADR-0014 and ADR-0035 name, whichever key fires them; this record adds none:

  | Action | Does | Parameters |
  |---|---|---|
  | `tell` | Queues the reading in the Chair of one chain, fenced as evidence | CEL: `chain`, `summary`, `quote`, `author` |
  | `start` | Queues a chain from a playbook, one at a time for a subscription (`queueStart`), what it hears meanwhile waits | literal: `title`, `goal`, `playbook` (the owner's words, as a routine's); CEL: `summary`, `quote`, `author`, appended to the goal as fenced evidence |
  | `run` | Runs one job of the closed registry Remit ships (ADR-0035) | literal: `job` (named by the subscription, never computed); CEL: the job's own, such as `chain` and `summary` |

  `chain` gives the chain by marks (a list of text: its merge request address or its branch, matched by equality, `chainsAt`,
  never by a mention in words) or by id (`value.chain`, for a fact). No chain, two chains or a finished chain is the firing's
  outcome, in words. `start` and `run` exist already; `start` gains the parameters so a chain can begin from what a reading says.
  What was `live` and `sync` are two jobs of the registry, `mark-live` (`MarkLive`) and `sync-merge-request` (`followMRs`, the
  forge's answer recorded, never what the event says), each taking `chain`. A new provider that needs a new effect adds a
  job, not an action, and a job is where an extension (ADR-0034) plugs in. A shipped rule that told the Chair of a merge and
  recorded its state is two subscriptions. A `tell` whose `author` is Remit's own account on the chain's forge is not told.
- **A firing's outcome says what became of the reading for that subscription**, in words (`Firing.Outcome`), as the placement's
  did. What became of an event is its firings: a reading and the firings on its revision are the Events tab's pair, so no second
  record says it.
- **GitHub and GitLab ship their rules as built-in subscriptions** (`Builtin`, `Edited`, ADR-0014's rule for what ships): the
  code gives their settings as data, one list for each provider, each subscription a sentence the owner can read on the
  Subscriptions tab and change by telling the Assistant. The list is the seam where a provider plugin (ADR-0034) supplies its
  own. The ping that was a `skip`
  has no subscription: an event nobody subscribed to is stored and does nothing.
- **The owner subscribes by telling the Assistant.** Its event-signal tools lose `preconditions` and `prompt`; subscription tools
  (`subscription_tools.go`) take an event key, a `when` and an action with its parameters; a routine keeps its own tools and a Chair's
  watches stay the Chair's. `read_event_fields` and `list_events` stay: the Assistant
  looks at a real event before it writes a `when`.
- **A retry of an event is the same event** (a writer's own id, or a hash of the body within the ten minutes a writer retries in: the same body later is a new event, as an alert that fires again is) by a `Dedupe` on the reading, unique within
  its key, where the placement kept it. The bound on events waiting for a Dispatcher (`maxPendingEvents`) has nothing to bound and
  goes; the door's size limit and key stand.
- **The quota of chains a day** (`eventQuota`, `EventChainsPerDay`) stays as the cap on `start` for one event signal, and a
  subscription's `Cooldown` stays.

## What goes

`model.Precondition`, `Settled`, `Found`, `PreconditionDoes`; `precondition.go`, `precondition_read.go`, `doorReads`,
`doorVerdictOf`, `placedBy`, `place`, `behindDispatcher`, `burstOf`; `signalSubscriptions`, `tellSubscription`, `waitOwner`'s tell
use, `exprSkip`, `exprTell`; `ActionDispatch`, `ActionSkip`; `Subscription.Position` and `SubscriptionWindow`'s
`Quiet` and `Longest` with `internal/signal/coalesce.go` (their only user is the tell); `dispatcher.go`, `dispatcher_tools.go`,
`event.go`'s Dispatcher prompt, `OriginDispatcher`, `dispatcherRole`, `MaxDispatchers`, the Dispatchers' pool in `admit.go`;
`model.Placement`, the `placements` table (`internal/store/placement.go`) and `PlacementUpdated`; on the web `PreconditionFlow`,
`PreconditionAction`, `PreconditionDetail`, `preconditionReading.ts`, and the placement parts of `EventFlow`, `EventDrawer` and
`EventBoard`; ADR-0008 and the matching parts of `docs/design.md`, `README.md`, the `app` skill and `event-signals.md`.

## As built

- Subscriptions the owner makes and the ones that ship are owned by Remit (`OwnerRemit`); `By` says who made one (`owner`, `assistant`, `agent`, `remit`). The migration's subscriptions are owned the same way.
- `Emit` stores the reading and kicks the scheduler; the scheduler judges it (`o.emit` judges inline for callers that need the firings at once). A subscription made or resumed after a reading starts at the newest reading (`SeenRev`).
- A `tell` never tells the chain whose agent wrote the event (`reading.Cause.ChainID`), so an agent's `emit_event` does not wake its own Chair.
- The shipped tell subscriptions require the forge's event header, so they never hear an agent's event.
- A firing runs with the scheduler's lock held, so it asks no forge itself. Who Remit works as on a host is asked in the background, kept in memory (served stale while it is asked again) and the tell that needs it waits, shown as held with the reason, for up to 30 minutes before it is given up with the reason. `sync-merge-request` is stored as a firing and then run in the background; the next look at the open merge requests records the answer if a restart came between.
- An event's address is `event:<signal>#<rev>`; events settle after 7 days (`pack.go`). A stored memory source that names a placement id is not migrated.
- The Subscriptions tab shows one line for each subscription to an event key; the signal page lists the same lines from the board.

## Pages

- **Signal page** (`/signals/signal:event:github`): *The event signal* (who set it, writers, door address), *Pull*, *Subscriptions*
  (one line for each subscription on the key: its `when` in words, its action, when it last fired, a link to it on the
  Subscriptions tab; an empty list says that nothing happens to its events yet), the key and the guide. Preconditions and Prompt
  are gone.
- **Events tab**: a reading with its firings: for each, the subscription, the action and its outcome. An event with no firing
  says "no subscription held for it".

## One change, one migration

One numbered migration, run once, never edited after: turns the rules of each stored event signal that is not an unedited
built-in into subscriptions of the owner (`Edited`; the shipped ones are made by `SeedSubscriptions`); rewrites each reading's
value to `{event}` and gives it the `Dedupe` its placement held; deletes the placements, the Dispatcher cards with their
drafts and inbox items, and the per-chain tell subscriptions; drops `preconditions` and `prompt` from the stored signals and the
`placements` table. It never touches the `events` table (`migrations.go` forbids it, `TestAMigrationNeverRewritesEvents` holds
it), so the Dispatchers' stored events stay as rows that no card or home reaches. The code, tests and docs
listed under *What goes* are deleted in the same change. The live database is backed up before the release runs it (*Go live*).

## Threats

| Threat | Control |
|---|---|
| Text in an event steers a Chair or a started chain (OWASP LLM01); a public issue is written by strangers | `tell` and `start` deliver the event's words fenced as evidence, and `title` and `goal` of a `start` read only what the owner wrote in the expression; the sender's words go in `quote` |
| A flood of events becomes a flood of wakes, because nothing batches | Each firing is one queued message; a subscription's `Cooldown` and the `start` quota hold; no batching is built (*Revisit when*) |
| An event a subscription should have caught is dropped without anyone seeing | Its reading is stored and the Events tab says that no subscription held for it |
| An injected Assistant adds a subscription that starts chains on strangers' issues | A subscription whose action is `start` or `tell`, set by anyone but the owner, should wait for his Apply (`holdEventSignal`'s pattern); the seam is `createSubscription`, the gate is not built in this change (*Revisit when*) |

## Consequences

- One place says what happens to an event: the subscriptions on its key, each a sentence. The signal page stops carrying a second
  rule language.
- A Dispatcher's judgement is the owner's to ask for: a subscription that starts a triage chain on the events he names.
  Nothing starts one for him.
- A burst on one chain is N messages in the Chair's queue, in order; whether the Chair's turn reads them together is the Chair's
  queue (`take`), not the signal's.
- Two subscriptions on one event both fire: nothing claims. A rule that must not overlap another says so in its own `when`.
- Admission has one pool fewer, and `OriginDispatcher` leaves ten places.

Tests first, one for each behaviour: a reading goes to each subscription whose `when` holds and to none whose `when` fails (a
field the event lacks makes it false); `tell` queues one fenced message in the target chain's Chair and no message for a chain
that is finished or not found, with the outcome saying why; `tell` skips Remit's own comment; the job `sync-merge-request` records the forge's
answer and not the event's claim; the job `mark-live` marks the change live; one action kind serves a `fact:` reading as it serves an
event (`fact:chain.finished` → `start`, its `goal` read from the reading); `start` queues a chain whose goal is the owner's and whose quote
is fenced, up to the quota; a replayed reading fires once; an event nobody subscribed to is stored with no firing; the migration
turns a stored rule into a subscription that fires on the same event, and a second run changes nothing.

## Revisit when

Events flood a chain's Chair (a coalescing seam in the Chair's queue, not in the signal); the owner wants a triage agent that
shipped with Remit; the Apply gate for `start`, `tell` and `run` set by the Assistant is wanted (build it in `createSubscription` and `updateSubscription`); `when` is to be shown in words rather than as written on the Subscriptions tab (ADR-0035 limits `end` to Remit's own
subscriptions and says nothing of who may hold `run`); a provider plugin supplies its own subscriptions and jobs; two
subscriptions on one key need an order; the owner wants a pause button on one subscription's page (today he tells the Assistant, `pause_subscription`,
or pauses the whole signal with its rail).
