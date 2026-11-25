
UPDATE agents SET doc = json_set(doc, '$.group', 'general') WHERE json_type(doc, '$.group') IS NULL;
UPDATE playbooks SET doc = json_set(doc, '$.group', 'general') WHERE json_type(doc, '$.group') IS NULL;
