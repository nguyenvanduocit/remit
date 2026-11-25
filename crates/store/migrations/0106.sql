-- ADR-0102: a fault planted on purpose in a trial chain's work, one per chain at most; Remit keeps it so the judges'
-- verdicts on the chain say whether they caught it.
CREATE TABLE IF NOT EXISTS seeded_faults (id TEXT PRIMARY KEY, chain_id TEXT NOT NULL UNIQUE, planted_at INTEGER NOT NULL, doc TEXT NOT NULL);
