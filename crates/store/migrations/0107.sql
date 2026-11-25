-- Second opinions (ADR-0103): a fresh judge re-decides a random share of the Assistant's answers for the owner, one row
-- each; its judge's card is looked up when the judge records, and the newest are read for the coach's figures.
CREATE TABLE IF NOT EXISTS second_opinions (id TEXT PRIMARY KEY, chain TEXT NOT NULL, judge TEXT NOT NULL, created_at INTEGER NOT NULL, doc TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS second_opinions_judge ON second_opinions(judge);
CREATE INDEX IF NOT EXISTS second_opinions_created ON second_opinions(created_at);
