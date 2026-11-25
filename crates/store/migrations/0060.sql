
DELETE FROM extensions WHERE json_type(doc, '$.program') IS NOT NULL AND json_type(doc, '$.tools') IS NULL;
DELETE FROM inbox WHERE status = 'open' AND json_extract(doc, '$.change.kind') = 'extension'
  AND json_type(json_extract(doc, '$.change.record'), '$.program') IS NOT NULL
  AND json_type(json_extract(doc, '$.change.record'), '$.tools') IS NULL;
