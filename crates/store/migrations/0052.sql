
INSERT INTO readings(key, rev, at, doc)
SELECT key, rev, at, json_object('key', key, 'rev', rev, 'at', received, 'cause', json_object('actor', json_object('kind', 'mindchains', 'who', 'webhook')),
  'value', json_object('delivery', json_object('id', id, 'names', json('[]'), 'does', '')))
FROM (SELECT d.id, 'delivery:' || json_extract(d.doc, '$.label') AS key, d.received_at AS at, json_extract(d.doc, '$.receivedAt') AS received,
    COALESCE((SELECT MAX(r.rev) FROM readings AS r WHERE r.key = 'delivery:' || json_extract(d.doc, '$.label')), 0)
      + ROW_NUMBER() OVER (PARTITION BY json_extract(d.doc, '$.label') ORDER BY d.received_at, d.id) AS rev
  FROM deliveries AS d WHERE NOT EXISTS (SELECT 1 FROM readings AS r WHERE json_extract(r.doc, '$.value.delivery.id') = d.id));
DELETE FROM readings WHERE json_extract(doc, '$.value.delivery.id') IS NOT NULL
  AND rev < (SELECT MAX(newer.rev) FROM readings AS newer WHERE newer.key = readings.key AND json_extract(newer.doc, '$.value.delivery.id') = json_extract(readings.doc, '$.value.delivery.id'));
CREATE TABLE placements (id TEXT PRIMARY KEY, dedupe TEXT NOT NULL UNIQUE, signal TEXT NOT NULL, reading_rev INTEGER NOT NULL, status TEXT NOT NULL, at INTEGER NOT NULL, doc TEXT NOT NULL);
CREATE INDEX placements_status ON placements(status, at);
CREATE UNIQUE INDEX placements_reading ON placements(signal, reading_rev);
INSERT INTO placements(id, dedupe, signal, reading_rev, status, at, doc)
SELECT d.id, d.key, json_extract(d.doc, '$.label'), r.rev, d.status, d.received_at,
  json_set(json_remove(d.doc, '$.key', '$.label', '$.method', '$.query', '$.header', '$.body', '$.bodyCut', '$.receivedAt'),
    '$.signal', json_extract(d.doc, '$.label'), '$.readingRev', r.rev, '$.dedupe', d.key, '$.at', json_extract(d.doc, '$.receivedAt'))
FROM deliveries AS d JOIN readings AS r ON json_extract(r.doc, '$.value.delivery.id') = d.id;
UPDATE readings SET doc = json_set(readings.doc, '$.value', json_object(
    'event', json_patch(json_object('via', 'door', 'body', COALESCE(json_extract(d.doc, '$.body'), '')),
      json_object('header', json(json_extract(d.doc, '$.header')), 'query', NULLIF(json_extract(d.doc, '$.query'), ''),
        'bodyCut', CASE WHEN json_extract(d.doc, '$.bodyCut') THEN json('true') END)),
    'found', json_object('names', json(COALESCE(json_extract(readings.doc, '$.value.delivery.names'), '[]')), 'does', COALESCE(json_extract(readings.doc, '$.value.delivery.does'), ''))))
FROM deliveries AS d WHERE json_extract(readings.doc, '$.value.delivery.id') = d.id;
UPDATE readings SET doc = json_set(doc, '$.value', json_object('event', json_object('via', 'door', 'body', ''),
    'found', json_object('names', json(COALESCE(json_extract(doc, '$.value.delivery.names'), '[]')), 'does', COALESCE(json_extract(doc, '$.value.delivery.does'), ''))))
WHERE substr(key, 1, 9) = 'delivery:' AND json_type(doc, '$.value.delivery') IS NOT NULL;
DROP INDEX readings_delivery;
DROP TABLE deliveries;
UPDATE readings SET key = 'event:' || substr(key, 10),
  doc = json_set(doc, '$.key', 'event:' || substr(key, 10), '$.cause.actor.who', CASE WHEN json_extract(doc, '$.cause.actor.who') = 'webhook' THEN 'door' ELSE json_extract(doc, '$.cause.actor.who') END)
WHERE substr(key, 1, 9) = 'delivery:';
UPDATE subscriptions SET key = 'event:' || substr(key, 10), doc = json_set(doc, '$.key', 'event:' || substr(key, 10)) WHERE substr(key, 1, 9) = 'delivery:';
UPDATE subscriptions SET doc = json_set(doc, '$.expr', replace(json_extract(doc, '$.expr'), 'value.delivery.', 'value.found.')) WHERE instr(json_extract(doc, '$.expr'), 'value.delivery.') > 0;
UPDATE subscriptions SET owner_kind = 'event_signal', doc = json_set(doc, '$.owner.kind', 'event_signal') WHERE owner_kind = 'webhook';
UPDATE subscriptions SET id = 'sub_es_' || substr(id, 8), doc = json_set(doc, '$.id', 'sub_es_' || substr(id, 8)) WHERE substr(id, 1, 7) = 'sub_wh_';
UPDATE subscriptions SET doc = json_set(doc, '$.by.who', 'door') WHERE json_extract(doc, '$.by.kind') = 'mindchains' AND json_extract(doc, '$.by.who') = 'webhook';
UPDATE firings SET key = 'event:' || substr(key, 10), doc = json_set(doc, '$.key', 'event:' || substr(key, 10)) WHERE substr(key, 1, 9) = 'delivery:';
UPDATE firings SET owner_kind = 'event_signal', doc = json_set(doc, '$.owner.kind', 'event_signal') WHERE owner_kind = 'webhook';
UPDATE firings SET subscription_id = 'sub_es_' || substr(subscription_id, 8), doc = json_set(doc, '$.subscriptionId', 'sub_es_' || substr(subscription_id, 8)) WHERE substr(subscription_id, 1, 7) = 'sub_wh_';
ALTER TABLE webhooks RENAME TO event_signals;
UPDATE chains SET doc = json_set(json_remove(doc, '$.deliveryId'), '$.placementId', json_extract(doc, '$.deliveryId')) WHERE json_type(doc, '$.deliveryId') IS NOT NULL;
UPDATE chains SET doc = json_set(doc, '$.origin', 'event') WHERE json_extract(doc, '$.origin') = 'webhook';
UPDATE decisions SET doc = json_set(doc, '$.kind', 'event') WHERE json_extract(doc, '$.kind') = 'webhook';
UPDATE decisions SET doc = json_set(doc, '$.by.who', 'door') WHERE json_extract(doc, '$.by.kind') = 'mindchains' AND json_extract(doc, '$.by.who') = 'webhook';
