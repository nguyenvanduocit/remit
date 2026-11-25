# 0105. The owner may also sign in with Google

Status: accepted, 2026-11-17. Supersedes ADR-0026 (the owner key as the only way in); ADR-0022's owner key and every rule
it sets for it hold unchanged. On 2026-11-17 the owner requested Google sign-in through Firebase. Related decisions include
ADR-0022, ADR-0026, ADR-0060 and ADR-0089.

## Context

Remit runs as a daemon on a machine the owner reaches from elsewhere, through a TLS proxy of his private network in front of
the loopback address (README, *Signing in*). Every new browser needs the owner key carried to it by hand: the link the
server printed, or the key file's content typed after `#key=`. ADR-0026 removed Google because a fresh server should start
without a Firebase project and the owner was one person at one machine; neither has changed, but carrying the key around
has become the pain.

What must not move: the owner key stays the way in that needs nothing outside the machine; a mentor still comes in with a
code (ADR-0060, ADR-0089); there is still one owner; every act still records who acted and through which channel; and no
agent can come in as the owner (ADR-0022: agents run as the server's OS user and reach its loopback).

A Firebase ID token proves a Google sign-in for one hour, and the Firebase SDK renews it in the page. The web app's
session runs in a SharedWorker's core (ADR-0095) that dials again by itself after a drop, a sleep or a server restart,
each time with the credential the newest tab handed it; a refused credential is final (`bad_credential`) and shows the way
in.

## Options

1. **The ID token is the session's credential**, checked at each hello. No state on the server, and Firebase's own renewal.
   But a dial after an hour away (a laptop asleep overnight) carries an expired token and is refused for good, unless the
   core learns to ask a tab for a fresh token before each dial and the page keeps Firebase signed in beside Remit: a
   change to the core, the tab protocol and the refusal's meaning, for every reconnect.
2. **The server checks the ID token once and answers a credential of its own**, which the browser keeps where it keeps the
   owner key and every dial carries. The core, the link and the tab protocol do not change; the session lasts as long as
   the server says. **Chosen.**
3. Option 2 with each minted session stored in a table, so one browser's can be revoked: a migration and a list to manage
   for a revocation the owner has not asked for. Its seam is kept (below).
4. Hand the owner key itself to a browser that signed in with Google. One key for every browser, never expiring, leaving
   the machine on every sign-in, and every act recorded as `local`: refused.

## Decision

- **Flags.** `-firebase-project`, `-firebase-api-key` and `-google-emails` (comma-separated) turn Google sign-in on;
  `-firebase-auth-domain` defaults to `<project>.firebaseapp.com`. All empty: Google sign-in is off and nothing differs from
  a server with the owner key alone. Some set and others not: the server does not start.
- **The way in asks.** `GET /auth/config` answers `wire::app::SignInConfig`: the project, API key and auth domain when
  Google is on, nothing when it is off. The sign-in page (`Login.vue`, `web/src/google.ts`) then sets Firebase up with
  in-memory persistence, so Firebase keeps nothing of the sign-in, and offers **Sign in with Google** (`signInWithPopup`,
  account chooser) above the mentor's code and the owner key's instructions.
- **The server checks the token** (`POST /auth/google`, body `wire::app::GoogleSignIn`, at most 16 KiB; the token at most
  8 KiB). `auth::google::verify_id_token` is sans-IO (token, keys, project and list, now) and checks what Firebase lists for
  a server without its Admin SDK (https://firebase.google.com/docs/auth/admin/verify-id-tokens): header `alg` RS256 and a
  `kid` among Google's securetoken keys, the signature, `aud` the project id, `iss`
  `https://securetoken.google.com/<project>`, `exp` after now, `iat` and `auth_time` not after now plus 60 s of clock
  skew, a non-empty `sub`; then `firebase.sign_in_provider` `google.com`, `email_verified`, and the address on
  `-google-emails` (compared without case). The keys come from
  `https://www.googleapis.com/robot/v1/metadata/x509/securetoken@system.gserviceaccount.com` and are kept for the response's
  `Cache-Control: max-age` (five minutes when it gives none), fetched once at a time, by `crates/remit/src/google_signin.rs`.
  Refused: 403 with the reason; Google out of reach: 503; Google sign-in off: 404.
- **The server answers a credential of its own** (`auth::OwnerGate::sign_in`, `wire::app::GoogleSignedIn`):
  `gs1.<hex of "<expires>:<email>">.<hex of HMAC-SHA256>`, valid 14 days (`auth::SESSION_FOR`, the longest a Firebase session
  cookie lasts). The MAC key is HMAC-SHA256 of a fixed purpose under the owner key, so no new secret is stored, and deleting
  the owner key ends every Google sign-in with the key's own links. The browser keeps it under `remit.key`, as it keeps the
  owner key, and every session's hello carries it.
- **The gate** (`auth::OwnerGate`, `rpc::Verifier`) lets in the owner key as `local`, else a minted credential whose MAC
  holds, whose time has not run out and whose address is still on `-google-emails`, as that address; anything else is
  `bad token`. Taking an address off the list and restarting ends its sign-ins.
- **One owner.** A Google sign-in is the owner, `rpc::Principal::Owner(<address>)`: his acts are recorded as
  `by_owner(<address>, "web")`, the key holder's as `by_owner("local", "web")`; rules read the actor's kind, never the name.
- The server still listens on loopback alone (ADR-0022): the ID token and the credential cross the owner's proxy as the
  owner key does.

## Consequences

- From a new device the owner opens the proxy's address and clicks once; no key travels by hand. A server with no Firebase
  project starts and runs as before.
- The owner must keep the Firebase project: Google enabled as its sign-in provider, and every host he opens Remit at among
  its authorized domains (`localhost` is by default; a Tailscale address is not).
- Every 14 days, after a restart that changed the list, or after the owner key is replaced, the page shows the way in and
  he clicks again.
- Two people on the list cannot be told apart in what they may do: both are the owner, told apart only by the address
  recorded on their acts.
- Threats (STRIDE over the boundaries the page, the proxy, Google and an agent cross into `/auth/google` and `/link`):
  - *Spoofing* with a forged token: the signature is checked with Google's keys alone, the algorithm is fixed to RS256, and
    the audience, issuer, provider and verified address are checked. With a token of another project, or of an account
    off the list: refused. An agent still cannot come in: it holds neither the key file nor a Google sign-in of the owner.
  - *Spoofing* with a forged credential: it needs the owner key to make its MAC; a changed one fails in constant time.
  - *Tampering*: the keys are fetched over TLS from Google and only RS256 keys parse.
  - *Repudiation*: every act is recorded with the address or `local`, and each Google sign-in is logged with its address.
  - *Information disclosure*: the credential lives in the browser's storage as the owner key does; it names the address in
    clear (hex), which is the owner's own. `/auth/config` tells any page the project and web API key, which Firebase
    shows every page that signs in anyway.
  - *Denial of service*: Google's keys out of reach stop Google sign-in (503) and nothing else; a sign-in already made goes
    on, as the gate does not call Google.
  - *Elevation of privilege*: a Google sign-in reaches exactly what the owner key does; mentors' codes and client keys are
    unchanged.

## Revisit when

- One browser's sign-in must be ended without ending the others: store each minted credential (option 3), with a revoke
  call on `OwnerGate`'s `verify` path and a list on the Assistant's side.
- `/auth/google` is reached by more than the owner's private network: a rate limit per address on it, which is not built,
  and Firebase App Check.
- Google rotates its keys faster than its `max-age` says: fetch again once on an unknown `kid`, which is not built.
- Two people on the list must be told apart in what they may do: roles, which ADR-0026's single owner refused.

Enforced by: `a_google_sign_in_of_an_account_on_the_list_lets_the_owner_in`,
`a_token_that_does_not_hold_is_refused_for_what_is_wrong_with_it`, `the_keys_google_publishes_are_read`
(`crates/auth/src/google.rs`), `a_google_sign_in_lets_its_browser_in_as_that_account_until_it_ends`
(`crates/auth/src/gate.rs`), and ADR-0022's tests of the owner key.
