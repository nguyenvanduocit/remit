
UPDATE cards SET doc = json_set(doc, '$.offer.ended', 'finished', '$.rev', json_extract(doc, '$.rev') + 1)
WHERE json_type(doc, '$.offer') = 'object' AND json_type(doc, '$.offer.ended') IS NULL
  AND json_extract(doc, '$.employee') IN (SELECT id FROM employees WHERE json_extract(doc, '$.kind') = 'person')
  AND home IN (SELECT id FROM chains WHERE json_extract(doc, '$.status') = 'done');
