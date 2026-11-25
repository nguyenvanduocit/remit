
UPDATE employees SET doc = json_set(json_remove(doc, '$.tenure'),
  '$.number', (SELECT n FROM (SELECT id AS eid, ROW_NUMBER() OVER (ORDER BY created_at, id) AS n FROM employees) WHERE eid = employees.id),
  '$.label', CASE WHEN json_extract(doc, '$.kind') = 'agent' AND substr(id, 3) IN (SELECT id FROM cards) THEN '' ELSE json_extract(doc, '$.label') END,
  '$.rev', COALESCE(json_extract(doc, '$.rev'), 0) + 1)
WHERE json_type(doc, '$.tenure') IS NOT NULL OR json_type(doc, '$.number') IS NULL;
