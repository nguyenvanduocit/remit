# 0029. An extension is a program a chain writes and the owner applies

Status: accepted, 2026-11-02; its program form is replaced by ADR-0034 (the API behind the proxy stays); its waits on the owner, but a secret act, are an agent's (ADR-0087). Narrows ADR-0017 ("agents hold only the tools Remit writes"): a source may also be a program a chain wrote,
once the owner applied it. Source: the owner's principle of 2026-11-02 (users and agents write small out-of-process plugins so the engine is not
forked), `docs/remit-design.md` (K5), ADR-0018 (one gate), ADR-0021 (the jail).

## Context

Every source shipped inside the binary: a payment system, a CRM or an internal API of a company that is not the first one needed a change to
Remit. The program form of ADR-0018 already runs a command-line program in a jail with a credential in its environment and only the
addresses the source lists, but the program, its commands and its addresses were Go code.

## Options

1. Keep all sources compiled in. Every customer's system waits for a release.
2. Load plugins into the server process. A plugin then holds the server's memory: its keys, its database.
3. An extension is a manifest and a program that runs only in the existing jail, through the existing gate; a chain writes it, the owner applies
   it. **Chosen.**

## Decision

- A manifest (`source.Extension`) names an id, a label, one of the capabilities a playbook lists, a program, its pinned hash, the leading words
  of the commands that may run, flags that may not be passed, the `host:port` addresses it may reach (no wildcard) and the variables of its
  credential. `Validate` is pure and refuses an id that a shipped source has, a program outside `<id>/`, a missing pin, no commands, a wildcard
  or portless address, and credential variables that move the loader or the path (`PATH`, `HOME`, `LD_*`, `DYLD_*`, interpreter options).
- A chain whose playbook lists the `extensions` capability gets `propose_extension` (the Chair's): the manifest and the whole text of a script
  (at most 256 KiB). The platform names the file by the hash of the text (`<id>/<hash12>-<file>`) and raises a change item. The owner reads the
  program's hash and its head, the commands, the addresses and the variables, and applies it or not. Only the owner's answer applies it
  (`answerChange`); the Assistant and agents cannot.
- Apply writes the program under the extensions directory (`<data>/ext`) from the exact text the owner was shown, stores the manifest, records
  who asked and who allowed it (a Change of kind `extension`), and the source exists for sessions started after. A manifest applied again
  replaces the one before; a proposal made against a record that moved is not applied.
- The extensions directory is read-only to every session (`protectedFromAgents`): a session that could edit a program would change what runs
  with a credential without an answer. Before every run the file is checked: a regular file, not a link, whose hash is the pinned one; another
  file does not run.
- The program runs as every program does (`run`, form env): in the jail, with the addresses of the manifest and no others, its own directory
  read-only and nothing else of the host's files (the data directory, the key, the database are not mounted), a scratch directory, the credential
  in its environment, its output cleaned of the credential. The agent writes only the arguments, which `Check` holds to the manifest's commands.
- A program that answers `poll` (`"poll": true` in the manifest, a field the owner reads) is also a source of events: an event signal's pull
  (ADR-0024) runs it on the signal's clock as `<program> poll`, with `MC_CURSOR`, `MC_QUERY` (the owner's query) and `MC_LIMIT` in its
  environment, never in its arguments, and it prints `{"items":[{"id","version","body":{}}],"cursor":"..."}`, oldest first. The same jail,
  addresses, pin and credential rules hold. With no cursor the items are dropped and only the cursor kept, so a new signal never replays history;
  an answer that is not that shape, an item with no id or a body that is not an object, more items than the limit, a failing exit or a timeout
  is an error and no event.
- Taking one away is the same road: `remove_extension` raises a change item that names the extension and says why; the owner's Apply deletes the
  manifest, the program and the source, and records who asked and who allowed it. A removal against an extension that moved (applied again
  meanwhile) is not applied. The connections the owner made for the source and the grants of chains stay in the store, naming a source that is gone.
