
CREATE TABLE chain_grants_new (chain_id TEXT NOT NULL, source TEXT NOT NULL, connection TEXT NOT NULL, effect TEXT NOT NULL CHECK (effect IN ('read', 'reversible', 'compensable', 'irreversible')), granted_by TEXT NOT NULL, granted_at INTEGER NOT NULL, PRIMARY KEY(chain_id, source, connection));
INSERT INTO chain_grants_new SELECT chain_id, source, connection, CASE effect WHEN 'write' THEN 'irreversible' ELSE effect END, granted_by, granted_at FROM chain_grants;
DROP TABLE chain_grants;
ALTER TABLE chain_grants_new RENAME TO chain_grants;
UPDATE inbox SET doc = json_set(doc, '$.change.record', json_set(json_extract(doc, '$.change.record'), '$.effect', 'irreversible'))
WHERE json_extract(doc, '$.change.kind') = 'grant' AND json_extract(json_extract(doc, '$.change.record'), '$.effect') = 'write';
