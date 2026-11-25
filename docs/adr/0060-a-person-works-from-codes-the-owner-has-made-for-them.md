# 0060. A person works from codes the owner has made for them

Status: accepted, 2026-11-04; the codes are a mentor's (0062); signed in on the web app by ADR-0089. Supersedes ADR-0053. Source: the owner's request of 2026-11-04 ("no Google sign-in; the person just enters
a code, and one person may hold many codes"), ADR-0026, ADR-0009, ADR-0056.

## Context

A person the owner hires works from devices of their own (the Work plugin in Paseo, ADR-0056) and needs a credential that opens their
work and nothing more. ADR-0053 had the server run Google's device flow and map the verified email to a hired person. In use it asks
for an OAuth client the owner must register and pass as flags, a Google account per person, an email typed into every hire, a
two-call wait on Google and a table of sign-ins with limits per host. The owner wants none of it: the person pastes a code, and a
person may hold several, one per device.

## Options

1. **K1: the code is the worker key.** The owner tells the Assistant to make a code for a person (`create_worker_key`); the server makes
   a worker key (`wk_` and 256 bits, sealed, as client keys are, ADR-0009); only the owner is shown its value, on the person's row; he
   passes it on; the person pastes it into the plugin, which presents it as its key. A person holds as many as they want. **Chosen.**
2. **K2: a short one-time code traded for a key.** A code of a few characters, dead after its first use or a day, which a device trades
   for a long key. A code leaked through chat opens nothing once used, at the cost of a second record (the code beside the key), an
   exchange call and an expiry sweep.
3. **K3: keep Google and add codes.** Two ways in for one person, two conventions to maintain, and the OAuth client the owner wants gone.

## Decision

Option K1.

- **Making a code.** `create_worker_key` (the Assistant, on the owner's words) makes a code for an active person, with the owner's
  note of what it is for ("Lan's phone"); `list_worker_keys` lists a person's codes and `revoke_worker_key` stops one or all of them.
  No tool result carries a value.
- **Reading it.** The owner reads a value through the `workerKey` read (`Orchestrator.WorkerKey`), refused to anyone but the owner and
  logged, the way `clientKey` shows a client key: the value enters no agent's context, record, push or transcript.
- **What it opens.** `/worker` only, for the person it was made for, matched in constant time over every key (`AuthorizeWorkerKey`). A
  socket with no key, or one that opens nothing, is turned away with `bad key`. The worker contract is 2: `signInStart`,
  `signInWait` and `signOut` are gone, and `me` says the code's note.
- **Ending it.** The Assistant on the owner's words (`revoke_worker_key`) and the person's retirement delete it; the channel closes its
  sockets (`WorkerKeyDeleted`). Signing out on a device only forgets the code there: the code is the owner's to stop.
- A person is hired by name; they hold no email. Migration 83 drops the sign-ins, rebuilds `worker_keys` without a device column,
  removes the email from people and open hires, and cancels an open change of a person's account.

## Consequences

- The server holds no OAuth client and reaches no Google endpoint; `-google-*` flags and `internal/googleauth` are gone.
- The owner passes each code on himself, over whatever channel he and the person use: a code in a chat opens the person's work until
  it is revoked.
- A person works from as many devices as they hold codes, and each is stopped on its own.

## Revisit when

- **A code leaks through the channel it was passed over:** make the pasted code one-time and short-lived and trade it for a device key
  (option K2); `createWorkerKey` and `AuthorizeWorkerKey` are the seams.
- **A key should be held by the device** and sign the person's acts (a key pair per device), so a stolen key alone acts as no one.
- **The server runs behind a reverse proxy:** a key's last address is the proxy's; read the forwarded header of a trusted proxy.
- **Hardening is asked for:** an expiry on codes, a rate limit on failed keys per address, an audit the owner reads of when each code
  was shown and used.
