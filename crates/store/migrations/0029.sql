
UPDATE webhooks SET doc = json_set(doc,
  '$.prompt', COALESCE((SELECT group_concat(line, char(10)) FROM (
      SELECT json_extract(r.doc, '$.title') || ': ' || json_extract(r.doc, '$.words') AS line
      FROM reactions r
      WHERE COALESCE(json_extract(r.doc, '$.paused'), 0) = 0
        AND (COALESCE(json_extract(r.doc, '$.source'), '') = '' OR lower(json_extract(r.doc, '$.source')) = webhooks.name)
      ORDER BY json_extract(r.doc, '$.createdAt'), r.id)), ''),
  '$.precondition', json('{"kind":"gitlab"}'));
UPDATE webhooks SET doc = json_remove(doc, '$.prompt') WHERE json_extract(doc, '$.prompt') = '';
DROP TABLE reactions;
