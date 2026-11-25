
CREATE TABLE IF NOT EXISTS employees (id TEXT PRIMARY KEY, created_at INTEGER NOT NULL, doc TEXT NOT NULL);
INSERT INTO employees(id, created_at, doc)
SELECT id, created_at, CASE WHEN loadout IS NULL THEN doc ELSE json_set(doc, '$.loadout', json(loadout)) END
FROM (
  SELECT 'e_' || k.id AS id, k.created_at AS created_at, json_extract(k.doc, '$.loadout') AS loadout, json_object(
    'id', 'e_' || k.id, 'label', json_extract(k.doc, '$.agent'), 'template', json_extract(k.doc, '$.agent'), 'templateRev', 0,
    'title', CASE json_extract(k.doc, '$.origin') WHEN 'chair' THEN 'chair' ELSE 'worker' END,
    'scope', '', 'reportsTo', 'owner', 'hiredBy', 'owner', 'tenure', 'chain',
    'status', CASE WHEN EXISTS (SELECT 1 FROM chains AS c WHERE c.id = k.home
        AND (json_extract(c.doc, '$.status') = 'done' OR json_type(c.doc, '$.archivedAt') IS NOT NULL)) THEN 'retired' ELSE 'active' END,
    'ceiling', json_object('effect', '', 'grants', json_array(), 'tools', json_array()),
    'rev', 1,
    'createdAt', strftime('%Y-%m-%dT%H:%M:%S', k.created_at / 1000, 'unixepoch') || printf('.%03dZ', k.created_at % 1000)) AS doc
  FROM cards AS k
  WHERE json_extract(k.doc, '$.origin') IN ('chair', 'hired', 'seed')
    AND NOT EXISTS (SELECT 1 FROM employees WHERE employees.id = 'e_' || k.id));
UPDATE cards SET doc = json_set(doc, '$.employee', 'e_' || id)
WHERE json_extract(doc, '$.origin') IN ('chair', 'hired', 'seed') AND json_type(doc, '$.employee') IS NULL;
