
UPDATE cards SET doc = json_set(json_remove(doc, '$.role'), '$.agent', json_extract(doc, '$.role')) WHERE json_type(doc, '$.role') IS NOT NULL;
UPDATE playbooks SET doc = json_set(doc, '$.seeds', json((
  SELECT json_group_array(json(json_set(json_remove(value, '$.role'), '$.agent', json_extract(value, '$.role')))) FROM json_each(playbooks.doc, '$.seeds'))))
WHERE json_type(doc, '$.seeds') = 'array' AND json_array_length(doc, '$.seeds') > 0;
