# 0028. A chain's shell and web are capabilities its playbook lists

Status: accepted, 2026-11-02; the capabilities a playbook lists superseded by ADR-0113: a chain starts with its kind's capabilities, and its Chair gives or asks for the rest. Source: `docs/remit-design.md` (K4, "phủ rộng"), ADR-0018 (one gate for the outside world), `AGENTS.md`
("a capability is an agent tool first … a named capability that playbooks list").

## Context

Every agent held every Claude Code tool unless its agent definition narrowed them (`Loadout.Tools` nil means all). Bash and the web tools
reach any system the OS user reaches, around the gate every other outside call passes (ADR-0018). A chain for a support team, whose agents
read customers through sources, had a shell and a browser it never needed, and an agent steered by a customer's words could use them.

## Options

1. Narrow each agent definition's tool list. One list per agent: a new playbook means editing agents, and a forgotten one stays open.
2. A capability per tool group, listed by the playbook, and the session denied what its chain does not list. **Chosen**: the owner's
   existing way to say what a kind of chain may do (`internal/orch/capabilities.go`), with no new setting.
3. Remove the tools for everyone. Breaks every code chain.

## Decision

- Two capabilities: `shell` (Claude Code's Bash) and `web` (WebFetch and WebSearch). A chain's cards (its Chair, hired agents, reviewers
  and judges) are started with each tool denied unless the chain's playbook listed the capability when it was queued. The deny rule is the
  session's own (`DenyTools`), enforced in every permission mode.
- A running chain's Chair gives the chain `shell`, `web` or any capability that does not write with `grant_capability`: it knows what its
  agents have to do, so it decides, and no one is asked. The chain's own writer (`capabilityCmd`) stores the capability and records a
  `capability` decision (who gave it, why) that the owner reads in the chain's history; the Chair's session closes and opens again with the
  tool through a message, and every agent opened afterwards holds it.
- A capability that writes (`writes` true in the catalog: playbooks, agents, library, extensions, sources, propose, events, outcomes,
  objectives, memory) changes what the product does, so the Chair asks for it with `request_capability`: one `change` inbox item reaches the
  owner, and only his Apply gives it, the same way. The catalog's `writes` flag is the one line between the two tools, and each refuses
  the other's capability naming the right one. The deny rule is the session's own and no approval of a tool call lifts it, so a tool
  reaches an agent only through a capability the chain holds.
- The Assistant and the Dispatchers belong to no chain: the Assistant keeps its tools, a Dispatcher holds none (ADR-0018).
- The code playbooks that build, test and search code (feature, discovery-fix, bug-sweep, wrong-number, backfill, self-development) list
  both. The playbook author and the analyst list neither.
- Migration 55 adds both to every stored playbook and chain: nothing running loses a tool. The owner narrows a playbook by asking the
  playbook author; chains started after get the narrower set.
- Neither is a capability that changes something outside (`writes` false): a proposing chain may still queue chains that hold them.

## Consequences

- A chain made for a non-software team can be given sources and no shell, so its agents cannot run a command or fetch a page of their own.
- A new playbook with no capabilities has neither: its Chair that needs a command gives it with `grant_capability`. A playbook
  is a template, not a ceiling: the Chair gives any capability of the catalog that does not write, and the owner's Apply is the rail
  for the ones that do. A chain that was proposed, or started by an event or a client, is refused a capability that writes
  (`mayNotHave`), as its playbook is.
- A capability is given as a set-add: the owner's Apply holds whatever else the chain was given meanwhile, so requests that wait
  together (shell and web) are each applied, in either order, and one the chain already holds closes as applied.
- The Chair is pointed to `grant_capability` in its brief and in the answer to `hire_agent`, which names the tools a hire
  lacks because its chain withholds them; a free-text question to the owner (`ask_owner`) gives nothing.
- A capability the Chair gave stays for the chain's life, as one the owner gave does: the owner reads the decision and stops the chain
  (pause, stop) when he disagrees.
- This is a deny rule on Claude Code's tools, not a jail: an agent that holds `shell` still runs as the server's OS user (ADR-0004).
  Isolating that is its own unit.
- Threat model (a tool surface an outsider's words can steer):

| Threat | Control |
|---|---|
| An agent steered by outside text runs a command or fetches a URL (elevation, information disclosure) | A chain without the capability has the tool denied in the session; a chain that lists it chose to |
| An agent edits its own playbook to gain the tool (elevation) | Playbook writing is the `playbooks` capability, the Chair's alone, and an edit of a playbook reaches chains started after; a built-in's capabilities are code |
| A migration or default silently removes a tool from running work (denial of service) | Migration 55 keeps both on everything stored |
| A worker steered by outside text gives itself the tool (elevation) | `grant_capability` and `request_capability` are the Chair's alone (`TestOnlyAChairHasTheCapabilityTools`); a worker's permission request reaches its Chair, and a tool the chain lacks is denied in the session, so no request for it exists |
| The Chair, steered by outside text, gives its chain a tool that reaches out (elevation) | Accepted for now, bounded: the capabilities it gives do not write, every outward command still passes the ask rules, the deny rules (push, merge) and the Chair's own answer, and the decision names the Chair and its reason (`TestAChairGivesItsChainAnyCapabilityThatDoesNotWriteItself`) |
| An agent gives itself a capability that writes (elevation) | The Chair's `request_capability` only raises the item; the capability is stored by the owner's Apply, which an agent's or the Assistant's answer cannot make (`TestTheOwnerGivesAChainACapabilityThatWritesAndNothingElseDoes`) |
| The Assistant is held back by a chain rule it is not under | It belongs to no chain and keeps its tools: `TestTheAssistantIsNotHeldToAChainsCapabilities` |

## Revisit when

The owner can take a capability back from a running chain, or give it for a time (neither is built: a given capability stays for the chain's life). Isolation of the shell (its own OS user and jail) lands: the capability then decides whether the jail is given at all. The owner wants a limit on what the Chair gives alone (a capability the Chair may not give without him, a count per chain, a notification at the moment it gives): the seam is `Capability.writes`, the one line `grant_capability` and `request_capability` split on, and the decision the Chair's grant records.

Enforced by: TestTheOwnerGivesAChainACapabilityThatWritesAndNothingElseDoes, TestAChairGivesItsChainAnyCapabilityThatDoesNotWriteItself, TestEachCapabilityHasOneAuthority, TestRefusingACapabilityLeavesTheChainAsItWas, TestARequestForWhatTheCatalogLacksIsRefused, TestAProposedChainsChairIsRefusedAWriterCapability, TestAHireTellsTheChairWhichToolsItsChainWithholds, TestTwoCapabilityRequestsThatWaitTogetherAreBothApplied, TestApplyingACapabilityTheChainAlreadyHoldsClosesTheItem, TestOnlyAChairHasTheCapabilityTools, TestAChainHoldsNoShellAndNoWebUnlessItsPlaybookListsThem, TestTheAssistantIsNotHeldToAChainsCapabilities, TestMigration55KeepsTheShellAndTheWebOfEverythingStored, TestPlaybookToolsDescribeEveryCapability
