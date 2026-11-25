ALTER TABLE images RENAME TO attachments;
ALTER TABLE attachments ADD COLUMN name TEXT NOT NULL DEFAULT '';
DROP INDEX images_home;
CREATE INDEX attachments_home ON attachments(home);

UPDATE drafts SET doc = json_remove(json_set(doc, '$.attachments', doc -> '$.images'), '$.images')
WHERE json_type(doc, '$.images') = 'array';

WITH prompt_data AS MATERIALIZED (
    SELECT id, CASE typeof(data) WHEN 'blob' THEN unpack_event(data) ELSE data END AS doc
    FROM events WHERE kind = 'prompt' AND data IS NOT NULL
)
UPDATE events
SET data = json_remove(json_set(prompt_data.doc, '$.attachments', prompt_data.doc -> '$.images'), '$.images')
FROM prompt_data
WHERE events.id = prompt_data.id
  AND json_type(CASE WHEN json_valid(prompt_data.doc) THEN prompt_data.doc END, '$.images') = 'array';
