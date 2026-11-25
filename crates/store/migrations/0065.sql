
UPDATE objectives SET doc = json_set(doc, '$.guardrails', json((
  SELECT json_group_array(json(CASE WHEN json_type(g.value, '$.bound') IS NULL THEN g.value
    ELSE json_object('kpi', json_extract(g.value, '$.kpi'), 'signal', '', 'condition', json_extract(g.value, '$.bound')) END))
  FROM json_each(json_extract(doc, '$.guardrails')) g)))
WHERE json_type(doc, '$.guardrails') = 'array'
  AND EXISTS (SELECT 1 FROM json_each(json_extract(doc, '$.guardrails')) WHERE json_type(value, '$.bound') IS NOT NULL);
