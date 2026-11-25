
CREATE TABLE IF NOT EXISTS chains (id TEXT PRIMARY KEY, created_at INTEGER NOT NULL, doc TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS cards (id TEXT PRIMARY KEY, chain_id TEXT NOT NULL, created_at INTEGER NOT NULL, doc TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS cards_chain ON cards(chain_id, created_at);
CREATE TABLE IF NOT EXISTS edges (id TEXT PRIMARY KEY, chain_id TEXT NOT NULL, created_at INTEGER NOT NULL, doc TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS edges_chain ON edges(chain_id, created_at);
CREATE TABLE IF NOT EXISTS decisions (id TEXT PRIMARY KEY, chain_id TEXT NOT NULL, created_at INTEGER NOT NULL, doc TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS decisions_chain ON decisions(chain_id, created_at);
CREATE TABLE IF NOT EXISTS review_threads (id TEXT PRIMARY KEY, chain_id TEXT NOT NULL, n INTEGER NOT NULL, doc TEXT NOT NULL);
CREATE UNIQUE INDEX IF NOT EXISTS review_threads_chain ON review_threads(chain_id, n);
CREATE TABLE IF NOT EXISTS inbox (id TEXT PRIMARY KEY, chain_id TEXT NOT NULL, status TEXT NOT NULL, created_at INTEGER NOT NULL, doc TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS inbox_status ON inbox(status, created_at);
CREATE TABLE IF NOT EXISTS playbooks (id TEXT PRIMARY KEY, doc TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS spend (day TEXT NOT NULL, chain_id TEXT NOT NULL, usd REAL NOT NULL, PRIMARY KEY(day, chain_id));
CREATE TABLE IF NOT EXISTS events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  chain_id TEXT NOT NULL, card_id TEXT NOT NULL, kind TEXT NOT NULL,
  tool_call_id TEXT NOT NULL DEFAULT '', tool_kind TEXT NOT NULL DEFAULT '', status TEXT NOT NULL DEFAULT '',
  title TEXT NOT NULL DEFAULT '', text TEXT NOT NULL DEFAULT '', from_who TEXT NOT NULL DEFAULT '', data TEXT,
  created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
CREATE INDEX IF NOT EXISTS events_card ON events(card_id, id);
CREATE INDEX IF NOT EXISTS events_tool ON events(card_id, tool_call_id);
