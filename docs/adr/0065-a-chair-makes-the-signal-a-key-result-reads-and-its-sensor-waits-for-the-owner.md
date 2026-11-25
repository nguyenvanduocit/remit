# 0065. A Chair makes the signal a key result reads, and its sensor waits for the owner

Status: accepted, 2026-11-05; its waits on the owner, but a secret act, are an agent's (ADR-0087). Builds on ADR-0047 (a sensor reads, a signal carries, a subscription acts), ADR-0045 (a measure is
read from a signal, proven by a dry run) and ADR-0062 (the Assistant decides for the owner, he overturns).

## Context

On the dev instance an `objective` chain (card `c_4b9360643e75`, capabilities `extensions, objectives, shell, signals, web`) needed
`event:lights-daily` to read for its key results. It sent the owner three questions (`i_fb123195f8a0`, `i_62972e388490`,
`i_ba56acdc4d79`) that spelled out the steps ("ask the Assistant for it with source `script`, connection `default` ... then approve
the pull"), with a preset option "Created and approved now". The option was picked twice while nothing existed, and the chain
believed it.

Root cause: a Chair's only sensor tool was the chain's script sensor (`create_sensor`, `addChainSensorTool`), whose key is
`chain:<id>.sensor.<name>` and which runs only while a watch on it is open, so no key result can read it. `create_event_signal`
and the standing sensor's `create_sensor` were the Assistant's alone (`addEntranceTools`). The chain had no route but to hand the
owner a recipe in free text, and a free-text answer proves nothing. The machinery the chain needed already existed: `createSensor`
holds any sensor written by anyone but the owner or Remit (`holdSensor`) as a change item, and Apply stores it.

## Options

- **A. The Chair makes the event signal at once and asks for the standing sensor, which waits for the owner (chosen).** Two tools
  given by `signals`, each calling the action the Assistant's tool calls, by the Chair's card. An event signal moves no rail (it
  lets events in, and its key is shown to the owner alone); the sensor's write is the rail, and it already waits.
- **B. The event signal is made inside the sensor's Apply.** One tool, nothing written until the owner answers. But `createSensor`
  checks the signal exists (`checkSensorSignal`), a signal that only some writers post to would need a second path, and Apply
  would write two records of two kinds: one concept with two owners.
- **C. The chain routes through the Assistant.** A message to the Assistant asking it to make both. It adds a hop whose result the
  chain cannot see as its own, and the item would be raised by the Assistant rather than the chain: the same rail, with an
  extra agent and no record of which chain asked.

## Decision

A Chair whose chain lists `signals` holds `create_event_signal` (`o.createEventSignal`, by `model.ByAgent(card)`, written at once)
and `create_standing_sensor` (`o.createSensor`, by the Chair, held by `holdSensor` as a change item; Apply runs `applySensor`, and
the Chair is told `[change result · …]`). The name `create_standing_sensor` keeps the chain's own `create_sensor` (a script for
a watch) apart. Registration is in `addCapabilityTools` beside the Chair's other `signals` tools (`addStandingSignalTools`,
`sensor_tools.go`); `signals` stays a capability that does not write, because the one write that matters waits for the owner.
The OKR briefs (`okr-instrument`, `objective`) and the okr skill tell the Chair to make both itself and never to ask the owner
or the Assistant to.

## Consequences

- A chain that needs a number read after it ends builds it in one turn, and the owner answers one Apply with the whole query in
  front of him, instead of following a recipe.
- An event signal a chain makes shows on the Signals tab with the Chair as its maker; the owner pauses or deletes it as any other.
- A chain proposed by another chain, or started by an event, may open event signals (`signals` is proposable). Each is inert until
  a sensor, a subscription or a writer holding its key acts on it, and the sensor still waits for the owner.

Enforced by: TestAChairWithSignalsOpensAnEventSignalAndItsStandingSensorWaitsForTheOwner.

## Revisit when

- A chain opens an event signal's door unasked (a signal no sensor of its own writes, used as a way in for an outside system):
  then, when hardening is asked, hold door creation behind the owner (a `ChangeEventSignal` that moves a rail), keeping the
  sensor's signal written at once.
- Chains open signals faster than the owner can read the Signals tab: a limit on how many event signals a chain may open, a field
  of `Config` with its flag.
