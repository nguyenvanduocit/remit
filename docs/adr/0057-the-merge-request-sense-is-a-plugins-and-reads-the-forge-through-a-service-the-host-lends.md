# 0057. The merge-request sense is a plugin's, and it reads the forge through a service the host lends

Status: superseded by ADR-0114 (no merge request to sense; the same pattern reads the forge's checks of a landed commit), 2026-11-19; accepted, 2026-11-03. Supersedes the one decision of ADR-0049 that the merge-request sense stays a built-in sensor; the rest of
ADR-0049 stands, and so does ADR-0020 (each host names its forge). Source: the owner's words of 2026-11-03: "move the sensors of GitHub,
GitLab, MRs and so on into plugins", and his approval, relayed by the session remit-e0, of the merge-request sense as a plugin through a
host-services broker, built as one change with the human-employees slice 4.

## Context

ADR-0049 made every sense a sensor and every plugin a possible sensor kind, and moved GitHub and GitLab into bundled plugins. One sense
stayed in the server: the merge request's. It reads the forge with `gh` and `glab` (`internal/forge/forge.go:3-4`), logged in as the
machine, so Remit holds no token of its own; and a plugin starts with the host's environment withheld (`SkipHostEnv`,
`internal/plugin/host.go`), so it has no login to read with. The decision recorded then: the sense stays built in, and the move waits for
a gate that lends read and no write.

The owner's direction is Anything-Plugin: the core stays small, and a forge is exactly the kind of thing a plugin is. A merge request
is also the one sense whose value the next slices extend (the head it stands at, how its checks stand), so the place that produces it
should be the place that changes with each forge, not the server.

## Options

1. **A service the host lends through go-plugin's broker.** The host serves one gRPC service to the plugin for the length of a call; the
   service has one method, `Standing`, which answers `forge.Standing` for a merge request the sensor's settings name. The plugin never
   holds a login, and the service has no method to merge, open or push, so nothing the plugin can ask moves anything. This is the
   framework's own path for a plugin that needs the host (`GRPCBroker`: `NextId`, `Accept`, and `Dial` on the plugin's side;
   `sdk/transport.go` receives the broker and discards it today).
2. **Lend the plugin a token.** Remit has none to lend: it would have to read the machine's login (`gh auth token`), a token that
   can merge and push, and hand it to a process whose job is to read. That breaks the rail ADR-0050 keeps (merging is one door, in core).
3. **Lend the plugin the machine's `HOME`.** The plugin runs `gh` or `glab` itself, as the machine's user, with every scope of the login
   and every other file of the account. Same objection as 2, wider.
4. **Keep the sense in core.** It works, and it is where ADR-0049 left it. It keeps the forge's code and the value's shape in the
   server, and ADR-0049 named this as the thing to revisit.

## Decision

Option 1. Three parts.

**The service.** `sdk/pluginpb/plugin.proto` gains `service Forge { rpc Standing }`: `StandingRequest{host, remote, iid}` answers
`StandingResponse{standing}`, the whole `forge.Standing` as JSON, never copied field by field, so a field the forge adds (the head, the
checks) reaches the plugin with no change to the contract. `SenseRequest` gains `forge_broker_id`: the id of the broker connection the
host serves `Forge` on for this call, 0 when none is lent. The host (`plugin.Host.Sense`) serves it for the length of the call and closes
it; the plugin dials it (`Call.Forge`). There is no other method, and that absence is the control: the service is the one choke point
through which a plugin reaches a login it does not hold.

**The policy.** What a call is lent is one function in the server, `hostServices`: today the `Forge` service to a bundled plugin's
sense, limited to the merge requests the sensor's own settings name (a plugin cannot read a repository its sensor was not made for).
The manifest's capabilities are the seam for the control not built (*Revisit when*).

**The sense.** A bundled plugin `forge` ("Merge requests") adds the sensor kind `merge-request`. The server keeps one sensor of that kind
for each chain that has an open merge request (name `merge-request`, signal `chain:<id>.sensor.merge-request`, period `mrSyncEvery`),
its settings the open merge requests as the chain records them (`repo`, `host`, `remote`, `iid`); a sensor with none is deleted, and
one is deleted with its chain. Each time it is opened the plugin asks `Standing` for each, and writes the standing, with the `iid`, as a
reading of `chain:<id>.sensor.merge-request.<repository>` when it differs from what it last wrote (the cursor, one JSON object by
repository): the state, when it was merged or closed, the head it stands at and how its checks stand there (`{iid, state, mergedAt,
closedAt, head, checks}`). The subscription `sub_run_record_merge_request` is unchanged, and so is its job: the one place that records
how a merge request stands, as a rail in core. An event about one (`sync-merge-request`) and a merge Remit made open the chain's
sensor at once (`runSensor`), so the standing is recorded before either returns, as it was; the periodic job `sync-merge-requests` only
keeps the sensors in step with the open merge requests.

**What stays in core.** Opening a merge request, merging, the forge's login and the decision of what a standing means for a chain
(`record-merge-request`). A plugin senses; the rails act.

## Threats

| Threat | Control |
|---|---|
| A plugin reads a repository it was not made for (information disclosure) | The service answers only a merge request the sensor's settings name |
| A plugin merges, pushes or opens through the lent service (elevation) | The service has no such method; `forge.Forge` writes are not in its interface |
| An unreviewed plugin gets a forge login (spoofing, elevation) | Only a bundled plugin's sense is lent the service; no plugin is lent a token or `HOME` |
| A plugin writes a standing of a merge request that is not so (tampering) | Remit's subscriber records only a state for the merge request the chain's repository names, and the next sensing replaces it |

Threat modelling (`aio-threat-models`) runs when the owner asks for hardening.

## Consequences

- The forge's reading is a plugin's: the head and the checks are added in `internal/forge` and cross the broker whole.
- The sense is one sensor per chain: it is made, changed and ended by the server in step with the chain's open merge requests, the way a
  clock sensor goes with its wait. The owner sees it among the chain's sensors, silent with the reason when its plugin is disabled.
- A disabled `forge` plugin stops the server following merge requests: the sensor is held silent with that reason, as for any plugin kind.
- Any later service a plugin needs from the host (a repository read) is a second method behind the same policy function, not a second
  mechanism.

## Revisit when

- A second plugin needs a host service, or a plugin that does not ship with Remit does: the policy function reads a `capabilities`
  list of the manifest (`forge.read`) and the owner's Apply shows it; today it is "bundled".
- A forge login that reads and cannot write exists (a read-only token the owner enters as a credential): the host service may become a
  lend of that credential, and the plugin reads the forge itself.
- A chain's merge requests span more forges than one sensor can read in one period: one sensor per host.

Enforced by: TestAPluginAsksTheHostForAStandingThroughTheBroker, TestTheMergeRequestSensorWritesAReadingOnAnyChangeOfStateHeadOrChecksAndOnlyThen, TestAForgeMergeIsSeenByThePluginSensorAndRecordedOnce, TestAPluginSensorCannotAskAboutAMergeRequestItsSettingsDoNotName, TestAPluginThatDoesNotShipWithRemitIsLentNoForge, TestTheMergeRequestSensorFollowsTheOpenMergeRequestsOfAChainAndEndsWithThem, TestGitLabSaysTheHeadAndHowItsPipelineStands, TestGitHubSaysTheHeadAndHowItsChecksStand
