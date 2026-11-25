
CREATE TABLE IF NOT EXISTS objectives (id TEXT PRIMARY KEY, created_at INTEGER NOT NULL, doc TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS objectives_status ON objectives(json_extract(doc, '$.status'));
INSERT INTO objectives(id, created_at, doc)
SELECT 'o_' || c.id, c.created_at, json_object(
  'id', 'o_' || c.id, 'rev', 1, 'task', json('true'), 'status', 'active', 'owner', 'owner', 'scope', '',
  'outcome', COALESCE(json_extract(c.doc, '$.goal'), ''),
  'keyResults', json_array(json_object('id', 'k_' || c.id, 'text', COALESCE(json_extract(c.doc, '$.goal'), ''), 'owner', 'owner', 'judge', 'owner',
    'check', json_object('kind', 'submit', 'how', ''))),
  'createdAt', strftime('%Y-%m-%dT%H:%M:%S', c.created_at / 1000, 'unixepoch') || printf('.%03dZ', c.created_at % 1000))
FROM chains AS c
WHERE json_type(c.doc, '$.serves') IS NULL AND NOT EXISTS (SELECT 1 FROM objectives WHERE id = 'o_' || c.id);
UPDATE chains SET doc = json_set(doc, '$.serves', json_object('objective', 'o_' || id, 'keyResult', 'k_' || id))
WHERE json_type(doc, '$.serves') IS NULL;
UPDATE approvals SET doc = json_set(doc, '$.findings', json((
  SELECT json_group_array(json(
    CASE WHEN json_type(f.value, '$.requirement') IS NULL THEN f.value
    ELSE json_remove(
      CASE WHEN EXISTS (SELECT 1 FROM chains WHERE chains.id = approvals.chain_id)
        THEN json_set(f.value, '$.item', json_extract(f.value, '$.requirement'), '$.keyResult', 'k_' || approvals.chain_id)
        ELSE json_set(f.value, '$.item', json_extract(f.value, '$.requirement')) END,
      '$.requirement') END) ORDER BY f.key)
  FROM json_each(approvals.doc, '$.findings') AS f)))
WHERE EXISTS (SELECT 1 FROM json_each(approvals.doc, '$.findings') AS f WHERE json_type(f.value, '$.requirement') IS NOT NULL);
