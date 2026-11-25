# 0053. A person signs in with Google, through the server's device flow

Status: superseded by ADR-0060, 2026-11-04 (accepted 2026-11-04). Source: `docs/superpowers/specs/2026-11-04-employee-runtime-design.md` (*`/worker`*, *Rails*), ADR-0026,
ADR-0051.

## Context

A person the owner hires works from a device of their own: today a Paseo app, through the plugin in `paseo/`, on a phone, a laptop or a
pod. They hold no Remit account, and the owner key (ADR-0026) is the owner's alone: no channel hands it to anyone else. The person
needs a credential of their own that opens their work and nothing more, which the owner neither types nor passes on, and which ends when
they stop working here. The owner hires a person with the Google account they work with (`Employee.Email`), so that account is who
they are. A device such as a pod or a phone app's server side has no browser to come back to after a sign-in.

## Options

1. **G1: the server runs Google's device flow.** The device asks the server to start a sign-in; the server asks Google for a device
   code and hands the device the user code and Google's URL; the person types the code at Google on any browser; the server polls
   Google, verifies the ID token, maps the verified email to an active person, and issues the device a worker key. **Chosen.**
2. **G2: the plugin runs OAuth and sends the ID token.** Every device holds the OAuth client's secret, and the server trusts a token a
   device relays, which it must then check for replay and audience as a bearer it did not ask for.
3. **G3: owner-issued keys.** The owner makes a key per person and passes it on: a credential the owner handles, a form the owner fills,
   and nothing that ends the key when the person's Google account is suspended.

## Decision

Option G1.

- `internal/googleauth` runs the device flow (`golang.org/x/oauth2`'s `DeviceAuth` and `DeviceAccessToken`) and is the one
  verification of an ID token (`accountOf`, with `github.com/coreos/go-oidc/v3`): signed by Google's published keys, issued by Google
  (`https://accounts.google.com` or `accounts.google.com`), for this client, not expired, with a verified email. The server is given
  the OAuth client by `--google-client-id` and `--google-client-secret`; without them sign-in is off and says so. `--google-endpoint`
  points the flow at a stand-in (`googletest`'s `fakegoogle`) for the dev instance.
- **A sign-in is two calls** (`signInStart`, then `signInWait` until it is not `waiting`), each answering within the socket's timeout:
  `WaitSignIn` polls Google for at most 20 seconds (inside the 30 seconds a host in between commonly gives one call) under one lock per sign-in. A device has one sign-in open at a time: starting
  again ends its earlier codes, so a retry after a code the person gave up on never meets the host's limit on open sign-ins. The device holds the sign-in's secret (`si_` and 256
  bits); the store holds its sha256 and the device code sealed apart from the record.
- **The key.** On Google's answer the verified email is mapped to an active person hired with it; any other account is refused with
  *This Google account has no work here*. A match issues a worker key (`wk_` and 256 bits, sealed for its record as the entrances'
  keys are, `entrance_keys.go`), stored with the sign-in in one transaction, one per person and device: signing in again on a host
  replaces that host's key. The key is handed to every wait of the sign-in until the device first uses it or ten minutes pass; then
  the sign-in holds no key, and a key no device used opens nothing.
- **What the key opens.** `/worker` only (ADR-0026 keeps the owner key the owner's), matched in constant time over every key, and acts
  as `ByWorker` on the person's own cards.
- **Ending a key.** Signing out, the person's retirement or a new Google account for them (applied by the owner), and the Assistant on
  the owner's words (`revoke_worker_key`) delete it; the channel closes its sockets (`WorkerKeyDeleted`).
- **Limits.** Three open sign-ins per host (the IP a device connects from, its port dropped, `workerrpc.hostOf`) and twenty
  server-wide; a sign-in past its code's lifetime is expired by the scheduler.

## Consequences

- A person needs nothing from the owner but the hire: they sign in with the account they were hired with, on each device.
- Retiring a person or giving them another account closes every device of theirs at once.
- The server reaches Google's endpoints while a sign-in waits; with no OAuth client configured, no person signs in.
- Which Google accounts may reach the consent screen at all is the OAuth client's setting (external), not code: the server admits
  only emails the owner hired.

## Revisit when

- **A Google account is suspended** while its key lives: check the account against Google on use, or issue short keys renewed by a
  refresh token.
- **The owner wants a domain rule** (`hd`): only accounts of his Workspace sign in.
- **A key should be held by the device** and sign the person's acts (a key pair per device), so a stolen key alone acts as no one.
- **The server runs behind a reverse proxy:** every device then connects from the proxy's IP, so the per-host limit counts them
  all as one host and a key's last address is the proxy's. Read the device's address from the proxy's forwarded header, set by
  a proxy the server trusts.
- **Hardening is asked for:** a rate limit per account, an audit of sign-ins the owner reads, alerts on a sign-in from a new address.
