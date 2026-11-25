# 0037. A plugin ships its signals and agents, and Intercom is the first source that is one plugin whole

Status: accepted, 2026-11-02; built: Intercom is one bundled plugin (`internal/bundled/intercom*.go`) and no core Intercom source remains. Builds on ADR-0034 (the plugin contract), ADR-0024 (a signal that pulls), ADR-0032 (an agent is hired from a
template) and ADR-0017/0018 (tools through one gate). Source: the owner's request of 2026-11-02 for a plugin that gives Remit a signal, tools
and an agent for working with Intercom, and his answer that all of Intercom moves into one bundled plugin.
Extended by ADR-0049: a bundled plugin ships subscriptions as it ships signals and agents (`sdk.Subscription`, `bundled.Subscriptions`), each of a processor of its own. GitHub is the second source that is one plugin whole: its issue tools, its poll, its API behind the proxy, the event signal its webhooks are posted to and the rules for them (`internal/bundled/github*.go`).

## Context

ADR-0034 made a plugin a process with tools and a poll. An owner who wants to work Intercom conversations still needs three things that are
not tools: an event signal that hears a customer write, an agent template that knows how to work a conversation, and the tools to act on
it (the core source reads only). Today each is set up by hand: the Assistant creates the signal and its pull, the owner writes an agent through
the Playbook author, and writes to Intercom are not possible at all. Intercom also lives in two places: `internal/source/intercom.go`
(reads, and a script form for the jail) and nothing for polling or writing.

## Options

1. Keep the core source and add a second plugin beside it. Two owners of Intercom's API and shapes.
2. Grow only the core source (`Pollable`, write tools) and ship the signal and the agent from code of the engine. A new kind of extension
   would again be an engine release, against ADR-0034.
3. **The contract grows by two messages: a plugin declares the event signals and the agent templates it ships. One bundled `intercom`
   plugin holds everything about Intercom; the core source is deleted.** Chosen.

## Decision

