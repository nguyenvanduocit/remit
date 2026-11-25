# 0009. A client key is a way in for one outside agent, with limits and no rail

Status: accepted, 2026-10-29; the key's allow-list of playbooks superseded by ADR-0113: a key starts `work` chains while its `chainsPerDay` is above zero. Source: the owner's request of 2026-10-29 (an evaluator that acts on the live system, "test ngay
trên hệ thống thật"), `docs/superpowers/specs/2026-10-29-client-keys-design.md`, and ADR-0004.

## Context

The owner runs an outside agent, the doctor, that evaluates Remit. Reading the database tells it what happened; it cannot
start a task and see what the system does with it, nor stop a run that has gone wrong. Google sign-in is the only way in
(`README.md`, *Signing in*), and an agent must not sign in as the owner. The owner decided, after a read-only key and a separate
lab were put to him, that the evaluator acts on the live system.

## Options

1. A read-only key. Rejected by the owner: it cannot run an experiment.
2. A lab, a second Remit with made-up data. Rejected by the owner: what is measured must be the system he uses.
3. The owner's own token for the agent. Rejected: no limit, no record of who acted, and a rail (sign-in) given away.
4. A key of its own kind: an actor `client` that never is the owner, with limits the owner sets and tools that cannot lift a rail.

## Decision

Option 4.

- **A key is a way in like a webhook's.** `ck_` plus 256 random bits, made by Remit, sealed under `-secret-key-file`, shown to
  the owner on the Client keys page (a logged read) and never to an agent, rotated or deleted in one step. The owner makes, limits,
  pauses, rotates and deletes keys by telling the Assistant; the page only shows them, and pauses or resumes one (a rail).
- **The endpoint is `/mcp/client`, on this machine only.** The handler refuses a request that does not come from a loopback
  address or carries a forwarding header, before it reads the key, and answers every refusal the same.
- **The actor is a new kind, `client`**, never `ForOwner()`: `act` refuses it, and every rule that reads the actor's kind falls
  closed for it. A client's tools call methods of their own that go through `clientAct`, which names the key.
- **The tools are an allow-list** (`list_chains`, `read_chain`, `chain_events`, `card_events`, `read_mr_changes`,
  `read_outcomes`, `read_board`; `start_chain`, `pause_chain`, `finish_chain`, `stop_card`, `cancel_turn`, `message`). Resume,
  answering the inbox, merging, archiving, deleting and every cap, credential, key, webhook and definition are absent.
- **A key has limits the orchestrator enforces:** chains a day (3), acts an hour
  (60), and the playbooks it may start (none until the owner names them; none may hire an engineer, nor may the Chair of a chain
  it starts).
- **A chain a key starts has origin `client`.** It takes no playbook that writes agents, skills, MCP servers or playbooks, never
  publishes through Remit (no push, no MR), is left out of the chain and per-playbook counts as a Scout's is, and its goal is
  headed as an outside agent's, not the owner's. `chainsPerDay` bounds how many experiments a key runs.
- **A message from a key reaches an agent as outside text**, fenced and attributed to the key.
- **The holder keeps its key in a file of mode 0600** that the process which connects reads: never in a prompt, a message or an
  argument. `-client-key-files` names those files, so agents are asked before reading them (ADR-0004's tripwire).

## Consequences

- The doctor starts known-answer experiments on the live system, within a few dollars, and steers what goes wrong. Each act is a
  decision on the chain it touched, by `client · <key>`.
- **Residual risk (the largest):** agents run as the server's OS user (ADR-0004). One steered into reading the key file, or the
  database with `secret.key`, holds the key, and with it `chainsPerDay` chains and pause and stop across chains. The ask rules are
  a tripwire; only isolation closes it.
- An injected holder (it reads text outsiders wrote) can pause and stop chains up to `actionsPerHour` and start `chainsPerDay`
  chains. It cannot resume, act beyond its limits, or change a definition.
- A future rule that reads an actor's kind carelessly could open a rail to `client`; the enumeration tests below fail when an
  action takes a `Request` without a gate, or the client door calls a method that takes its actor from the caller.
- Pausing or deleting a key stops its next act, not the chains it started: the key's page lists them and the owner pauses them.

Enforced by: TestAClientHolderIsGivenExactlyTheAllowedTools, TestEveryActionOfTheOrchestratorChecksWhoAsks, TestTheClientDoorCallsNoMethodThatTakesAnActor, TestClientActorIsNotTheOwner, TestAClientKeysValueAppearsOnlyInTheOwnersReveal, TestOnlyTheOwnerIsShownAClientKey, TestARequestFromAnotherMachineIsRefusedBeforeItsKeyIsChecked, TestEveryRefusalIsTheSameAndEchoesNothing, TestAClientsChainDoesNotPublishAtAnyStepOfASubmit, TestAHolderCannotMakeMoreActsAnHourThanItsKeyAllows, TestAHolderCannotSteerWhatIsInNoChain

## Revisit when

- A holder appears on another machine (TLS and forwarding are decided deliberately then).
- A key gains a tool, or a rule is added that reads an actor's kind.
- The first incident from an injected holder.
- Agents are isolated (ADR-0004 is superseded): the residual risk above then shrinks.
