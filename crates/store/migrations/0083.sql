
DROP TABLE IF EXISTS worker_signins;
CREATE TABLE worker_keys_new (id TEXT PRIMARY KEY, employee TEXT NOT NULL, sealed BLOB NOT NULL, key_id TEXT NOT NULL DEFAULT '', doc TEXT NOT NULL);
INSERT INTO worker_keys_new SELECT id, employee, sealed, key_id,
  json_set(json_remove(doc, '$.device', '$.account'), '$.note', coalesce(json_extract(doc, '$.account'), ''), '$.rev', json_extract(doc, '$.rev') + 1)
FROM worker_keys;
DROP TABLE worker_keys;
ALTER TABLE worker_keys_new RENAME TO worker_keys;
CREATE INDEX worker_keys_employee ON worker_keys(employee);
UPDATE employees SET doc = json_set(json_remove(doc, '$.email'), '$.rev', json_extract(doc, '$.rev') + 1) WHERE json_type(doc, '$.email') IS NOT NULL;
UPDATE inbox SET status = 'cancelled', doc = json_set(doc, '$.status', 'cancelled', '$.answer', 'a person works from the codes the owner has made for them, not a Google account')
WHERE status = 'open' AND json_extract(doc, '$.change.kind') = 'employee' AND json_extract(json_extract(doc, '$.change.record'), '$.action') = 'account';
UPDATE inbox SET doc = json_set(doc, '$.change.record', json_remove(json_extract(doc, '$.change.record'), '$.hire.email'))
WHERE status = 'open' AND json_extract(doc, '$.change.kind') = 'employee' AND json_type(json_extract(doc, '$.change.record'), '$.hire.email') IS NOT NULL;
