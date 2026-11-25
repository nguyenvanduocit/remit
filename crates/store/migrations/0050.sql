
UPDATE chains SET doc = json_set(doc, '$.origin', 'routine') WHERE json_extract(doc, '$.origin') = 'scout';
UPDATE subscriptions SET doc = json_set(doc, '$.builtin', json('false'), '$.action.start.origin', 'routine')
WHERE id = 'rt_scout' AND json_extract(doc, '$.edited') = 1;
DELETE FROM subscriptions WHERE id = 'rt_scout' AND COALESCE(json_extract(doc, '$.edited'), 0) <> 1;
