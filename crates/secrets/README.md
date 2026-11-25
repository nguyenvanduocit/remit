# secrets

Seals the credentials Remit stores, so the database and its backups hold none in the clear: AES-256-GCM under a key kept in
a file beside the database, the slot name (`server/slot`) as additional data, and the id of the sealing key (`key_id`) on
every sealed value. `load_or_create_key`, `load_key` and `back_up_key` own the key file and its backups.

The sealed format is fixed, byte for byte, because sealed values already sit in the owner's database: `box` is the
12-byte random nonce, then the ciphertext, then the 16-byte tag; `key_id` is the first 8 bytes of
HMAC-SHA256(key, `remit secret key id`) in hex. `tests/stored_sealed.rs` opens a stored sealed value
(`tests/fixtures/stored_sealed.tsv`, made with a throwaway key).

It reads and writes the key file; it depends on no runtime, driver or client.
