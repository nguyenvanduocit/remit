
UPDATE chains SET doc = json_set(doc, '$.rev', 1);
