
UPDATE chains SET doc = json_set(doc, '$.repos', json((
  SELECT json_group_array(CASE
    WHEN json_type(r.value, '$.mr') = 'object' AND json_extract(r.value, '$.pushedSha') IS NOT NULL
      THEN json_set(r.value, '$.mr.head', json_extract(r.value, '$.pushedSha'))
    ELSE json(r.value) END ORDER BY r.key)
  FROM json_each(doc, '$.repos') AS r)))
WHERE json_type(doc, '$.repos') = 'array';
UPDATE review_threads SET doc = json_set(doc,
  '$.head', COALESCE((SELECT json_extract(r.value, '$.mr.head') FROM chains AS c, json_each(c.doc, '$.repos') AS r
    WHERE c.id = review_threads.chain_id AND json_extract(r.value, '$.repo') = json_extract(review_threads.doc, '$.repo')), ''),
  '$.outdated', json('false'))
WHERE COALESCE(json_extract(doc, '$.head'), '') = '';
