-- Accounts are listed by when they were made. A document's createdAt is RFC 3339 text in the offset it was written with,
-- which sorts as text out of time order across offsets, so the time is a column of its own, in Unix milliseconds as
-- every time column is, filled from the document.
ALTER TABLE accounts ADD COLUMN created_at INTEGER NOT NULL DEFAULT 0;
UPDATE accounts SET created_at = COALESCE(CAST(ROUND((julianday(json_extract(doc, '$.createdAt')) - 2440587.5) * 86400000) AS INTEGER), 0);
