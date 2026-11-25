
UPDATE employees SET doc = json_set(doc, '$.kind', 'agent', '$.rev', json_extract(doc, '$.rev') + 1) WHERE json_type(doc, '$.kind') IS NULL;
