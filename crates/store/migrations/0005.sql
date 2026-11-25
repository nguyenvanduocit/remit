
ALTER TABLE events ADD COLUMN priority INTEGER NOT NULL DEFAULT 0;
UPDATE events SET priority = 1 WHERE kind='prompt' AND status='queued' AND from_who='owner';
DROP INDEX events_queued;
CREATE INDEX events_queued ON events(card_id, priority DESC, id) WHERE kind='prompt' AND status='queued';
