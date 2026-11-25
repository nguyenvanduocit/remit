# 0044. A bundled `script` plugin reads what no source gives by running code the owner applies

Status: accepted, 2026-11-03; partly superseded by ADR-0047: the `script` source is read by a sensor, and its code is the sensor's query. Builds on ADR-0024 (a signal that pulls), ADR-0034 (a plugin is a process with full rights),
ADR-0037 (a bundled plugin ships its signals) and ADR-0043 (a watch carries a sensor). Source: the owner's words of 2026-11-03, after
the `replies` plugin: "why a jail; security is the last concern and we have not reached it; we care only about what can be executed".

## Context

A signal that needs a reading no source gives had two ways in, and `replies` (X, LinkedIn, Facebook and Reddit replies, read through the
owner's logged-in `ego-browser`) used both and found each too heavy or too narrow:

- **A plugin binary** (ADR-0034). `replies` was ~1,300 lines of Go around a 249-line `read.mjs`, a Chair, an engineer, a reviewer and a QC
  card, and three of the owner's Applies (the `extensions` capability, the extension, the signal's pull). Everything generic in it
  (cursor, dedupe by id, "the first poll answers nothing older", a failed read as an error item) is the same for every reader.
- **A sensor** (ADR-0043). It runs in the script tool's jail, whose only way out is a proxy that adds a connection's credential, so it
  reaches an HTTP API with a credential and nothing else: no browser, no local program. It prints one state value, not items, and its
  key is the chain's own, never an event signal's.

*Where we are* says hardening is built when asked and the change leaves the seam for it. Nobody has asked for a jail around a reader the
owner reads and applies himself.

## Options

1. **A browser gateway and a stream shape for the jailed script** (ADR-0043 extended). Reaches the browser, but builds a proxy, a URL
   list and a gateway for a control nobody asked for.
2. **A `script` field on an event signal's pull** (a new kind of pull in `model.Pull`, a new reader in `internal/source`). One more
   concept beside `Pollable`, and a change to the pull's record.
3. **A bundled plugin `script`.** A bundled plugin is already a process with full host rights, started as `remit plugin script` and
   called through the gate (ADR-0034); its `Poll` takes the script in the pull's `query`, runs it on the host and answers its items. The
   pull is `{source: "script", connection: "default", query: <the script>, every}`, which the pull already is. **Chosen.**

## Decision

- **Source `script`, a bundled plugin** (`internal/bundled/script.go`, written with `sdk`, capability `data`). Its `Poll` takes a query that is the code
  itself under a first line naming the runtime, `#!sh`, `#!node`, `#!python3` or `#!ego-browser` (that program's `nodejs -e`), so the owner
  reads the script as written, with nothing escaped. The plugin finds the runtime on the host and has no environment of the host's (ADR-0034). It runs the code as the server's user, with the poll's deadline less a margin.
- **The script prints items, one JSON object a line:** `{"id": "...", "version": "...", "body": {...}}`; a line that is not an item is
  the script's own log. A run that fails or prints no item line where it printed an error line gives an **error item** whose id carries
  the UTC day and the error without its numbers (as `replies` does), so a failure shows once a day and a partial read never looks like
  "nothing new".
- **Remit owns the generic part, once, in the plugin:** the first poll answers no items and a cursor that marks now; the cursor
  holds the ids already answered, so a replay is the same event (ADR-0024); items come oldest first and at most `limit`.
- **The owner reads the script where he applies it.** The pull's `query` is the script, and a pull whose source, connection or query
  changed waits for his Apply with the query shown (`event-signals.md`): one Apply instead of three.
- **A Chair or the Assistant writes the script** and asks for the signal. No engineer card and no build are needed.
- **`replies` moves onto it** as a script (`read.mjs` as `runtime: ego-browser`), and the `replies` plugin and its Go code are deleted in
  the same change, with its tests moved to the script's behaviour.

## Consequences

- A new reader is a script and a pull, not a program: the cost of changing it tomorrow is editing text the owner re-applies.
- The reader runs with full rights of the server's user. That is the choice of ADR-0034 and of *Where we are*; the seam below is where a
  control goes.
- The script runs only on the host the server runs on. `replies` is a one-machine reader through that machine's logged-in browser,
  and stays so.

## Revisit when

- **The owner asks for hardening.** The seam is the plugin boundary: `script`'s run goes through the `Runner` of `internal/source`
  (`program.go`), whose second implementation is the jail already there; a URL list or a gateway for the browser is a policy in front of
  that run, not a new mechanism. Threat modelling (`aio-threat-models`) runs then.
- **A script needs a credential Remit holds.** Today it reads with the host's own sessions. A connection's secret reaching a script
  is the proxy of ADR-0018, not this plugin.
- **A sensor (ADR-0043) should run unjailed too.** It is a separate door, bound to a chain's watch; it can take the same host run.
- **A script prints a state, not items.** A reading of a chain's key is ADR-0043's; `script` answers items only.

Enforced by: TestTheScriptsFirstPollAnswersNoItemsAndNow, TestAReplayedItemIsTheSameEventAndANewOneIsOneItem,
TestAnErrorLineIsAnErrorItemAndTheSameErrorWithOtherNumbersIsOneEvent, TestAScriptRunsOnTheHostWithTheRuntimeItNamesAndNoEnvironmentOfTheHosts,
TestAScriptThatRunsPastThePollsDeadlineIsEnded, TestTheCodeIsTheQueryAfterItsFirstLineAndIsRunAsWritten,
TestThePluginIsBundledAsASourceThatPollsAndShipsNothing,
TestAQueryAsLongAsAScriptReachesThePluginAndOneBeyondTheCapDoesNot
