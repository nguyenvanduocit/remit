
UPDATE subscriptions SET doc = json_set(doc, '$.until', json_extract(doc, '$.window.until'))
WHERE json_type(doc, '$.window.until') IS NOT NULL;
UPDATE subscriptions SET doc = json_remove(json_set(doc, '$.flow', json(CASE
    WHEN COALESCE(json_extract(doc, '$.window.cooldown'), 0) > 0 THEN json_object(
      'policy', CASE WHEN json_extract(doc, '$.action.kind') = 'wake' THEN 'batch' ELSE 'delay' END,
      'period', json_extract(doc, '$.window.cooldown'))
    ELSE json_object('policy', 'each') END)), '$.window')
WHERE json_type(doc, '$.window') IS NOT NULL OR json_type(doc, '$.flow') IS NULL;
UPDATE firings SET doc = json_remove(json_set(doc, '$.seqs', json(CASE
    WHEN COALESCE(json_extract(doc, '$.expired'), 0) THEN '[]'
    ELSE (SELECT json_group_array(seq) FROM (SELECT r.seq FROM readings AS r
      WHERE r.key = firings.key AND r.rev > COALESCE(json_extract(firings.doc, '$.fromRev'), 0) AND r.rev <= firings.rev ORDER BY r.seq)) END)), '$.fromRev')
WHERE json_type(doc, '$.fromRev') IS NOT NULL OR json_type(doc, '$.seqs') IS NULL AND COALESCE(json_extract(doc, '$.expired'), 0) = 0;
