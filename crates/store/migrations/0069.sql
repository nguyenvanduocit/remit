
UPDATE sensors SET doc = json_set(doc, '$.kind', CASE WHEN json_type(doc, '$.script') IS NOT NULL THEN 'script' ELSE 'poll' END) WHERE json_type(doc, '$.kind') IS NULL;
INSERT OR IGNORE INTO sensors(id, chain, name, signal, doc)
SELECT 'clock_' || s.id, s.owner_id, s.id, 'clock:' || s.id,
  json_object('id', 'clock_' || s.id, 'kind', 'clock', 'name', s.id, 'chain', s.owner_id, 'signal', 'clock:' || s.id,
    'clock', json_object('cron', COALESCE(json_extract(s.doc, '$.cron'), ''), 'next', json_extract(s.doc, '$.next')),
    'paused', json(CASE WHEN s.paused THEN 'true' ELSE 'false' END),
    'by', json(COALESCE(json_extract(s.doc, '$.by'), '{}')), 'createdAt', json_extract(s.doc, '$.createdAt'), 'updatedAt', json_extract(s.doc, '$.updatedAt'), 'rev', 1)
FROM subscriptions AS s
WHERE s.match = 'clock' AND (COALESCE(json_extract(s.doc, '$.cron'), '') != '' OR (json_extract(s.doc, '$.next') IS NOT NULL AND json_extract(s.doc, '$.next') NOT LIKE '0001-%'));
UPDATE subscriptions SET doc = json_set(doc, '$.once', json('true')) WHERE match = 'clock' AND COALESCE(json_extract(doc, '$.cron'), '') = '';
UPDATE subscriptions SET
  due_at = CASE WHEN json_extract(doc, '$.window.until') IS NULL OR json_extract(doc, '$.window.until') LIKE '0001-%' THEN NULL ELSE CAST(strftime('%s', json_extract(doc, '$.window.until')) AS INTEGER) * 1000 END,
  doc = json_set(json_remove(doc, '$.cron', '$.next'), '$.match', 'clock:' || id),
  match = 'clock:' || id
WHERE match = 'clock';
