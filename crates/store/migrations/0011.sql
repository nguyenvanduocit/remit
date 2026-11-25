
ALTER TABLE events ADD COLUMN by_kind TEXT NOT NULL DEFAULT '';
ALTER TABLE events ADD COLUMN by_who TEXT NOT NULL DEFAULT '';
ALTER TABLE events ADD COLUMN by_via TEXT NOT NULL DEFAULT '';
UPDATE events SET
  by_kind = CASE WHEN from_who = 'owner' THEN 'owner' WHEN from_who = 'system' OR from_who LIKE 'mindchains:%' THEN 'mindchains' ELSE 'agent' END,
  by_who = CASE WHEN from_who IN ('owner', 'system') THEN '' WHEN from_who LIKE 'mindchains:%' THEN substr(from_who, 12) ELSE from_who END
WHERE from_who != '';
UPDATE events SET data = json_set(data, '$.by', json(CASE WHEN json_extract(data, '$.by') = 'owner' THEN json_object('kind', 'owner')
    WHEN json_extract(data, '$.by') = 'system' THEN json_object('kind', 'mindchains')
    WHEN json_extract(data, '$.by') LIKE 'mindchains:%' THEN json_object('kind', 'mindchains', 'who', substr(json_extract(data, '$.by'), 12))
    ELSE json_object('kind', 'agent', 'who', json_extract(data, '$.by')) END))
WHERE json_type(data, '$.by') = 'text';
ALTER TABLE events DROP COLUMN from_who;
UPDATE inbox SET doc = json_set(json_remove(doc, '$.retry.from'), '$.retry.by', json(CASE WHEN json_extract(doc, '$.retry.from') = 'owner' THEN json_object('kind', 'owner')
    WHEN json_extract(doc, '$.retry.from') = 'system' THEN json_object('kind', 'mindchains')
    WHEN json_extract(doc, '$.retry.from') LIKE 'mindchains:%' THEN json_object('kind', 'mindchains', 'who', substr(json_extract(doc, '$.retry.from'), 12))
    ELSE json_object('kind', 'agent', 'who', json_extract(doc, '$.retry.from')) END))
WHERE json_type(doc, '$.retry.from') = 'text';
UPDATE review_threads SET doc = json_set(doc, '$.messages', json((
  SELECT json_group_array(json(json_set(m.value, '$.by', json_object('kind', json_extract(m.value, '$.by')))) ORDER BY m.key) FROM json_each(doc, '$.messages') AS m)))
WHERE json_type(doc, '$.messages') = 'array' AND json_array_length(doc, '$.messages') > 0;
