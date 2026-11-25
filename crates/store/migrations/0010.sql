
ALTER TABLE events ADD COLUMN rev INTEGER NOT NULL DEFAULT 1;
UPDATE cards SET doc = json_set(doc, '$.rev', 1);
UPDATE review_threads SET doc = json_set(doc, '$.rev', 1);
UPDATE inbox SET doc = json_set(doc, '$.rev', 1);
UPDATE playbooks SET doc = json_set(doc, '$.rev', 1);
