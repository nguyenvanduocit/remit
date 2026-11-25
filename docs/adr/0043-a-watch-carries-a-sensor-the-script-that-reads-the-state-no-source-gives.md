# 0043. A watch carries a sensor, the script that reads the state no source gives

Status: accepted, 2026-11-03; partly superseded by ADR-0047: the watch's sensor is a chain sensor of its own table, made by the Chair's `create_sensor`, and `subscribe` takes no sensor. Builds on ADR-0013 (a chain owns its outcome until the effect is verified), ADR-0014 (signal is the
backbone), ADR-0018 (one gate) and ADR-0034 (a plugin supplies a source). Source: the owner's request of 2026-11-03: a chain that
needs a signal no source has should bring it in with a script, without waiting for a plugin to be written.

## Context

A chain waits on a signal through a watch (`subscribe` with a key). The keys a chain may watch are a rollup and its own records
(`readableBy`, `watch.go`); an event key is refused. A new source is a plugin (ADR-0034), which is a process the owner applies. While
a chain waits its sessions close, and with them anything an agent runs in the background (`tools.go`, `subscribe`). So a chain that
needs a state nobody gives (a deployment's status, a queue's depth, the content of a page) had two moves: poll by clock wakes, which
cost one of its 24 wakes a day (`wakesPerDay`) for every look and carry no replay safety, or stop waiting.

The script tool already runs a script behind a connection's proxy in a jail with no credential in it (ADR-0018, `source.Set.scriptExec`),
for the sources that implement `Injected`: GitHub, Grafana, Datadog, and an extension that declares only `auth`.

## Options

1. A script field on an event signal. The owner would write scripts and every one is a standing use of a credential, so each waits
   for his Apply; against *Agent-centric* and the speed asked for.
2. A bundled plugin that runs a stored command. A plugin is an unconfined process (ADR-0034, *Revisit when*), a larger risk than the
   jail for a read.
3. **A watch carries a sensor: a script Remit runs on a period while the watch is open, through the script tool's jail and the
   chain's own grants, whose output is the next reading of the chain's own key.** Chosen.

## Decision

- **`subscribe` takes `sensor`** (`SensorInput`: source, connection, language, code, every) together with `key` and `until`. The key is
  `chain:<id>.sensor.<name>`, in the chain's own namespace and nowhere else, so a script never writes a record Remit writes
  (`chain:<id>.mr.*`, `chain:<id>.live`) or a signal another source gives (`sensorFor`). `signals` stays the capability a watch needs.
- **The watch stores the sensor** (`model.Subscription.Sensor`), so it is one record with its condition, deadline and key: it is
  replaced by the same watch, ends with its deadline, its firing or the chain, and counts against `subscriptionsMax`.
- **The script prints one JSON value, the state as it is now.** Remit runs it every `every` (at least 1 minute, default 5) as the
  chain's Chair, so `grantPolicy` decides it for that chain exactly as it decides the `script` tool: a read behind the proxy that the
  capability gives, an open request otherwise. A value that differs from the key's newest is the next reading (`keepSensed`), so an
  unchanged state wakes no one and a replay stores nothing twice. The watch's condition judges it as it judges a rollup.
- **The first run is the Chair's, at `subscribe`.** A script that fails, prints no JSON or is refused by the gate is an error to the
  Chair while it can fix it, and nothing is stored. After that `pollSensors` (from `Tick`, as `pollRollups`) runs each open sensor in
  the background, never under `tickMu`, one run at a time for a watch, none for a paused chain.
- **`source.Set.RunScript` is the one new door in `internal/source`**: the script tool's run core (`scriptRun`) answering stdout alone,
  through the same gate, policy and recorder, so a run is on the owner's record of calls like any other read.
- **The owner sees it where he sees the watch.** `SubscriptionLine.sensor` carries source, connection, language, period and the script;
  the Subscriptions page and the chain's page show it as *Reads with*, the script under *Show the script*. A sensor whose script has
  failed `silentAfter` runs in a row holds its watch's line with the last error (`sensorSilence`), and the deadline still wakes the Chair.
  The new source `sensor` is on the Signals board with the shape of its key.

## Consequences

- A chain can wait on any state an API with a script-capable connection can read, today and without the owner's Apply, inside the
  grants it already holds. An API with no source is one `auth`-only extension away (ADR-0034), not a plugin binary.
- A source that is not `Injected` (a database, a plugin that only has tools) has no sensor. The Chair is told which sources run scripts
  (`list_sources`); the typed tools of the others stay the way to read them.
- Cost of change tomorrow: a second language is a line in `scriptLanguages`; a sensor on an event key or a chain-free scope is a new
  key shape in `sensorFor`, not a new mechanism.

## Threats

| Threat | Control |
|---|---|
| A script reads where the chain has no grant | The run is as the chain's Chair, through `grantPolicy`; a refusal is the Chair's error at `subscribe` and a silence afterwards |
| A script forges a record Remit writes | The key is only `chain:<id>.sensor.<name>` (`sensorFor`) |
| Text a script prints steers the Chair (OWASP LLM01) | The wake delivers the reading fenced as evidence, as for a rollup (`watch.go`); the reading is data to a condition |
| A sensor keeps a credential in use after it is of no use | It runs only while its watch is open and its chain not paused; the deadline ends it |
| Many sensors read one API | One run at a time for a watch, at least a minute apart, at most `subscriptionsMax` watches a chain |

## Revisit when

- **A limit on the sensors of all chains together** (runs an hour against one connection): the seam is `pollSensors`.
- **A sensor without a connection** (a public address): the script tool's proxy needs a credential today (`InjectSpec.Check`); it needs
  an egress rule the owner approves.
- **The owner wants a sensor to outlive its chain**: promote it to an event signal with a pull (ADR-0024) through the Assistant.
- **A script prints a stream, not a state**: a cursor and items as `Poll` has them (ADR-0024).

Enforced by: TestAChainsSensorWritesStateReadingsAndWakesTheChairWhenItMeetsTheCondition, TestACreateSensorIsRefusedWithTheReasonAndStoresNothing, TestAChainsSensorWhoseScriptKeepsFailingIsHeldOnItsWatchLineAndTheFirstGoodRunClearsIt, TestAChainsSensorRunsOnlyWhileAWatchOnItsKeyIsOpenAndItsChainNotPaused, TestRunScriptAnswersWhatTheScriptPrintedAndNothingElse, TestRunScriptIsHeldToTheGrantOfTheCallerAndToTheScriptRules, TestASensorsScriptRunsInTheRealJailAsTheChairAndPrintsTheStateAlone
