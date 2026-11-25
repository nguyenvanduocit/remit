
UPDATE agents SET doc = json_set(json_remove(doc, '$.brief'), '$.instructions', json_extract(doc, '$.brief')) WHERE json_type(doc, '$.brief') IS NOT NULL;
