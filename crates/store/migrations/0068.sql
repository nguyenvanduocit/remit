
ALTER TABLE readings ADD COLUMN seq INTEGER;
UPDATE readings SET seq = (SELECT n FROM (SELECT rowid AS rid, ROW_NUMBER() OVER (ORDER BY at, key, rev) AS n FROM readings) AS o WHERE o.rid = readings.rowid);
UPDATE readings SET doc = json_set(doc, '$.seq', seq);
CREATE UNIQUE INDEX readings_seq ON readings(seq);
CREATE TABLE log_seq (id INTEGER PRIMARY KEY CHECK (id = 1), last INTEGER NOT NULL);
INSERT INTO log_seq(id, last) SELECT 1, COALESCE(MAX(seq), 0) FROM readings;
ALTER TABLE subscriptions RENAME COLUMN key TO match;
ALTER TABLE subscriptions RENAME COLUMN seen_rev TO seen_seq;
UPDATE subscriptions SET seen_seq = CASE
  WHEN seen_seq > 0 THEN COALESCE(
    (SELECT r.seq FROM readings AS r WHERE r.key = subscriptions.match AND r.rev = subscriptions.seen_seq),
    (SELECT MAX(r.seq) FROM readings AS r WHERE r.key = subscriptions.match AND r.rev < subscriptions.seen_seq), 0)
  WHEN EXISTS (SELECT 1 FROM readings AS r WHERE r.key = subscriptions.match) THEN 0
  ELSE (SELECT last FROM log_seq) END;
UPDATE subscriptions SET doc = json_set(json_remove(doc, '$.key', '$.seenRev'), '$.match', match, '$.seenSeq', seen_seq);
