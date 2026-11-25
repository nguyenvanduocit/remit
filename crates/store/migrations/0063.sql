
CREATE TABLE IF NOT EXISTS accounts (id TEXT PRIMARY KEY, doc TEXT NOT NULL);
INSERT OR IGNORE INTO accounts(id, doc) VALUES('main', json_object('id', 'main', 'createdAt', '1970-01-01T00:00:00Z'));
