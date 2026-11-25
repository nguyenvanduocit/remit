
CREATE TABLE IF NOT EXISTS memory_entries (id TEXT PRIMARY KEY, employee TEXT NOT NULL, created_at INTEGER NOT NULL, doc TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS memory_entries_employee ON memory_entries(employee, created_at);
CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(entry_id UNINDEXED, text, kind);
