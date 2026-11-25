
UPDATE skills SET doc = json_set(doc, '$.group', 'general') WHERE json_type(doc, '$.group') IS NULL;
