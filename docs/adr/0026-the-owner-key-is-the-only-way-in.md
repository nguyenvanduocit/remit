# 0026. The owner key is the only way in

Status: superseded by ADR-0105 (2026-11-17: the owner may also sign in with Google; the owner key stays). Accepted 2026-11-02. Supersedes ADR-0022's `-sign-in=google|local` choice: `local` is the only way in and Google through Firebase is
gone. Source: the owner's decision of 2026-11-02 (no login, no users, no roles: whoever reaches the server may use it, and securing the
network is the operator's job), `docs/remit-design.md` (K3), ADR-0004, ADR-0021, ADR-0022.

## Context

Two ways in existed, one per server: Google through Firebase (a project, a web API key, a list of accounts) and the owner key of ADR-0022.
Google was the default, so a fresh server did not start without a Firebase project. The owner decided that Remit has one owner per
server and no member list, and that what lies between the network and the server is the operator's concern. A way in that keeps a list
of accounts and an identity provider contradicts that decision, depends on a third party for every start, and ships a second credential
path through the rail that guards authentication.

Asking for nothing at all stays refused (ADR-0022): every agent runs as the server's OS user and reaches `127.0.0.1`, so a socket that
lets anyone in lets an agent act as the owner and lift the rails the owner holds.

## Options

1. Keep both ways in, with `local` as the default. The Firebase code, flags, web SDK and tests stay for no user the owner has.
2. Remove Google and keep the owner key as the one way in. **Chosen.**
3. Remove all sign-in. Refused for the reason above.

## Decision

- The owner key of ADR-0022 (`internal/auth/local.go`, `cmd/remit/signin.go`) is the only credential of the owner; a person's Paseo holds a worker key (ADR-0053), which opens `/worker` only. `-sign-in`, `-firebase-*` and
  `-google-emails` do not exist; `/auth/config` is not served; the browser holds the key and sends it as the socket's credential.
- Every property of the key stays as ADR-0022 set it: created once with mode 0600, never replaced, hidden from agent sessions, printed
  only to a terminal, the server on loopback alone. To reach the server from elsewhere the operator puts a proxy they trust in front of it.
- The rpc gate answers "bad token" or lets in. "not allowed" and "auth unavailable" belonged to Google (an account off the list, keys out
  of reach) and are gone with it.
- There is one owner, `local` (`auth.LocalOwner`). Every act is still recorded with who acted and through which channel.

## Consequences

- A server starts with no account of any provider, and a deployment has one secret less to move.
- Two people cannot be told apart. A second person with the key is the owner too; this is the owner's decision, not a gap to close here.
- Stored sign-ins of Firebase in a browser are ignored; the browser asks for the owner link once.
- Threats (STRIDE, the boundaries a browser, an agent and a web page cross into the socket): the table of ADR-0022 holds unchanged for
  spoofing by an agent or a web page, disclosure through the address bar, tampering with the key file, repudiation and elevation. What
  this decision removes: spoofing through a forged or stolen ID token, and denial of service by Google's keys being out of reach.

## Revisit when

A server is used by more than one person who must be told apart, or a deployment cannot keep the key file private to one OS user.

Enforced by: TestTheLinkTheServerPrintsHoldsTheOwnerKey, TestTheOwnerKeyIsRefusedOffThisMachine, TestTheOwnerKeyIsPrintedToATerminalAndNeverLogged, TestTheOwnerKeyIsHiddenFromAgents, TestOnlyTheOwnerKeyLetsABrowserIn, TestTheHolderOfTheOwnerKeyIsLetIn, TestWithoutAGateNobodyComesIn
