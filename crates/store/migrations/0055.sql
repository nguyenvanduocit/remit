
UPDATE playbooks SET doc = json_set(doc, '$.capabilities', json(json_insert(COALESCE(json_extract(doc, '$.capabilities'), '[]'), '$[#]', 'shell')))
WHERE NOT EXISTS (SELECT 1 FROM json_each(COALESCE(json_extract(doc, '$.capabilities'), '[]')) WHERE value = 'shell');
UPDATE playbooks SET doc = json_set(doc, '$.capabilities', json(json_insert(COALESCE(json_extract(doc, '$.capabilities'), '[]'), '$[#]', 'web')))
WHERE NOT EXISTS (SELECT 1 FROM json_each(COALESCE(json_extract(doc, '$.capabilities'), '[]')) WHERE value = 'web');
UPDATE chains SET doc = json_set(doc, '$.capabilities', json(json_insert(COALESCE(json_extract(doc, '$.capabilities'), '[]'), '$[#]', 'shell')))
WHERE NOT EXISTS (SELECT 1 FROM json_each(COALESCE(json_extract(doc, '$.capabilities'), '[]')) WHERE value = 'shell');
UPDATE chains SET doc = json_set(doc, '$.capabilities', json(json_insert(COALESCE(json_extract(doc, '$.capabilities'), '[]'), '$[#]', 'web')))
WHERE NOT EXISTS (SELECT 1 FROM json_each(COALESCE(json_extract(doc, '$.capabilities'), '[]')) WHERE value = 'web');
