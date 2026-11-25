
DELETE FROM firings WHERE json_extract(doc, '$.action') IN ('dispatch', 'skip');
