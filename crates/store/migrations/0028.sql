
CREATE TABLE changes (id TEXT PRIMARY KEY, kind TEXT NOT NULL, name TEXT NOT NULL, at INTEGER NOT NULL, doc TEXT NOT NULL);
CREATE INDEX changes_by_record ON changes(kind, name, at);
