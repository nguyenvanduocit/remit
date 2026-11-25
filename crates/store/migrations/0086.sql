
UPDATE playbooks SET doc = json_set(doc, '$.chair', (
    SELECT e.id FROM employees e
    WHERE json_extract(e.doc, '$.title') = 'chair' AND json_extract(e.doc, '$.status') = 'active'
      AND json_extract(e.doc, '$.template') = json_extract(playbooks.doc, '$.chair')
    ORDER BY e.created_at, e.id LIMIT 1), '$.rev', json_extract(doc, '$.rev') + 1)
WHERE coalesce(json_extract(doc, '$.chair'), '') <> '' AND json_extract(doc, '$.chair') NOT IN (SELECT id FROM employees)
  AND EXISTS (SELECT 1 FROM employees e
    WHERE json_extract(e.doc, '$.title') = 'chair' AND json_extract(e.doc, '$.status') = 'active'
      AND json_extract(e.doc, '$.template') = json_extract(playbooks.doc, '$.chair'));
UPDATE playbooks SET doc = json_set(json_remove(doc, '$.chair'), '$.rev', json_extract(doc, '$.rev') + 1)
WHERE coalesce(json_extract(doc, '$.chair'), '') <> '' AND json_extract(doc, '$.chair') NOT IN (SELECT id FROM employees);
