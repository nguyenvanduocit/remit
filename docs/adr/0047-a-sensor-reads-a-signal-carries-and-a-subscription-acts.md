# 0047. A sensor reads, a signal carries, a subscription acts

Status: accepted, 2026-11-03; partly superseded by ADR-0049: a sensor has a `kind` (`poll`, `script`, `clock` or a plugin's), time is a sensor, and the one loop senses by kind (`Sensing`); its waits on the owner, but a secret act, are an agent's (ADR-0087). Builds on ADR-0014 (signal is the backbone), ADR-0024 (everything that comes in is an event signal),
ADR-0036 (every rule is a subscription), ADR-0043 (a watch carries a sensor) and ADR-0044 (a bundled `script` plugin). It replaces
the pull of ADR-0024 and the watch-carried sensor of ADR-0043 with one record, the sensor. Source: the owner's words of 2026-11-03:
"a sensor reads data, calls the API, and emits a signal; the subscriptions listen and process; the processor lives in the
subscription".

## Context

Three things read the outside world on a period and turned what they read into readings, each with its own record and its own
loop:

- **A pull** (`EventSignal.Pull`, `pull.go`): a source's connection read from a cursor; each new or changed item is an event of the
  event signal that holds the pull.
- **A sensor** (`Subscription.Sensor`, `sensor.go`): a script behind a connection's proxy, owned by one chain's watch; what it prints is
  the next reading of `chain:<id>.sensor.<name>`, and it ends with the watch.
- **A rollup** (`rollup.go`): a rollup of the owner's telemetry store, read while a watch listens.

The pull lived inside the signal, so a signal could not exist without declaring its reader and could not have two; the sensor lived
inside the subscription, so what listened owned what read, and no second subscription could hear it. The two did the same job
(*reads, then writes readings of a key*) under two names, with two loops, two silence counters and two screens. The owner's model has
three nouns with one job each: the **sensor** reads, the **signal** carries, the **subscription** listens and acts, its action being
the processor.

## Options

1. **Rename the pull to "sensor" in the UI and the docs.** One word, two records and two loops stay; the watch's sensor still belongs
   to its subscription.
2. **A sensor field on the signal for both** (`EventSignal.Sensors`, and a record for each chain's key). The reader stays a part of
   what it feeds; a chain's signal has no record to hold it.
3. **A sensor is a record of its own** (table `sensors`, one loop, `pollSensors`), naming the signal it writes. A signal and a
   subscription hold no reader. **Chosen.**

## Decision

- **`model.Sensor`** is one stored record: `id`, `name`, `note`, `chain` (the chain it reads for; empty for a standing sensor), `signal`
  (the key whose readings it writes), what it reads, `everyMinutes`, `cursor`, `paused`, who made it, and its revision. What it
  reads is exactly one of:
  - a **poll**: `source`, `connection`, `query`, answered in items. Each new or changed item is an event of an `event:<name>`
    signal, written through `Emit` as before (`ViaPoll`), so the cursor, dedupe by `id@version` and "the first read emits nothing
    older than the sensor" are unchanged;
  - a **script**: `source`, `connection`, `language`, `code`, run behind the connection's proxy as the chain's Chair
    (`source.Set.RunScript`). It prints one JSON value, the state as it is now, which is the next reading of
    `chain:<id>.sensor.<name>` when it differs from the newest.

  `Sensor.Check` (one pure function) is the only place the pairs are decided: a poll writes an event signal and has no chain; a
  script writes its chain's own sensor key and belongs to that chain. A sensor is built through it at the boundary and trusted
  inside (*illegal states unrepresentable*).
- **One loop.** `pollSensors` (from `Tick`, in the background, never under `tickMu`) runs every sensor whose period has passed, one
  run at a time for a sensor, with the silence counter of `polls.go` (`silentAfter`). `pull.go` and the old `sensor.go` loops are one
  file. A standing sensor runs while it and its event signal are not paused; a chain's sensor runs while its chain is not paused and
  an open watch listens to its key, so a credential is in use only while something waits on what it reads (ADR-0043's control, kept).
- **The signal holds no reader.** `EventSignal.Pull` and `Subscription.Sensor` are removed. An event signal is its name, note,
  door, key and pause; its sensors are the records that name it. A subscription is a key, a condition and an action, and never
  owns what feeds the key. A reader can be replaced (a plugin, a script, a door) with no subscription touched.
- **The processor is the subscription's action**: `tell`, `start`, `run`, `wake` (ADR-0036). No new concept sits between a signal
  and its action.
- **Who makes one.** A standing sensor is made by telling the Assistant (`create_sensor`, `update_sensor`, `pause_sensor`,
  `resume_sensor`, `delete_sensor`, `list_sensors`); one that reads a source, a connection, a query or a script the sensor did
  not read before waits for the owner's Apply (`ChangeSensor`, `sensorRail`), as the pull did, since it is a standing use of a
  credential or of the host. A chain's Chair makes its own with `create_sensor` and watches the key with `subscribe`; `subscribe`
  takes no sensor. The Chair's first run is made when it creates the sensor, so a script that fails is told to the Chair while it can
  fix it. A chain's sensors end with the chain.
- **A bundled plugin ships its sensors with its signals** (ADR-0037): the shipped event signal is made with its sensor once, and the
  sensor is the owner's after that.
- **The owner sees it where he sees the signal.** `Snapshot.sensors`, `sensorUpdated` and `sensorDeleted` carry the record as they
  carry an event signal's. An event signal's page lists its sensors under *Sensors* (what it reads, how often, the last error when
  silent, a script under *Show the script*); a chain's watch shows its key's sensor as *Reads with*. The Signals board keeps the
  source `sensor` for the chain keys.
- **Stored data moves once** (migration 66): each event signal's pull becomes a standing sensor of that signal, named for it, with the
  cursor it had; each subscription's sensor becomes a chain sensor; both fields leave their records; an open change item that carried
  a pull is cancelled (the owner asks again through `create_sensor`).

## Consequences

- One reader per kind of thing is one loop, one silence rule, one status in the UI and one Apply. The `replies` signal is a sensor
  whose poll is a `script`; a Datadog monitor is a sensor whose poll is `reading:…`; both feed signals subscriptions already hear.
- A signal can have two sensors and a sensor can be replaced under a standing subscription. A chain's state is a signal with a
  subscription, like any other.
- Cost of change tomorrow: a sensor whose script prints a stream into an event signal is a new pair in `Sensor.Check`; the
  rollup read becoming a sensor is a new read kind there.
- `event-signals.md`, `design.md` and the Assistant's tools say *sensor* for everything that reads; *pull* survives only as the verb
  of a source's `Poll`.

## Threats

| Threat | Control |
|---|---|
| A reader starts reading a new place without the owner | A standing sensor that reads a source, connection, query or script it did not read before waits for his Apply (`sensorRail`); the poller's gate verdict allows exactly the source and connection of a live sensor (`pullVerdict`) |
| A script forges a record Remit writes | `Sensor.Check`: a script writes only `chain:<id>.sensor.<name>` of its own chain |
| Text a sensor reads steers an agent (OWASP LLM01) | Readings reach a Chair fenced as evidence, events as for any writer; a sensor never acts |
| A sensor keeps a credential in use after it is of no use | A chain's sensor runs only while its chain lives, is not paused, and a watch on its key is open |
| Many sensors read one API | One run at a time for a sensor, at least a minute apart; a limit on all sensors together waits for hardening (seam: `pollSensors`) |

## Revisit when

- **A limit on the sensors of all chains together** (runs an hour against one connection): the seam is `pollSensors`.
- **A script prints a stream, not a state**, into an event signal (a jailed reader of an API with items): a new pair in
  `Sensor.Check`, not a new mechanism.
- **The rollup read becomes a sensor** (a telemetry rollup is a read of a source on a period): a read kind of `Sensor`, and
  `rollup.go` goes.
- **A sensor without a connection** (a public address): the script tool's proxy needs a credential today; it needs an egress rule
  the owner approves.
- **A jail in front of a host-run script** when the owner asks for hardening: the `Runner` seam of ADR-0044.

Enforced by (`internal/model/sensor_test.go`): TestASensorCheckPairsAPollWithAnEventSignalAndAScriptWithItsChainsKey, TestASensorsWriterIsAPollOrAScript.
(`internal/store/sensor_test.go`, `migration66_test.go`): TestASensorIsStoredByIdAndNameAndCountsItsRevisions, TestTwoSensorsOfOneOwnerNeverShareAName, TestSensorsAreDeletedWithTheirChainAndWithTheirSignal, TestSensorMigrationMovesAPullAndAWatchsSensorIntoSensors.
(`internal/orch/sensor_test.go`): TestAStandingSensorWritesItsItemsAsEventsAndStoresItsCursor, TestAReplayedPollIsOneEvent, TestASilentSensorIsSaidOnItsSignalsLine, TestASensorIsReadOnceAPeriodAndNotWhileItOrItsSignalIsPaused, TestTheGateLetsThePollerReadOnlyWhatASensorNames, TestASensorTheAssistantMakesWaitsForTheOwner, TestDeletingAnEventSignalDeletesItsSensors, TestAPolledDatadogReadingIsANumberAMeasureReads, TestAChainsSensorWritesStateReadingsAndWakesTheChairWhenItMeetsTheCondition, TestACreateSensorIsRefusedWithTheReasonAndStoresNothing, TestAChainsSensorWhoseScriptKeepsFailingIsHeldOnItsWatchLineAndTheFirstGoodRunClearsIt, TestAChainsSensorRunsOnlyWhileAWatchOnItsKeyIsOpenAndItsChainNotPaused.
