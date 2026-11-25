
CREATE TABLE drafts (id TEXT PRIMARY KEY, chain_id TEXT NOT NULL, at INTEGER NOT NULL, ended INTEGER, doc TEXT NOT NULL);
CREATE INDEX drafts_chain ON drafts(chain_id, at);
CREATE INDEX drafts_ended ON drafts(ended) WHERE ended IS NOT NULL;
