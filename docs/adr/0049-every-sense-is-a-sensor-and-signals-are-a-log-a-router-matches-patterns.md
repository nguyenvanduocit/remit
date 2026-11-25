# 0049. Every sense is a sensor, signals are a log, a router matches patterns

Status: accepted, 2026-11-03; the merge-request sense is a plugin's, not a built-in sensor, by ADR-0057; partly superseded by ADR-0064: a reading enters through `ingest` and only the Tick judges it, every reading is stored (no transient reading), time is a subscription's own `Schedule` (no clock sensor), a firing is recorded before its effect is delivered outside `tickMu`, and a cooldown is a named `Flow`. Builds on ADR-0014 (signal is the backbone) and ADR-0047 (a sensor reads, a signal carries, a
subscription acts); it widens ADR-0047 from the sensors that read the outside world to every writer of a signal, and replaces the
exact key of a subscription and the clock subscription of ADR-0014. Source: the owner's words of 2026-11-03: "everything is a
sensor, the senses of this system, the clock too; it produces values and stores them, in the database or in memory, and that is
a signal; then a router distributes the signals to the subscribers that subscribed to a key pattern, which is a filter; the
subscriber processes the data itself, programmatically or agentically"; "some sensors are temporary: those need no storing";
"use an event-driven or event-sourcing mechanism for the pipeline".

## Context

ADR-0047 gave the outside-reading sensors one record, but the system still has three shapes the owner's model does not:

- **Time is not a sensor.** `ClockKey` "has no readings" (`internal/model/signal.go`). A clock subscription carries `Cron` and `Next`,
  the evaluator has a clock branch (`ReasonClock`, `Rescheduled`), and the scheduler tick reads `due_at` from subscriptions.
- **Writers are many and unnamed.** The door, a rollup, a merge request, "live" and the facts each call `emit` from their own place;
  only the poll and script writers are a record (`sensors`).
- **A subscription hears one exact key.** `Subscription.Key` is one `SignalKey` and its cursor `SeenRev` is a revision of that one
  key, so a subscription cannot listen to a family (`event:*`, `chain:*.live`), and the router (`emit`, `Evaluate`) is a lookup by key.

The reading log is already an event log with consumers: readings are append-only per key, a subscription holds an offset (`SeenRev`),
a firing is keyed by (subscription, key, revision) so a retry stores nothing twice. What is missing is one offset across keys, a
filter instead of a key, and readings that are not kept.

## Options

1. **Keep per-key revisions, add a pattern.** A subscription would hold a cursor per matched key (a map). Rejected: a cursor that is a
   map grows with the keys, and "which readings has it not judged" becomes a query over a map.
2. **Event-source the whole application** (state derived from the log, replayed on start). Rejected: the architecture articles
   (Event Sourcing, vol. 03) say most applications do not need it, and Remit's state (chains, playbooks, agents) is not a
   projection of signals. Only the *signal pipeline* is a log.
3. **One log with one global offset; consumers hold an offset and a filter; transient readings are routed from memory.** Chosen: it
   is the log-and-consumer-offset core of Kafka-style pub/sub, with the durable/transient subscriber distinction of Pub/Sub
   (vol. 03: a transient delivery reaches those listening now, a durable one is replayed from the offset).

## Decision

**A sensor is anything that senses and writes readings of a signal.** The sensor kinds: `poll` (a source's connection from a
cursor), `script` (a script behind a connection, for a chain), and `clock` (an instant: cron, a period, or a moment). The server's
own senses (the door, a rollup, a merge request, a change going live, the facts it records) are sensors too, built into the
server: they are kinds of sensor that need no record because the owner configures none of them. A kind that is a record has a
`sensors` row; one that is built in is listed on the Signals tab as a source (ADR-0014). A new sense is a new kind.

**A signal is a log.** Every reading of a stored sensor is appended to one log with a global `seq`, strictly increasing, kept beside
its key and the key's own revision (`rev`, unchanged: a revision still names a reading of one key). `seq` is the offset. A sensor
is `transient` when its readings are not worth keeping: the clock's are. A transient reading takes a `seq` from the same counter,
is routed from memory and never stored: it exists for the subscriptions listening when it is written, and no one reads it later.

