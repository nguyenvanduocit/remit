-- ADR-0079: what the owner viewed, one mark per subject (a home, or 'assistant.feed'); at is Unix milliseconds. The
-- Assistant feed's mark moves here from the Assistant's document.
CREATE TABLE IF NOT EXISTS seen (subject TEXT PRIMARY KEY, event_id INTEGER NOT NULL, at INTEGER NOT NULL);
INSERT INTO seen(subject, event_id, at)
  SELECT 'assistant.feed', 0, CAST(round(unixepoch(json_extract(doc, '$.decidedSeenAt'), 'subsec') * 1000) AS INTEGER)
  FROM assistant
  WHERE unixepoch(json_extract(doc, '$.decidedSeenAt'), 'subsec') > 0
  ON CONFLICT(subject) DO NOTHING;
UPDATE assistant SET doc = json_remove(doc, '$.decidedSeenAt') WHERE json_type(doc, '$.decidedSeenAt') IS NOT NULL;
-- ADR-0082: what the coach wrote after a chain finished, one per finish.
CREATE TABLE IF NOT EXISTS retrospectives (id TEXT PRIMARY KEY, chain_id TEXT NOT NULL, created_at INTEGER NOT NULL, doc TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS retrospectives_chain ON retrospectives(chain_id, created_at);
