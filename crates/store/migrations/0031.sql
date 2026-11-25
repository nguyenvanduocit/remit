
UPDATE deliveries SET doc = json_set(json_remove(doc, '$.settled.when', '$.settled.does'), '$.settled.precondition',
  json_object('when', json_extract(doc, '$.settled.when'), 'does', json_extract(doc, '$.settled.does')))
WHERE json_type(doc, '$.settled.when') IS NOT NULL;
