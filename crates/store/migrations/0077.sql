
UPDATE cards SET doc = json_set(doc,
  '$.offer', json_object('state', 'accepted', 'question', json('true'), 'offeredAt', json_extract(doc, '$.createdAt'), 'answeredAt', json_extract(doc, '$.createdAt')),
  '$.rev', json_extract(doc, '$.rev') + 1)
WHERE json_type(doc, '$.offer') IS NULL
  AND json_extract(doc, '$.employee') IN (SELECT id FROM employees WHERE json_extract(doc, '$.kind') = 'person');
