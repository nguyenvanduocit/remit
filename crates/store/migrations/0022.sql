
CREATE TABLE agents (id TEXT PRIMARY KEY, doc TEXT NOT NULL);
CREATE TABLE skills (name TEXT PRIMARY KEY, doc TEXT NOT NULL);
CREATE TABLE mcp_servers (name TEXT PRIMARY KEY, doc TEXT NOT NULL);
CREATE TABLE mcp_secrets (server TEXT NOT NULL, slot TEXT NOT NULL, sealed BLOB NOT NULL, PRIMARY KEY(server, slot));
-- A card signs the gate its role text named, as the code read it until now (a role containing "review"; qc or qa, or a
-- role starting with them, or containing "tester"), and the goal judge signs the goal: stored once, so no code reads role
-- text for it again.
UPDATE cards SET doc = json_set(doc, '$.signs', CASE
  WHEN json_extract(doc, '$.origin') = 'judge' AND json_extract(doc, '$.role') = 'judge' THEN 'goal'
  WHEN lower(trim(json_extract(doc, '$.role'))) LIKE '%review%' THEN 'review'
  ELSE 'qc' END)
WHERE (json_extract(doc, '$.origin') = 'judge' AND json_extract(doc, '$.role') = 'judge')
  OR lower(trim(json_extract(doc, '$.role'))) LIKE '%review%'
  OR lower(trim(json_extract(doc, '$.role'))) IN ('qc', 'qa')
  OR lower(trim(json_extract(doc, '$.role'))) LIKE 'qc%'
  OR lower(trim(json_extract(doc, '$.role'))) LIKE 'qa%'
  OR lower(trim(json_extract(doc, '$.role'))) LIKE '%tester%';
