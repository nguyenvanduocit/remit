
UPDATE cards SET doc = json_set(doc, '$.loadout', json(COALESCE(
  (SELECT json_object(
      'instructions', COALESCE(json_extract(a.doc, '$.instructions'), ''),
      'tools', json(COALESCE(json_extract(a.doc, '$.tools'), 'null')),
      'skills', json(COALESCE(json_extract(a.doc, '$.skills'), '[]')),
      'mcp', json(COALESCE(json_extract(a.doc, '$.mcp'), '[]')))
   FROM agents a WHERE a.id = json_extract(cards.doc, '$.agent')),
  '{"instructions":"","tools":null,"skills":[],"mcp":[]}')))
WHERE json_extract(doc, '$.origin') IN ('hired', 'seed') AND json_type(doc, '$.loadout') IS NULL;
