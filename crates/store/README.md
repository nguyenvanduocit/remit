# store

Remit's records in one SQLite file (ADR-0072, section 6): migrations in `migrations/NNNN.sql`, listed in
`migrations.rs` and checked against each database's `schema_migrations` checksums; WAL, busy_timeout 5000, foreign keys
on, synchronous FULL; an exclusive `flock` on `<db>.lock`.

One module per kind of record. Writes are `Tx` methods run through `Store::update` on the one writer thread, in `BEGIN IMMEDIATE`; reads are
`Reader` methods run through `Store::read` on a `query_only` connection, in one read transaction. `storetest` (feature
`testing`) opens a copy of a database migrated once per test binary.

A new migration is a new file appended to `migrations/` and to `MIGRATIONS`; a released one is never edited.

A row this build cannot read (a value a newer build named) is left out of every list, refused by id
(`Error::Unreadable`), logged once and counted (`Store::unreadable`, shown as `Health.unreadable`); it is never written,
so a build that reads it finds it as it was (ADR-0107).
