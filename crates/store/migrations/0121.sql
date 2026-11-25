-- Key results are an entity of their own (ADR-0115): agents refine a key result into key results without bound, so they
-- leave the objective's document for a table, keyed by their objective and their id. `doc` is what was written of it;
-- `parent` the key result it refines (none for the owner's); `place` its place among the owner's (0 for one that refines
-- another); `lifted` the last lift of its progress rail, rail state kept apart from the commitment so that writing the
-- owner's key results again never drops it. Each objective's key results move there with no parent, keeping their ids
-- and their order; the objective's document no longer holds them. A second run inserts nothing and finds no document
-- that still holds them.
CREATE TABLE IF NOT EXISTS key_results (
    objective TEXT NOT NULL,
    id TEXT NOT NULL,
    parent TEXT,
    place INTEGER NOT NULL DEFAULT 0,
    lifted TEXT,
    doc TEXT NOT NULL,
    PRIMARY KEY (objective, id)
);
CREATE INDEX IF NOT EXISTS key_results_parent ON key_results(objective, parent);

INSERT INTO key_results(objective, id, parent, place, doc)
SELECT o.id, json_extract(kr.value, '$.id'), NULL, kr.key, kr.value
FROM objectives o, json_each(o.doc, '$.keyResults') kr
WHERE json_type(o.doc, '$.keyResults') = 'array'
ON CONFLICT(objective, id) DO NOTHING;

UPDATE objectives SET doc = json_remove(doc, '$.keyResults') WHERE json_type(doc, '$.keyResults') IS NOT NULL;
