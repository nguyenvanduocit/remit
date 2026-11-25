
CREATE TABLE IF NOT EXISTS sensors (id TEXT PRIMARY KEY, chain TEXT NOT NULL, name TEXT NOT NULL, signal TEXT NOT NULL, doc TEXT NOT NULL, UNIQUE(chain, name));
CREATE INDEX IF NOT EXISTS sensors_signal ON sensors(signal);
INSERT OR IGNORE INTO sensors(id, chain, name, signal, doc)
SELECT 'sn_pull_' || es.id, '', json_extract(es.doc, '$.name'), 'event:' || json_extract(es.doc, '$.name'),
  json_object('id', 'sn_pull_' || es.id, 'name', json_extract(es.doc, '$.name'), 'signal', 'event:' || json_extract(es.doc, '$.name'),
    'source', json_extract(es.doc, '$.pull.source'), 'connection', json_extract(es.doc, '$.pull.connection'), 'query', json_extract(es.doc, '$.pull.query'),
    'everyMinutes', json_extract(es.doc, '$.pull.everyMinutes'), 'cursor', COALESCE(json_extract(es.doc, '$.pull.cursor'), ''),
    'by', json_object('kind', 'mindchains', 'who', 'migration'), 'createdAt', json_extract(es.doc, '$.createdAt'), 'updatedAt', json_extract(es.doc, '$.updatedAt'), 'rev', 1)
FROM event_signals AS es WHERE json_type(es.doc, '$.pull') IS NOT NULL;
UPDATE sensors SET doc = json_remove(doc, '$.cursor') WHERE json_extract(doc, '$.cursor') = '';
INSERT OR IGNORE INTO sensors(id, chain, name, signal, doc)
SELECT 'sn_watch_' || s.id, s.owner_id, substr(s.key, length('chain:' || s.owner_id || '.sensor.') + 1), s.key,
  json_object('id', 'sn_watch_' || s.id, 'name', substr(s.key, length('chain:' || s.owner_id || '.sensor.') + 1), 'chain', s.owner_id, 'signal', s.key,
    'source', json_extract(s.doc, '$.sensor.source'), 'connection', json_extract(s.doc, '$.sensor.connection'),
    'script', json_object('language', json_extract(s.doc, '$.sensor.language'), 'code', json_extract(s.doc, '$.sensor.code')),
    'everyMinutes', json_extract(s.doc, '$.sensor.everyMinutes'), 'by', json(json_extract(s.doc, '$.by')),
    'createdAt', json_extract(s.doc, '$.createdAt'), 'updatedAt', json_extract(s.doc, '$.updatedAt'), 'rev', 1)
FROM subscriptions AS s WHERE json_type(s.doc, '$.sensor') IS NOT NULL AND s.owner_kind = 'chain';
UPDATE event_signals SET doc = json_remove(doc, '$.pull') WHERE json_type(doc, '$.pull') IS NOT NULL;
UPDATE subscriptions SET doc = json_remove(doc, '$.sensor') WHERE json_type(doc, '$.sensor') IS NOT NULL;
UPDATE inbox SET status = 'cancelled', doc = json_set(doc, '$.status', 'cancelled', '$.answer', 'a pull is a sensor now: ask for a sensor of this signal')
WHERE status = 'open' AND json_extract(doc, '$.change.kind') = 'event_signal' AND json_type(json_extract(doc, '$.change.record'), '$.pull') IS NOT NULL;
