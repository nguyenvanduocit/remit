
UPDATE cards SET doc = json_set(doc, '$.model', CASE
    WHEN json_extract(doc, '$.movedFrom') IN ('opus', 'sonnet', 'haiku') THEN json_extract(doc, '$.movedFrom')
    WHEN json_extract(doc, '$.model') = 'gpt-6-astra' THEN 'opus'
    WHEN json_extract(doc, '$.model') = 'gpt-6.1-sol' THEN 'sonnet'
    ELSE 'haiku' END)
WHERE json_extract(doc, '$.model') IN ('gpt-6-astra', 'gpt-6.1-sol', 'gpt-6-luna');
UPDATE cards SET doc = json_remove(doc, '$.movedFrom') WHERE json_type(doc, '$.movedFrom') IS NOT NULL;