- An extension may instead be an API behind the injecting proxy (ADR-0018, form T1), with no program: `auth` names an https address (a public
  host name, no address of a machine, no login or query), one scheme of a closed set (`bearer`: one variable; `basic`: a user and a password;
  `header`: one variable and the header's name, never `Host` or a framing header; `hmac-query`: one secret, the sorted query parameters signed with HMAC-SHA256 or SHA512 and the hash added as the parameter the manifest names, for payment gateways that ask it; `sigv4`: AWS Signature Version 4 from a key id, a secret key, a region and a service, signed by the proxy over the request the upstream receives), the variables it reads (each declared), and the paths where
  POST only reads. The platform adds the credential itself, from the connection's variables, and removes every spelling of it from what a script
  is answered; the agent writes scripts, which reach that one host with GET. The owner reads the address, the scheme and the variables, and
  there is no file to pin. A manifest is one or the other, never both.
- The call is accounted for as every call is (`source_calls`), with its argv.

## Consequences

- Any system can be reached by a program in any language that a chain writes and the owner reads, without a release and without touching the
  engine. No form, no setting: the owner says "connect X" and applies what comes back.
- What the manifest cannot fix: what the program does with the credential it is given. The credential must be read-only, as for every program;
  the owner's reading of the program is the control, and he is shown its hash and head, not asked to trust the author.
- A program is a script in this slice (text in the proposal). A compiled build and a conformance harness are not here.
- Threat model (an entrance that runs code with a credential; STRIDE, OWASP LLM Top 10 where an agent writes code a person then runs):

| Threat | Control |
|---|---|
| A chain writes a program that steals the credential (information disclosure) | The owner reads the program before it exists; it has no network but the listed addresses and no file of the host but its own directory; its output is cleaned of the credential; the credential should be read-only |
| An agent changes the program after the owner applied it (tampering) | The directory is read-only to sessions; the hash is checked before every run; the file is a regular file under `<id>/` |
| A manifest widens what the owner saw (elevation) | Apply recomputes the rail fields from the stored record and refuses a change that moved; the program written is the text the item holds, not a file an agent can edit meanwhile |
| An agent applies its own proposal (elevation) | Only `ActorOwner` answers a change item; `applyExtension` checks it again |
| A manifest names a path or a variable that escapes (tampering) | `Validate`: program under `<id>/` with no `..`, no absolute path; reserved variables refused; the id of a shipped source refused |
| Two extensions or a restart leave an old program running (repudiation) | The file name holds its hash, so a new program never overwrites the old; `UseExtensions` drops what was built from the old manifest; a restart reads the stored manifests |
| An agent's arguments move the program (injection) | `ProgramSpec.Check`: commands, flags and values allowed by the manifest only, none read by a shell |

## Revisit when

A compiled program must be proposed, or extensions need a conformance harness before the
owner reads them.

Enforced by: TestAnHMACQueryExtensionSignsTheSortedParametersAndAddsTheHash, TestAnHMACQueryManifestNeedsAParamAHashAndOneSecret, TestASigV4ExtensionSignsTheRequestTheUpstreamReceivesAndTheScriptHoldsNoKey, TestASigV4ManifestNeedsItsFourVariablesAndAFullRegionAndService, TestARequestIsSignedByTheCallersSignerAfterTheProgramsOwnCredentialIsDropped, TestARequestTheSignerRefusesIsNeverSent, TestAProxiedExtensionAddsItsCredentialByAClosedSchemeAndTheScriptNeverHoldsIt, TestAProxiedExtensionThatCouldReachOrSignWrongIsNotOne, TestAChairProposesToRemoveAnExtensionAndItWorksUntilTheOwnerApplies, TestTheOwnersApplyRemovesTheExtensionItsProgramAndTheSource, TestAnExtensionThatIsNotThereCannotBeProposedForRemoval, TestARemovalIsReplacedByANewerProposalForTheSameExtension, TestAnExtensionThatIsRemovedIsGoneAndRemovingAgainIsNothing, TestAFirstPollOfAnExtensionKeepsTheCursorAndNoItems, TestAManifestThatCouldMoveAnythingIsNotAnExtension, TestApplyingAnExtensionAgainReplacesWhatWasBuilt, TestAChainWithoutTheExtensionsCapabilityHasNoToolToProposeOne, TestAServerThatStartsAgainHasTheExtensionsTheOwnerApplied, TestAProgramReadsItsOwnDirectoryAndNothingElseOfTheHost, TestAnExtensionIsKeptByIDAndAppliedAgainReplacesIt
