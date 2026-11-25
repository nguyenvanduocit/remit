
UPDATE subscriptions SET doc = json_set(doc, '$.builtin', json('false'))
WHERE id = 'rt_self_improvement' AND json_extract(doc, '$.edited') = 1;
DELETE FROM subscriptions WHERE id = 'rt_self_improvement' AND COALESCE(json_extract(doc, '$.edited'), 0) <> 1;
