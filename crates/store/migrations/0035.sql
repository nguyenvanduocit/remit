
ALTER TABLE decisions ADD COLUMN kind TEXT GENERATED ALWAYS AS (json_extract(doc, '$.kind')) VIRTUAL;
CREATE INDEX decisions_kind ON decisions(kind, created_at);
