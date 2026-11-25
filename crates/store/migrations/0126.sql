-- Judgments (ADR-0118): one decider's answer to one typed question about one state, one row each. The asking it answers
-- (shared with the shadows asked beside it), its question and its decider are columns; what the code made of it and the
-- outcome its pick proved are columns generated from its document, which a question's standings and tallies read by.
-- Safe to run again.
CREATE TABLE IF NOT EXISTS judgments (id TEXT PRIMARY KEY, asking TEXT NOT NULL, question TEXT NOT NULL, decider TEXT NOT NULL, created_at INTEGER NOT NULL, doc TEXT NOT NULL, used TEXT GENERATED ALWAYS AS (json_extract(doc, '$.used')) VIRTUAL, proved TEXT GENERATED ALWAYS AS (json_extract(doc, '$.outcome.proved')) VIRTUAL);
CREATE INDEX IF NOT EXISTS judgments_asking ON judgments(asking);
CREATE INDEX IF NOT EXISTS judgments_scored ON judgments(question, decider, proved);
CREATE INDEX IF NOT EXISTS judgments_used ON judgments(question, used, created_at);
