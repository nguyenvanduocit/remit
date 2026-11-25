-- A card's session init and context count are kept as Claude Code reported them, beside when Remit heard them
-- (ADR-0107 decision 4): the card and its home are the row's key and column, and the document is {init|context, at}.
-- The card, home and time an earlier build wrote into the report itself are moved out of it; a document that has its
-- report under `init` or `context` already is left as it is.
UPDATE session_inits SET doc = CASE json_type(doc, '$.at')
  WHEN 'text' THEN json_object('init', json(json_remove(doc, '$.cardId', '$.home', '$.at')), 'at', json_extract(doc, '$.at'))
  ELSE json_object('init', json(json_remove(doc, '$.cardId', '$.home', '$.at')))
END
WHERE json_type(doc, '$.init') IS NULL;

UPDATE session_contexts SET doc = CASE json_type(doc, '$.at')
  WHEN 'text' THEN json_object('context', json(json_remove(doc, '$.cardId', '$.home', '$.at')), 'at', json_extract(doc, '$.at'))
  ELSE json_object('context', json(json_remove(doc, '$.cardId', '$.home', '$.at')))
END
WHERE json_type(doc, '$.context') IS NULL;
