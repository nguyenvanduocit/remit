
UPDATE subscriptions SET
  doc = json_set(json_remove(subscriptions.doc, '$.once', '$.match'),
    '$.schedule', json(json_patch('{}', json_object(
      'cron', NULLIF(json_extract(sn.doc, '$.clock.cron'), ''),
      'at', CASE WHEN COALESCE(json_extract(sn.doc, '$.clock.cron'), '') = '' THEN json_extract(sn.doc, '$.clock.next') END,
      'last', json_extract(sn.doc, '$.clock.last')))),
    '$.dueAt', json_extract(sn.doc, '$.clock.next')),
  match = '',
  due_at = CAST(strftime('%s', json_extract(sn.doc, '$.clock.next')) AS INTEGER) * 1000 + CAST(substr(strftime('%f', json_extract(sn.doc, '$.clock.next')), 4) AS INTEGER)
FROM sensors AS sn
WHERE sn.id = 'clock_' || subscriptions.id AND json_extract(sn.doc, '$.kind') = 'clock' AND subscriptions.match GLOB 'clock:*';
UPDATE subscriptions SET
  doc = json_set(json_remove(doc, '$.once', '$.match', '$.dueAt'),
    '$.schedule', json(json_patch('{}', json_object(
      'at', COALESCE(json_extract(doc, '$.lastFired'), json_extract(doc, '$.createdAt')),
      'last', json_extract(doc, '$.lastFired'))))),
  match = '',
  due_at = NULL
WHERE match GLOB 'clock:*';
UPDATE subscriptions SET
  due_at = CAST(strftime('%s', json_extract(doc, '$.until')) AS INTEGER) * 1000 + CAST(substr(strftime('%f', json_extract(doc, '$.until')), 4) AS INTEGER)
WHERE json_type(doc, '$.until') = 'text' AND json_type(doc, '$.dueAt') IS NULL;
UPDATE subscriptions SET doc = json_set(doc, '$.overlap', 'buffer_one')
WHERE owner_kind = 'mindchains' AND json_type(doc, '$.schedule') = 'object' AND json_extract(doc, '$.action.kind') = 'start';
UPDATE firings SET
  key = 'schedule:' || substr(key, 7),
  doc = json_remove(json_set(doc, '$.key', 'schedule:' || substr(key, 7), '$.cause', 'scheduled'), '$.seqs')
WHERE key GLOB 'clock:*';
DELETE FROM near_misses WHERE json_extract(doc, '$.key') GLOB 'clock:*';
DELETE FROM readings WHERE key GLOB 'clock:*';
DELETE FROM sensors WHERE json_extract(doc, '$.kind') = 'clock';
