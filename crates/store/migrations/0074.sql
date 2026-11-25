
UPDATE cards SET doc = json_set(doc,
  '$.line', json_object('prompt', (SELECT max(e.id) FROM events AS e WHERE e.card_id = cards.id AND e.kind = 'prompt' AND e.status = 'delivered'),
    'state', CASE json_extract(doc, '$.status') WHEN 'running' THEN 'open' ELSE 'paused' END),
  '$.rev', json_extract(doc, '$.rev') + 1)
WHERE json_type(doc, '$.line') IS NULL
  AND (json_extract(doc, '$.status') = 'running' OR json_extract(doc, '$.status') = 'idle' AND json_extract(doc, '$.activity') = 'paused')
  AND json_extract(doc, '$.employee') IN (SELECT id FROM employees WHERE json_extract(doc, '$.kind') = 'person')
  AND home NOT IN (SELECT id FROM chains WHERE json_extract(doc, '$.status') = 'done')
  AND EXISTS (SELECT 1 FROM events AS e WHERE e.card_id = cards.id AND e.kind = 'prompt' AND e.status = 'delivered');
