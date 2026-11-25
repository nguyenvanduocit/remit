-- Rounds of competing attempts at a key result whose check is a command (ADR-0115): one row each, the key result it is of
-- columns, and its state a column generated from its document, which the rounds still to judge are read by. Safe to run
-- again.
CREATE TABLE IF NOT EXISTS rounds (id TEXT PRIMARY KEY, objective TEXT NOT NULL, key_result TEXT NOT NULL, number INTEGER NOT NULL, created_at INTEGER NOT NULL, doc TEXT NOT NULL, state TEXT GENERATED ALWAYS AS (json_extract(doc, '$.state.kind')) VIRTUAL);
CREATE INDEX IF NOT EXISTS rounds_key_result ON rounds(objective, key_result, number);
CREATE INDEX IF NOT EXISTS rounds_state ON rounds(state);
