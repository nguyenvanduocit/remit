
ALTER TABLE firings ADD COLUMN status TEXT NOT NULL DEFAULT 'done' CHECK (status IN ('pending', 'done', 'failed', 'skipped', 'cancelled'));
UPDATE firings SET doc = json_remove(json_set(doc, '$.status', 'done', '$.attempts', 1,
    '$.cause', CASE WHEN COALESCE(json_extract(doc, '$.expired'), 0) THEN 'expired' ELSE 'reading' END), '$.expired')
WHERE json_type(doc, '$.status') IS NULL;
CREATE INDEX firings_open ON firings(key) WHERE status IN ('pending', 'failed');
ALTER TABLE subscriptions ADD COLUMN control TEXT NOT NULL DEFAULT 'active' CHECK (control IN ('active', 'paused', 'ended'));
UPDATE subscriptions SET control = CASE
    WHEN paused THEN 'paused'
    WHEN owner_kind = 'mindchains' AND match GLOB 'clock:*' AND NOT EXISTS (SELECT 1 FROM sensors
      WHERE sensors.id = 'clock_' || subscriptions.id AND json_extract(sensors.doc, '$.clock.next') IS NOT NULL) THEN 'ended'
    ELSE 'active' END;
UPDATE subscriptions SET doc = json_remove(CASE control WHEN 'active' THEN doc ELSE json_set(doc, '$.control', control) END, '$.paused')
WHERE json_type(doc, '$.paused') IS NOT NULL OR control <> 'active' AND json_type(doc, '$.control') IS NULL;
DROP INDEX subscriptions_key;
DROP INDEX subscriptions_due;
ALTER TABLE subscriptions DROP COLUMN paused;
CREATE INDEX subscriptions_key ON subscriptions(match, control);
CREATE INDEX subscriptions_due ON subscriptions(due_at) WHERE control = 'active' AND due_at IS NOT NULL;
CREATE TABLE near_misses (subscription_id TEXT NOT NULL, seq INTEGER NOT NULL, reason TEXT NOT NULL, at INTEGER NOT NULL, doc TEXT NOT NULL,
  PRIMARY KEY(subscription_id, seq));
