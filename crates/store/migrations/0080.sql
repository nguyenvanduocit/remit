
UPDATE chains SET doc = json_set(doc, '$.repos', json((
  SELECT json_group_array(json(CASE
    WHEN json_type(r.value, '$.mr') = 'object' AND json_type(r.value, '$.mr.version') IS NULL
    THEN json_set(r.value, '$.mr.version', 1, '$.mr.checks', json_object('state', 'pending'))
    ELSE r.value END))
  FROM (SELECT value FROM json_each(chains.doc, '$.repos') ORDER BY key) AS r)),
  '$.rev', json_extract(doc, '$.rev') + 1)
WHERE EXISTS (SELECT 1 FROM json_each(chains.doc, '$.repos') AS r
              WHERE json_type(r.value, '$.mr') = 'object' AND json_type(r.value, '$.mr.version') IS NULL);
