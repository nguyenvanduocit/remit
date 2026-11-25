
CREATE TABLE chain_grants (chain_id TEXT NOT NULL, source TEXT NOT NULL, connection TEXT NOT NULL, effect TEXT NOT NULL CHECK (effect IN ('read', 'write')), granted_by TEXT NOT NULL, granted_at INTEGER NOT NULL, PRIMARY KEY(chain_id, source, connection));
ALTER TABLE source_connections ADD COLUMN protected INTEGER NOT NULL DEFAULT 0 CHECK (protected IN (0, 1));
