
CREATE TABLE IF NOT EXISTS placements (id TEXT PRIMARY KEY, dedupe TEXT NOT NULL UNIQUE, signal TEXT NOT NULL, reading_rev INTEGER NOT NULL, status TEXT NOT NULL, at INTEGER NOT NULL, doc TEXT NOT NULL);
INSERT OR IGNORE INTO subscriptions(id, owner_kind, owner_id, key, paused, due_at, seen_rev, doc)
WITH rules AS (
  SELECT es.name AS name, CAST(j.key AS INTEGER) + 1 AS n, 'event:' || es.name AS key,
    json_extract(j.value, '$.when') AS wh, json_extract(j.value, '$.does') AS does, json_extract(j.value, '$.marks') AS marks,
    json_extract(j.value, '$.summary') AS summary, json_extract(j.value, '$.quote') AS quote, json_extract(j.value, '$.author') AS author,
    COALESCE((SELECT MAX(r.rev) FROM readings AS r WHERE r.key = 'event:' || es.name), 0) AS seen,
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now') AS at
  FROM event_signals AS es, json_each(es.doc, '$.preconditions') AS j
  WHERE (COALESCE(json_extract(es.doc, '$.builtin'), 0) = 0 OR COALESCE(json_extract(es.doc, '$.edited'), 0) = 1)
    AND json_extract(j.value, '$.marks') IS NOT NULL),
subs AS (
  SELECT 'sub_es_' || name || '_' || n AS id, key, wh, seen, at,
    json_object('kind', 'tell', 'with', json_patch(json_object(), json_object('chain', marks, 'summary', summary, 'quote', quote, 'author', author))) AS action
  FROM rules WHERE does = 'chair'
  UNION ALL
  SELECT 'sub_es_' || name || '_' || n || '_sync', key, wh, seen, at,
    json_object('kind', 'run', 'job', 'sync-merge-request', 'with', json_object('chain', marks))
  FROM rules WHERE does = 'chair' AND (instr(wh, 'merge_request') > 0 OR instr(wh, 'pull_request') > 0)
  UNION ALL
  SELECT 'sub_es_' || name || '_' || n, key, wh, seen, at,
    json_object('kind', 'run', 'job', 'mark-live', 'with', json_patch(json_object(), json_object('chain', marks, 'summary', summary)))
  FROM rules WHERE does = 'live')
SELECT id, 'mindchains', '', key, 0, NULL, seen,
  json_set(json_object('id', id, 'owner', json_object('kind', 'mindchains'), 'key', key, 'expr', wh, 'window', json_object(), 'action', json(action),
    'seenRev', seen, 'by', json_object('kind', 'mindchains', 'who', 'migration'), 'createdAt', at, 'updatedAt', at), '$.rev', 1)
FROM subs;
UPDATE readings SET doc = json_set(doc, '$.value', json_object('event', json(json_extract(doc, '$.value.event'))))
WHERE substr(key, 1, 6) = 'event:' AND json_type(doc, '$.value.found') IS NOT NULL;
UPDATE readings SET doc = json_set(readings.doc, '$.dedupe', p.dedupe)
FROM placements AS p WHERE readings.key = 'event:' || p.signal AND readings.rev = p.reading_rev;
CREATE UNIQUE INDEX IF NOT EXISTS readings_dedupe ON readings(key, json_extract(doc, '$.dedupe')) WHERE json_extract(doc, '$.dedupe') IS NOT NULL;
CREATE INDEX IF NOT EXISTS firings_key ON firings(key, rev);
DELETE FROM subscriptions WHERE substr(id, 1, 9) = 'sub_tell_' OR substr(id, 1, 7) = 'sub_es_' AND (substr(id, -5) = '_skip' OR substr(id, -9) = '_dispatch');
UPDATE subscriptions SET doc = json_remove(doc, '$.position', '$.window.quiet', '$.window.longest')
WHERE json_type(doc, '$.position') IS NOT NULL OR json_type(doc, '$.window.quiet') IS NOT NULL OR json_type(doc, '$.window.longest') IS NOT NULL;
UPDATE firings SET owner_kind = 'mindchains', owner_id = '', doc = json_set(json_remove(doc, '$.owner.id'), '$.owner.kind', 'mindchains') WHERE owner_kind = 'event_signal';
DELETE FROM cards WHERE substr(home, 1, 3) IN ('pl_', 'dl_');
DELETE FROM drafts WHERE substr(home, 1, 3) IN ('pl_', 'dl_');
DELETE FROM inbox WHERE substr(home, 1, 3) IN ('pl_', 'dl_');
DELETE FROM session_inits WHERE substr(home, 1, 3) IN ('pl_', 'dl_');
DELETE FROM decisions WHERE substr(chain_id, 1, 3) IN ('pl_', 'dl_');
UPDATE chains SET doc = json_remove(doc, '$.placementId') WHERE json_type(doc, '$.placementId') IS NOT NULL;
UPDATE event_signals SET doc = json_remove(doc, '$.preconditions', '$.prompt') WHERE json_type(doc, '$.preconditions') IS NOT NULL OR json_type(doc, '$.prompt') IS NOT NULL;
DROP TABLE IF EXISTS placements;
