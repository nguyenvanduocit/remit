
UPDATE chains SET doc = json_set(doc, '$.archivedAt', json_extract(doc, '$.updatedAt'))
WHERE json_extract(doc, '$.origin') = 'scout' AND json_extract(doc, '$.status') = 'done' AND json_type(doc, '$.archivedAt') IS NULL;
