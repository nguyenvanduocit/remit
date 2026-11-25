
UPDATE chains SET doc = json_set(doc, '$.repos', json((
  SELECT json_group_array(json(
    CASE WHEN json_extract(v, '$.submit.head') IS NOT NULL AND json_type(v, '$.submit.from') IS NULL
         THEN json_set(v, '$.submit.from', json_extract(v, '$.submit.head')) ELSE v END))
  FROM (SELECT CASE WHEN json_extract(value, '$.pushedSha') IS NOT NULL AND json_type(value, '$.pushedFrom') IS NULL
                    THEN json_set(value, '$.pushedFrom', json_extract(value, '$.pushedSha')) ELSE value END AS v
        FROM json_each(chains.doc, '$.repos') ORDER BY key))),
  '$.rev', json_extract(doc, '$.rev') + 1)
WHERE EXISTS (SELECT 1 FROM json_each(chains.doc, '$.repos') AS r
              WHERE json_extract(r.value, '$.pushedSha') IS NOT NULL AND json_type(r.value, '$.pushedFrom') IS NULL
                 OR json_extract(r.value, '$.submit.head') IS NOT NULL AND json_type(r.value, '$.submit.from') IS NULL);
