# auth

Decides who is let in: whoever holds the owner key, a file only the owner can read (ADR-0022), or the owner signed in with
Google through the server's Firebase project, when it has one (ADR-0105). A rail, so plain code with no judgement in it.

- `LocalKey::open` reads the key or makes it at the first start; `LocalKey::verify` compares a credential with it in
  constant time (`subtle`).
- `google::verify_id_token` checks a Firebase ID token against Google's securetoken keys (`google::SigningKeys`, parsed
  from what `google::SIGNING_KEYS_URL` answers), the project and the accounts on the list, at a time it is given.
- `OwnerGate` is the one gate: `verify` lets in the owner key as `local`, or a credential `sign_in` minted for a Google
  address (`gs1.`, an HMAC under a key drawn from the owner key, valid `SESSION_FOR`) as that address.

It reads and writes one file, so it is not a pure crate; it depends on no runtime, driver or client: fetching Google's keys
and the clock are the server's (`crates/remit/src/google_signin.rs`).