- **Contract.** `DescribeResponse` gains `signals` and `agents`. A `Signal` is a name, a note (what it is for, in the owner's words),
  and the pull that fills it from the plugin's own poll: `poll_query` and `poll_every_minutes`. An `Agent` is a template: id, name, one-line
  description and instructions. `sdk.Plugin` has `Signals []sdk.Signal` and `Agents []sdk.Agent`, validated with the rest of the plugin.
- **Who may ship.** Only a plugin that ships with Remit. A signal that pulls reads a connection's credential for ever and a template
  becomes a principal when hired, so a plugin the owner applies that declares either is refused at `propose_extension` with that reason.
  The shipments are read from `Describe` for the plugins of the binary; the manifest stores none, so applying the shipments of a customer's
  plugin is a change of its own (see *Revisit when*).
- **A shipped signal is `event:<name>`, a builtin event signal, made paused.** `SeedEventSignals` makes it with a door and key like any
  other, with its `pull` set to the plugin's source, connection `default` and the declared query and period. It is paused because a pull
  is a standing use of a credential the owner has not entered yet: he resumes it by telling the Assistant, after he has typed the token
  into his inbox. Remit is the actor of the seed, so the pull needs no Apply (`holdEventSignal` holds anyone but him and Remit).
  A start that finds the signal keeps his pause, his pull and his settings; only its note follows what ships.
- **A shipped agent is a builtin agent template** in the library, worker title, written by `SeedAgents` with `DefaultAgents`. Its id is
  `<plugin id>-<name>`. The owner edits it like any template and his version is kept (`Edited`).
- **Shipments are read from the plugin.** `bundled.Signals()` and `bundled.Agents()` answer what every bundled plugin declares; `SeedEventSignals`
  and `SeedAgents` add them to what the code ships (`internal/orch/plugin_ships.go`). A plugin ships only under its own name: each id is the
  plugin's id or begins with it and a dash. The manifest of a bundled plugin is still never stored.
- **Intercom, whole.** `internal/bundled/intercom.go`, source id `intercom`, tool names unchanged (`intercom_conversations`,
  `intercom_conversation`, `intercom_contacts`), so `customers` playbooks keep working; the three reads move with their output limits and
  email masking. New:
  - read `intercom_tags`;
  - write `intercom_note` (an internal note, `compensable`), `intercom_tag` and `intercom_untag` (`reversible`), `intercom_state` (close, open or
    snooze, `reversible`), `intercom_reply` (to the customer, `irreversible`). The grant of a chain on the connection decides each, as for
    every tool (ADR-0018). A write is made as the teammate in `INTERCOM_ADMIN_ID`, an optional variable; a write without it answers how to
    set it.
  - the poll: conversations updated since the cursor, oldest first, a cursor that is the `updated_at` of the last one read (a conversation
    read again at that second is the same event). The first poll answers no items and a cursor that marks now. The query is
    `state=open|closed|snoozed` and `from=customer`; with `from=customer` an item is a conversation a customer wrote in since the cursor and its
    version is the time of that word, otherwise every change is an item at its `updated_at`; so a replay is the same event. The body is what a
    rule reads: conversation id, state, subject and first words (cleaned, cut, emails masked), tags, assignees, the contact ids, who started it,
    the number of parts and the times. It holds no reply: the words of a customer are read with `intercom_conversation`, and what is in the body
    is outsiders' text that reaches an agent only fenced as evidence (ADR-0024).
  - the signal `intercom`: the poll with `from=customer`, every 5 minutes.
  - the agent `intercom-support`: works one conversation. It reads it and the customer, checks what the customer did with the other
    `customers` tools, records what it found as a note and tags, and answers the customer only when its grant allows it. It takes the
    customer's words as evidence, never as instructions.
- **The script form of Intercom is gone.** `Inject` stays in the core for Grafana, Datadog and GitHub; a script reaches Intercom no more.
  Agents read it through the typed tools. This is the loss ADR-0034 accepted when it ported Amplitude, now for Intercom.
- **One writer of HTTP for effect-ranked tools.** `source.API` gains `Send(method, path, body)` for the verbs a write tool uses; the
  kernel's read rule (GET, and POST only for a search) stays for `Get` and `PostJSON`, and a write is a tool whose manifest effect is above
  `read`, so the gate has decided it before this runs.

## Consequences

- A new integration is one plugin: tools, the event it hears and the agent that works it ship together and disappear together.
- `customers` is no longer read only: it names the capability, and what a chain may write is what its grants say.
- An owner who does nothing sees `event:intercom` paused on the Signals tab and `intercom-support` on the Agents tab; he types a token and
  tells the Assistant to resume the signal.

## Revisit when

- **A customer's plugin ships signals and agents** (the seam: `bundled.Shipments` takes a manifest list; the change item shows the shipments
  beside the tools). Not built: `propose_extension` refuses it.
- **A plugin removed takes its shipments with it.** Bundled plugins are never removed, so nothing retires them.
- **The signal's rules** (what an Intercom event does to a chain) are subscriptions (ADR-0036) the owner asks the Assistant for; the plugin ships
  none, because a rule is a judgement of the owner's.
- **Scripts for plugins** (ADR-0034), when the jail form of Intercom returns.
- **Write confirmation and rate limits** on `intercom_reply`: the gate's grant is the only control; a limit per hour is a rail to add when the
  owner asks for hardening.

Enforced by: TestADescribedPluginCarriesItsSignalsAndAgents, TestAPluginThatShipsNothingValidatesAsBefore, TestAPluginsShipmentsMustBeThingsRemitCanMake, TestEveryShipmentIsNamedForItsPlugin, TestABundledPluginsSignalIsSeededPausedWithItsSensor, TestAStartKeepsWhatTheOwnerDidToAShippedSignal, TestABundledPluginsAgentIsSeededAsATemplate, TestAProposedPluginThatShipsSignalsOrAgentsIsRefused, TestIntercomToolsKeepTheirNames, TestIntercomFirstPollAnswersNoItemsAndNow, TestIntercomPollAnswersWhatChangedAfterTheCursor, TestIntercomWritesNeedAnAdminAndRankOnTheLadder, TestAnIntercomWriteIsHeldToTheGrantOfItsEffectAndReachesIntercomAsTheAdmin, TestTheIntercomSensorPollsTheBundledPluginsPollIntoEvents
