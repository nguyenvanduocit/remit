
INSERT INTO subscriptions(id, owner_kind, owner_id, key, paused, due_at, seen_rev, doc)
SELECT id, 'mindchains', '', 'clock', paused,
  CASE WHEN next IS NULL THEN NULL ELSE CAST(strftime('%s', next) AS INTEGER) * 1000 END, 0,
  json_remove(json_object(
    'id', id,
    'owner', json_object('kind', 'mindchains'),
    'key', 'clock',
    'cron', cron,
    'next', next,
    'window', json_object(),
    'action', json_object('kind', 'start', 'start', json_object('title', title, 'goal', goal, 'playbookId', playbook, 'origin', 'routine')),
    'paused', json(CASE WHEN paused THEN 'true' ELSE 'false' END),
    'builtin', json(CASE WHEN builtin THEN 'true' ELSE 'false' END),
    'edited', json(CASE WHEN edited THEN 'true' ELSE 'false' END),
    'by', json(by),
    'createdAt', created_at,
    'updatedAt', updated_at,
    'rev', rev), CASE WHEN next IS NULL THEN '$.next' ELSE '$.window.none' END)
FROM (
  SELECT id,
    COALESCE(json_extract(doc, '$.title'), '') AS title,
    COALESCE(json_extract(doc, '$.goal'), '') AS goal,
    COALESCE(json_extract(doc, '$.playbookId'), '') AS playbook,
    COALESCE(json_extract(doc, '$.cron'), '') AS cron,
    CASE WHEN json_extract(doc, '$.nextAt') IS NULL OR json_extract(doc, '$.nextAt') LIKE '0001-%' THEN NULL ELSE json_extract(doc, '$.nextAt') END AS next,
    COALESCE(json_extract(doc, '$.paused'), 0) AS paused,
    COALESCE(json_extract(doc, '$.builtin'), 0) AS builtin,
    COALESCE(json_extract(doc, '$.edited'), 0) AS edited,
    COALESCE(json_extract(doc, '$.by'), '{}') AS by,
    json_extract(doc, '$.createdAt') AS created_at,
    json_extract(doc, '$.updatedAt') AS updated_at,
    COALESCE(json_extract(doc, '$.rev'), 1) AS rev
  FROM routines);
DROP TABLE routines;
UPDATE chains SET doc = json_set(doc, '$.routineId', 'rt_scout') WHERE json_extract(doc, '$.origin') = 'scout' AND json_type(doc, '$.routineId') IS NULL;
INSERT INTO subscriptions(id, owner_kind, owner_id, key, paused, due_at, seen_rev, doc)
SELECT 'sub_cb_' || id, 'chain', id, 'clock', 0, CAST(strftime('%s', at) AS INTEGER) * 1000, 0,
  json_object(
    'id', 'sub_cb_' || id,
    'owner', json_object('kind', 'chain', 'id', id),
    'key', 'clock',
    'next', at,
    'window', json_object(),
    'action', json_object('kind', 'wake', 'why', why),
    'by', json_object('kind', 'agent', 'who', chair_id),
    'createdAt', set_at,
    'updatedAt', set_at,
    'rev', 1)
FROM (
  SELECT id,
    COALESCE(json_extract(doc, '$.chairId'), '') AS chair_id,
    json_extract(doc, '$.checkBack.at') AS at,
    COALESCE(json_extract(doc, '$.checkBack.why'), '') AS why,
    COALESCE(json_extract(doc, '$.checkBack.setAt'), json_extract(doc, '$.checkBack.at')) AS set_at
  FROM chains WHERE json_type(doc, '$.checkBack') = 'object' AND json_extract(doc, '$.checkBack.at') IS NOT NULL);
UPDATE chains SET doc = json_remove(doc, '$.checkBack') WHERE json_type(doc, '$.checkBack') IS NOT NULL;
CREATE INDEX events_stopped ON events(home, kind, id) WHERE status='stopped';
INSERT INTO subscriptions(id, owner_kind, owner_id, key, paused, due_at, seen_rev, doc)
SELECT 'sub_wh_' || name || '_skip', 'webhook', name, 'delivery:' || name, 0, NULL, 0,
  json_object('id', 'sub_wh_' || name || '_skip', 'owner', json_object('kind', 'webhook', 'id', name), 'key', 'delivery:' || name,
    'expr', 'value.delivery.does == ''skip''', 'window', json_object(), 'action', json_object('kind', 'skip'),
    'by', json_object('kind', 'mindchains', 'who', 'webhook'), 'createdAt', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), 'updatedAt', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), 'rev', 1)
FROM webhooks;
INSERT INTO subscriptions(id, owner_kind, owner_id, key, paused, due_at, seen_rev, doc)
SELECT 'sub_wh_' || name || '_dispatch', 'webhook', name, 'delivery:' || name, 0, NULL, 0,
  json_object('id', 'sub_wh_' || name || '_dispatch', 'owner', json_object('kind', 'webhook', 'id', name), 'key', 'delivery:' || name,
    'window', json_object(), 'action', json_object('kind', 'dispatch'), 'position', 1000,
    'by', json_object('kind', 'mindchains', 'who', 'webhook'), 'createdAt', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), 'updatedAt', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), 'rev', 1)
FROM webhooks;
CREATE INDEX readings_delivery ON readings(json_extract(doc, '$.value.delivery.id'));
ALTER TABLE firings ADD COLUMN owner_kind TEXT NOT NULL DEFAULT '';
ALTER TABLE firings ADD COLUMN owner_id TEXT NOT NULL DEFAULT '';
CREATE INDEX firings_owner ON firings(owner_kind, owner_id, at);
