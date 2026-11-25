-- ADR-0082: an edge's label names the act in the owner's words. Edges saved before carry the route kind as their label.
UPDATE edges SET doc = json_set(doc, '$.label', 'hand-off') WHERE json_extract(doc, '$.label') = 'handoff';
UPDATE edges SET doc = json_set(doc, '$.label', 'sent back') WHERE json_extract(doc, '$.label') = 'feedback';
UPDATE edges SET doc = json_set(doc, '$.label', 'asked') WHERE json_extract(doc, '$.label') = 'question';
