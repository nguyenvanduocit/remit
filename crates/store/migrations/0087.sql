
CREATE TABLE IF NOT EXISTS mentors (id TEXT PRIMARY KEY, created_at INTEGER NOT NULL, doc TEXT NOT NULL);
INSERT INTO mentors(id, created_at, doc)
SELECT id, created_at, json_object('id', id, 'label', coalesce(json_extract(doc, '$.label'), ''),
    'strengths', coalesce(json_extract(doc, '$.scope'), ''),
    'status', CASE json_extract(doc, '$.status') WHEN 'retired' THEN 'removed' ELSE 'active' END,
    'availability', json_extract(doc, '$.availability'), 'rev', 1, 'createdAt', json_extract(doc, '$.createdAt'))
FROM employees WHERE json_extract(doc, '$.kind') = 'person' ON CONFLICT(id) DO NOTHING;
UPDATE mentors SET doc = json_remove(doc, '$.availability') WHERE json_extract(doc, '$.availability') IS NULL;
UPDATE mentors SET doc = json_remove(doc, '$.createdAt') WHERE json_extract(doc, '$.createdAt') IS NULL;
UPDATE cards SET doc = json_set(json_remove(doc, '$.employee', '$.offer', '$.line'), '$.status',
    CASE WHEN json_extract(doc, '$.status') IN ('failed', 'stopped') THEN json_extract(doc, '$.status') ELSE 'stopped' END,
    '$.rev', json_extract(doc, '$.rev') + 1)
WHERE json_extract(doc, '$.employee') IN (SELECT id FROM mentors) OR json_extract(doc, '$.offer') IS NOT NULL OR json_extract(doc, '$.line') IS NOT NULL;
UPDATE inbox SET status = 'cancelled', doc = json_set(doc, '$.status', 'cancelled', '$.rev', json_extract(doc, '$.rev') + 1)
WHERE json_extract(doc, '$.kind') = 'dispute' AND status = 'open';
DELETE FROM employees WHERE json_extract(doc, '$.kind') = 'person';
UPDATE employees SET doc = json_remove(doc, '$.kind', '$.availability') WHERE json_extract(doc, '$.kind') IS NOT NULL;
CREATE TABLE IF NOT EXISTS worker_keys_new (id TEXT PRIMARY KEY, mentor TEXT NOT NULL, sealed BLOB NOT NULL, key_id TEXT NOT NULL DEFAULT '', doc TEXT NOT NULL);
INSERT INTO worker_keys_new SELECT id, employee, sealed, key_id, json_set(json_remove(doc, '$.employee'), '$.mentor', employee) FROM worker_keys;
DROP TABLE worker_keys;
ALTER TABLE worker_keys_new RENAME TO worker_keys;
CREATE INDEX worker_keys_mentor ON worker_keys(mentor);
