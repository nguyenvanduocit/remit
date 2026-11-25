
CREATE TABLE source_calls (id INTEGER PRIMARY KEY AUTOINCREMENT, chain_id TEXT NOT NULL, at INTEGER NOT NULL, doc TEXT NOT NULL);
CREATE INDEX source_calls_chain ON source_calls(chain_id, id);
