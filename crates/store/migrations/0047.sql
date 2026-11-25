
DROP TABLE IF EXISTS mcp_secrets;
DROP TABLE IF EXISTS mcp_servers;
CREATE TABLE source_connections (source TEXT NOT NULL, name TEXT NOT NULL, description TEXT NOT NULL DEFAULT '', PRIMARY KEY(source, name));
CREATE TABLE source_values (source TEXT NOT NULL, connection TEXT NOT NULL, name TEXT NOT NULL, plain TEXT NOT NULL DEFAULT '', sealed BLOB, key_id TEXT NOT NULL DEFAULT '', PRIMARY KEY(source, connection, name));
UPDATE agents SET doc = json_remove(doc, '$.mcp') WHERE json_type(doc, '$.mcp') IS NOT NULL;
UPDATE cards SET doc = json_remove(doc, '$.loadout.mcp') WHERE json_type(doc, '$.loadout.mcp') IS NOT NULL;
DELETE FROM changes WHERE kind = 'mcp';
DELETE FROM inbox WHERE json_extract(doc, '$.kind') = 'credential';
