-- Projects (ADR-0108): the products the owner's work is on, one document with a revision each, like an objective. The
-- name is kept in a column of its own so that two projects cannot share one, in any case: a name is how the owner and
-- the agents tell them apart. An objective names its project inside its own document, so no other table changes.
CREATE TABLE IF NOT EXISTS projects (id TEXT PRIMARY KEY, name TEXT NOT NULL, created_at INTEGER NOT NULL, doc TEXT NOT NULL);
CREATE UNIQUE INDEX IF NOT EXISTS projects_name ON projects(name COLLATE NOCASE);
