
CREATE TABLE session_inits (card_id TEXT PRIMARY KEY, chain_id TEXT NOT NULL, doc TEXT NOT NULL);
CREATE INDEX session_inits_chain ON session_inits(chain_id);
