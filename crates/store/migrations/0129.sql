-- An agent keeps its loadout under `loadout`, as a card and an employee do (ADR-0107 step 4d): the link's encoding carries
-- no flattened keys, and an agent crosses to every channel as itself. The keys an earlier build wrote among the agent's
-- own (`instructions`, `tools`, `skills`) move there; `tools` stays absent where it was absent or null (every tool).
-- A document that has its `loadout` already is left as it is.
UPDATE agents SET doc = json_remove(json_set(doc, '$.loadout', CASE json_type(doc, '$.tools')
    WHEN 'array' THEN json_object(
      'instructions', coalesce(json_extract(doc, '$.instructions'), ''),
      'tools', json_extract(doc, '$.tools'),
      'skills', CASE json_type(doc, '$.skills') WHEN 'array' THEN json_extract(doc, '$.skills') ELSE json('[]') END)
    ELSE json_object(
      'instructions', coalesce(json_extract(doc, '$.instructions'), ''),
      'skills', CASE json_type(doc, '$.skills') WHEN 'array' THEN json_extract(doc, '$.skills') ELSE json('[]') END)
  END), '$.instructions', '$.tools', '$.skills')
WHERE json_type(doc, '$.loadout') IS NULL;

-- A change item for an agent keeps the agent it would write as JSON text (`$.change.record`), which its Apply reads as an
-- agent: the same move, inside that text.
UPDATE inbox SET doc = json_set(doc, '$.change.record', '' || json_remove(json_set(json_extract(doc, '$.change.record'), '$.loadout',
  CASE json_type(json_extract(doc, '$.change.record'), '$.tools')
    WHEN 'array' THEN json_object(
      'instructions', coalesce(json_extract(json_extract(doc, '$.change.record'), '$.instructions'), ''),
      'tools', json_extract(json_extract(doc, '$.change.record'), '$.tools'),
      'skills', CASE json_type(json_extract(doc, '$.change.record'), '$.skills')
        WHEN 'array' THEN json_extract(json_extract(doc, '$.change.record'), '$.skills') ELSE json('[]') END)
    ELSE json_object(
      'instructions', coalesce(json_extract(json_extract(doc, '$.change.record'), '$.instructions'), ''),
      'skills', CASE json_type(json_extract(doc, '$.change.record'), '$.skills')
        WHEN 'array' THEN json_extract(json_extract(doc, '$.change.record'), '$.skills') ELSE json('[]') END)
  END), '$.instructions', '$.tools', '$.skills'))
WHERE json_extract(doc, '$.change.kind') = 'agent'
  AND json_valid(json_extract(doc, '$.change.record'))
  AND json_type(json_extract(doc, '$.change.record'), '$.loadout') IS NULL;
