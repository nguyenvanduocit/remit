
UPDATE employees SET doc = json_set(doc, '$.ceiling', json('{"effect":"reversible"}'), '$.rev', json_extract(doc, '$.rev') + 1)
WHERE json_extract(doc, '$.title') = 'chair' AND json_extract(doc, '$.status') = 'active'
  AND coalesce(json_extract(doc, '$.ceiling.effect'), '') = ''
  AND coalesce(json_array_length(doc, '$.ceiling.grants'), 0) = 0 AND coalesce(json_array_length(doc, '$.ceiling.tools'), 0) = 0;
