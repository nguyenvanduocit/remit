# 0022. A local server lets in the holder of the owner key

Status: accepted, 2026-11-01; its choice between `google` and `local` is superseded by ADR-0026: the owner key is the only way in. Source: the owner's request of 2026-11-01 (a mode that asks for no sign-in, for a user who runs
Remit on their own machine), ADR-0004, ADR-0021.

## Context

Google through Firebase was the only way in: the server would not start without a Firebase project, its web API key and the
accounts that may come in. A person who runs Remit on their own laptop has to make a Firebase project first, for a server
nobody but them can reach.

Asking for nothing at all is not an option on that machine. Every agent runs as the server's OS user (ADR-0004) and reaches
`127.0.0.1`: a socket that lets anyone in would let any agent act as the owner, answer its own questions and lift the rails
the owner holds (merge, pause, stop). Any web page the owner opens can also open a WebSocket to `127.0.0.1`, as WebSockets are
not held to the same origin.

## Options

1. **No sign-in on a loopback address.** Any agent and any web page the owner visits become the owner: refused.
2. **A password the owner types.** A form to fill in at every new browser, and a secret the owner chooses and reuses.
3. **An owner key in a file only the owner can read, handed over by a link the server prints** (the way Jupyter does).
   **Chosen**: the owner clicks once per browser, an agent cannot read the file, and a web page cannot guess the key.

## Decision

- **`-sign-in=google|local`**, one way in per server; `google` stays the default. An unknown value does not start.
- **`-sign-in=local`** (`internal/auth/local.go`, `cmd/remit/signin.go`):
  - The first start writes 32 random bytes, in hex, to `<data>/owner.key` with mode 0600, never replacing a file there. A key
    file others may read, a link, or one that holds no key is refused and left alone. Deleting it makes a new key at the
    next start.
  - The key is in `protectedFromAgents`: no agent session can read, change or replace it (ADR-0018, ADR-0021).
  - The server listens on this machine's loopback alone (`127.0.0.1`, `::1`, `localhost`); any other `-addr` does not start,
    as the key travels in plain HTTP.
  - Once it listens, the server prints `http://<addr>/#key=<key>` to its terminal. With no terminal (a supervisor's log, which
    agents of the same user may read), it logs where the key is, never the key.
  - `/auth/config` answers `{"mode":"local"}` and nothing more.
  - The browser takes the key out of the address before anything reads it, keeps it in `localStorage` as Firebase keeps its
    sign-in, and sends it as the socket's credential. A browser comes in as the owner `local` (`auth.LocalOwner`): every act
    is recorded as that owner's, through the web channel.
- **One credential for both ways in**: the socket's handshake carries `credential` (a Google ID token, or the owner key), and
  `rpc.Verifier` checks it. A key is compared in constant time.

## Consequences

- A person on their own machine runs `./bin/remit -sign-in=local` and clicks the link: no Firebase, no account.
- A local server has one owner, `local`, and no list of accounts; the Google sign-out in the top bar does not show.
- Whoever holds the key is the owner, from any browser on the machine, until the file is deleted and the server restarted.
- **Threats** (STRIDE over the boundaries a browser, an agent and a web page cross into the socket):
  - *Spoofing* by an agent: it needs the key, and the key file is hidden from every session, the server never puts it in its
    environment or arguments (which a macOS session can read, ADR-0021), and it reaches a log only as the file's path.
    Residual: a program of the same OS user that is not a session (one the owner starts) can read the file and the browser's
    storage, as it can read Firebase's stored sign-in today.
  - *Spoofing* by a web page: the page cannot know the key, and `/auth/config` tells it only the mode; a rebinding DNS name
    reaches the same socket and is refused the same way.
  - *Information disclosure*: the key leaves the address bar before the router reads it, so it is in no copied link or
    history entry; it is never sent but over loopback.
  - *Tampering*: an agent cannot replace the key file with one it knows (hidden paths cannot be written or renamed); a file
    placed before the first start is used only if it is the owner's alone, a regular file, and a key.
  - *Repudiation*: every act through the socket is recorded with who acted (`local`, through `web`), as before.
  - *Elevation of privilege*: nothing new is reachable: the client keys of outside agents (ADR-0009) and the MCP endpoints are
    unchanged, and no rail moves.

Enforced by: TestTheOwnerKeyIsMadeOnceAndKeptAcrossStarts, TestOnlyTheOwnerKeyLetsABrowserIn, TestAKeyFileThatCannotBeTrustedIsRefusedAndLeftAlone, TestTheLinkTheServerPrintsHoldsTheOwnerKey, TestTheOwnerKeyIsRefusedOffThisMachine, TestTheOwnerKeyIsPrintedToATerminalAndNeverLogged, TestTheOwnerKeyIsHiddenFromAgents

## Revisit when

- Remit is used by more than one person on a server, which a single owner key cannot tell apart.
- Agents get an OS user of their own (ADR-0004), which also keeps them from the browser's storage.
