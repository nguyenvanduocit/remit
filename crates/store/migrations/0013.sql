
CREATE TABLE IF NOT EXISTS approvals (id TEXT PRIMARY KEY, chain_id TEXT NOT NULL, created_at INTEGER NOT NULL, doc TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS approvals_chain ON approvals(chain_id, created_at);
ALTER TABLE approvals RENAME COLUMN created_at TO at;
INSERT INTO approvals(id, chain_id, at, doc)
  SELECT 'ap_' || c.id || '_' || a.key, c.id, CAST(round((julianday(json_extract(a.value, '$.at')) - 2440587.5) * 86400000) AS INTEGER),
    json_set(a.value, '$.id', 'ap_' || c.id || '_' || a.key, '$.chainId', c.id)
  FROM chains AS c, json_each(c.doc, '$.approvals') AS a;
UPDATE chains SET doc = json_remove(doc, '$.approvals', '$.goalChecks');
