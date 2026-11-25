
CREATE TABLE deliveries (id TEXT PRIMARY KEY, key TEXT NOT NULL UNIQUE, status TEXT NOT NULL, received_at INTEGER NOT NULL, doc TEXT NOT NULL);
CREATE INDEX deliveries_status ON deliveries(status, received_at);
