
UPDATE cards SET doc = json_remove(doc, '$.queued') WHERE json_type(doc, '$.queued') IS NOT NULL;
