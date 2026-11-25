
UPDATE chains SET doc = json_set(doc, '$.capabilities', json('["history","issues","propose"]'))
  WHERE json_extract(doc, '$.capabilities') IS NULL AND json_extract(doc, '$.origin') = 'scout';
UPDATE chains SET doc = json_set(doc, '$.capabilities', json('["history","playbooks"]'))
  WHERE json_extract(doc, '$.capabilities') IS NULL AND json_extract(doc, '$.playbookId') = 'playbook-author';
