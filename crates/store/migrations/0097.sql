
CREATE TABLE IF NOT EXISTS workflows (
  id TEXT PRIMARY KEY, home TEXT NOT NULL, created_at INTEGER NOT NULL, doc TEXT NOT NULL,
  status TEXT GENERATED ALWAYS AS (json_extract(doc, '$.status')) VIRTUAL);
CREATE INDEX IF NOT EXISTS workflows_home ON workflows(home, status);
CREATE INDEX IF NOT EXISTS workflows_status ON workflows(status);
CREATE TABLE IF NOT EXISTS workflow_steps (
  workflow_id TEXT NOT NULL, seq INTEGER NOT NULL, home TEXT NOT NULL, doc TEXT NOT NULL,
  status TEXT GENERATED ALWAYS AS (json_extract(doc, '$.status')) VIRTUAL,
  card_id TEXT GENERATED ALWAYS AS (json_extract(doc, '$.card')) VIRTUAL,
  PRIMARY KEY(workflow_id, seq));
CREATE INDEX IF NOT EXISTS workflow_steps_card ON workflow_steps(card_id) WHERE card_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS workflow_steps_home ON workflow_steps(home);