**A subscription is a filter and an action.** It holds `Match`, a pattern over keys (SQLite `GLOB`, the syntax `SignalRows`
already takes: `*`, `?`, `[...]`; a key with none is an exact key), `Expr` (a CEL condition over the reading's value, unchanged),
and `SeenSeq`, the last offset it has judged. A subscription hears a stored reading when its key matches `Match`, the reading is past
`SeenSeq` and `Expr` holds; it hears a transient reading when it matches and `Expr` holds, once.

**The router is one function.** A reading enters at one place (`emit`): if the sensor stores, it is appended; then the active
subscriptions whose `Match` matches are judged, and what fires is stored with its effect in one transaction. A firing is keyed by
(subscription, key, revision of the reading it covers): a retry or a restart stores nothing twice, which is also what makes every consumer idempotent
(at-least-once delivery is the contract; exactly-once is not claimed). The tick is only for what has no reading to wake it: a
window's quiet time, a cooldown, a deadline, and a clock sensor's next instant.

**The clock is a sensor.** A clock sensor holds when it senses (cron, `@every`, or one instant) and writes the transient key
`clock:<name>`. A subscription that waits for time listens to that key. `Subscription.Cron`, `Next` and the clock branch of the
evaluator are gone. A one-time clock sensor deletes itself after it writes; a subscription that ends after its first firing says so
(`Once`). A clock sensor is the subscription's own: deleting the subscription, by id or with its owner, deletes its clock sensor in the same write (`store.DeleteSubscription`, `TestDeletingAWaitDeletesItsClockSensor`). A chain's clock sensors end with the chain, as its script sensors do. Creating a time wait is one application function that
makes the clock sensor and the subscription together.

**The subscriber processes.** Its action is the processor, programmatic or agentic as the owner wants: `run` and `end` are code,
`tell`, `wake`, `start` and `hire` are an agent. The list of actions stays closed.

**The core is small; a plugin is the way to extend it (ADR-0034, ADR-0037).** The server keeps the log, the router, the pattern and the
closed list of actions' guarantees (idempotence, the rails); every sense and every processor beyond them is a plugin's. Two plugin
points, each one interface the core calls and a gRPC adapter that a plugin satisfies:

- **A sensor kind.** The core declares `Sensing`: a kind that, given its record, writes readings through the router. `poll`, `script`
  and `clock` are in-process implementations; the server's own senses (the door, a rollup, a merge request, live, a fact) are too.
  A plugin declares sensor kinds in `Describe` and implements `Sense(SenseRequest)`, a stream of readings the host appends
  (stored or transient as the kind says), or the existing `Poll`. A `Sensor.Kind` names a kind; a record of an unknown kind is held
  silent with the reason, never dropped.
- **A processor.** The core declares `Processing`: what a firing does. `run`, `end`, `tell`, `wake`, `start` and `hire` are in-process;
  a plugin declares processors in `Describe` and implements `Process(ProcessRequest)`, which gets the reading and the subscription's
  parameters and answers what it did. A subscription's action names a processor, and a plugin's is `plugin:<id>.<name>`.

A plugin that senses or processes through a connection is lent that connection's credential variables through the one gate (`Set.Lend`, the caller `sensor` or `processor`, and the effect the processor declares); a processor may write
`event:<suffix>` readings of the event signal its plugin ships, as a plugin's sense does. The Assistant makes a sensor of a plugin's kind with `create_sensor` (`kind`, `settings`, checked against the kind's schema; `list_sensor_kinds` names them),
and the owner's Apply sees both. GitHub and GitLab are plugins whole (their tools, polls, event signals and rules), and the merge-request sense stays a built-in sensor: it reads the forge through the `gh` and `glab` logins of the machine,
named by the server's own flags (`-gh`, `-glab`, `-forges`), which no plugin is lent. It is the sensor and the subscriber of the model, split where the credential ends: the sense (`senseMR`) asks the forge and, when
the forge says a merge request stands otherwise than recorded and than it last said, writes what the forge answered (`iid`, `state`, `mergedAt`, `closedAt`, nothing of Remit's) as a reading of
`chain:<id>.sensor.merge-request.<repository>`; the subscription `sub_run_record_merge_request`, shipped with Remit on `chain:*.sensor.merge-request.*`, runs the job `record-merge-request` (`fireRecordMR`), the one
place that writes the chain's record, the decision, the reading `chain:<id>.mr.<repository>` and the fact `chain.merged`, under the chain's lock, a rail no plugin holds. It is idempotent (a state already recorded, a merge request the
repository has left and a chain that is gone each end in a firing that says so) and it holds while a submit works on the repository (`errNotAdmitted`, shown on the Subscriptions tab). The periodic job `sync-merge-requests`, an
event's `sync-merge-request` and a merge Remit made all call the sense, and none records.

Both reach the application layer as a channel with the plugin as the actor, and no code path is keyed on a plugin's id (AGENTS.md:
*Agent-centric*). Plugin kinds need the plugin enabled; a disabled plugin's sensors and processors are silent with the reason.

**Stored data moves once.** A numbered migration gives every stored reading a `seq` in the order of (time, key, revision), turns
each subscription's `key` into `match` and `seen_rev` into `seen_seq` (the `seq` of its last judged reading), and replaces each clock subscription by a clock sensor plus a subscription on its key. Runtime code reads one shape.

## Consequences

- A subscription can listen to a family: `event:*` (every event), `chain:*.live`, `clock:*`. The Signals tab's pattern search and a
  subscription's `Match` are one syntax.
- The sensor is the one noun for what senses: the Signals tab lists clock sensors with the others, and a signal's page names what
  writes it.
- The evaluator is simpler in time (no clock branch) and larger in matching (a pattern, a global cursor).
- A transient reading is lost to a subscription that was not running when it was written (a restart between two ticks drops one
  tick): a clock sensor therefore computes its next instant from its schedule and its last written instant, kept on the sensor, so a
  missed tick fires once on start and not for every missed period.
- `seq` has gaps (transient readings, forgotten readings); no code assumes it is dense.

## Threats

A pattern can reach more than its author meant (`*`): a subscription whose action starts a chain, written by an agent, is held for the
owner's Apply by the existing rail (ADR-0036), which compares the record including `Match`; hardening of who may listen to which
family is not built (it waits for the owner to ask for it) and has its seam in `Match` being one field every write passes.

## Revisit when

- A processor is wanted that no action covers: write it as a plugin processor; the seam is `Processing`, never a code path keyed on a subscription.
- Who may write which family of keys or run which processor is wanted: the seams are the manifest's `capabilities` and the one policy function (ADR-0034).
- A reading is needed after the fact that a transient sensor did not keep: make that sensor stored (one field), nothing else moves.
- The log outgrows one SQLite table: partition by `seq` range; consumers already hold an offset.
- A plugin may be lent the forge's login by a gate that gives it read and no write: the merge-request sense moves into the forge's plugin, writing the same reading, and the subscriber does not change.
- Pattern syntax is too weak (alternatives, regular expressions): the pattern is one type with one matcher.

Enforced by: TestAPatternSubscriptionHearsEveryMatchingKeyPastItsCursor, TestABatchGroupsByKeyAndTheCursorStopsBeforeTheFirstHeldReading,
TestATransientReadingFiresWhatItMatchesOnceAndMovesNoCursor, TestAOnceSubscriptionEndsAfterItsFirstFiringOnATransientReading, TestAClockIsDueFromItsInstantAndAClockWithNoInstantNeverIs,
TestARepeatingClockIsCountedAgainFromNowAndTheMissedTimesAreNotReplayed (`internal/signal`); TestAPatternMatchesAKeyAsSQLiteGlobDoes, TestTheLogGivesEveryReadingAndEveryTransientReadingItsOwnPlace,
TestSubscriptionsAreFoundAndReadingsReadByPattern, TestLogMigrationGivesReadingsOnePlaceInOrderAndSubscriptionsAPositionInIt, TestClockMigrationMakesEachClockSubscriptionASensorAndEverySensorAKind (`internal/store`);
TestASubscriptionToAPatternIsFiredByAnyKeyItMatchesAndNoOther, TestATransientReadingFiresItsSubscriptionsOnceAndIsNeverStored, TestAClockReadingIsNeverStored, TestASubscriptionIsDueAtTheEarlierOfItsClockAndItsDeadline,
TestASensorOfAKindNobodySensesIsHeldSilentWithTheReasonAndKept, TestAPluginsSensorWritesItsReadingsUnderItsSignalAndKeepsItsCursor, TestAPluginReadingWithASuffixThatIsNotAWordIsRefusedAndCountsTowardSilence,
TestAPluginProcessorIsGivenTheReadingAndItsParametersAndTheFiringRecordsWhatItDid, TestAPluginProcessorIsNotRunForASubscriptionThatIsNotRemit, TestADisabledPluginsProcessorIsNotFoundAndSoNothingFires,
TestAnExtensionThatShipsSubscriptionsIsRefusedWhenTheOwnerAppliesItAndMappedWhenItShipsWithRemit (`internal/orch`).
Plugin contract: TestADescribedPluginCarriesItsSensorKindsProcessorsAndSubscriptions, TestASenseStreamsTheReadingsOfItsSensorUntilTheEngineEndsTheCall,
TestAProcessGetsTheReadingAndTheParametersAndAnswersWhatItDid, TestAPluginsSensesAndShipmentsMustBeThingsRemitCanRun (`sdk/sense_test.go`);
TestASenseDeliversTheReadingsOfASensorAndStopsWhenItsContextEnds, TestAPluginThatDiesMidSenseEndsTheStreamWithAnErrorAndTheNextSenseRunsInANewProcess,
TestAProcessRoundTripsTheReadingTheParametersAndTheConnection (`internal/plugin/host_test.go`); TestAPluginThatOnlyAddsASensorKindOrAProcessorIsAnExtension
(`internal/source/extension_test.go`); TestABundledPluginCanAddSensorKindsProcessorsAndSubscriptions (`internal/bundled/bundled_test.go`). TestAShippedSubscriptionMayNameACoreActionAndWhatItIsGiven, TestAShippedSignalSaysWhetherItWaitsForTheOwner, TestAShippedSignalMayHaveNoPoll (`sdk/`);
TestTheGitlabPluginShipsItsSignalAndItsRulesAsData (`internal/bundled/bundled_test.go`); TestGitlabRulesMoveToTheGitlabPluginsIdsKeepingWhatTheOwnerDid (`internal/store/migration70_test.go`);
TestAForgeMergeIsSeenByThePluginSensorAndRecordedOnce, TestAStandingWaitsForTheSubmitThatIsWorkingOnTheRepository, TestAStandingOfAnEarlierMergeRequestOrAGoneChainRecordsNothing, TestAForgeThatCannotBeReadMakesNoReading, TestTheMergeRequestSenseHasKeysOfItsOwn (`internal/orch/merge_request_sense_test.go`); TestTheGithubPluginShipsItsSignalAndItsRulesAsData, TestAScriptReachesGithubWithTheTypedToolsHeadersAndNeverPosts, TestANewGithubSignalStartsFromNowAndHearsOnlyWhatChangesAfter (`internal/bundled/github_test.go`); TestGithubRulesMoveToTheGithubPluginsIdsKeepingWhatTheOwnerDid (`internal/store/migration71_test.go`);
TestAnExtensionIsAProgramAndAnAPIBehindTheProxyAtOnce, TestAPollItemNeverCarriesTheConnectionsSecret (`internal/source/extension_test.go`); TestADescribedPluginCarriesTheAPIItIsReachedBehindTheProxy (`sdk/sense_test.go`);
TestAPluginSensorIsLentItsConnectionsVariablesThroughTheGateAndOnlyWhileASensorNamesIt, TestAPluginProcessorIsLentItsConnectionWhenARemitSubscriptionRunsItAndItsEmitsAreWrittenOnlyToItsOwnSignals, TestTheAssistantMakesASensorOfAPluginsKindWithSettingsThatFitItsSchemaAndTheOwnerAppliesWhatItReads (`internal/orch/plugin_pipeline_test.go`);
TestTheGitlabPluginsSignalIsSeededRunningWithItsRulesAndNoSensor (`internal/orch/plugin_ships_test.go`); TestDeletingAWaitDeletesItsClockSensor (`internal/store/signal_test.go`); TestTheLineOfASignalWithTwoSensorsNamesTheOneThatIsSilent (`internal/orch/sensor_test.go`).
