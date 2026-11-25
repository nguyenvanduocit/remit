
CREATE TABLE feedback (id TEXT PRIMARY KEY, card_id TEXT NOT NULL, created_at INTEGER NOT NULL, doc TEXT NOT NULL);
CREATE INDEX feedback_card ON feedback(card_id);
CREATE INDEX feedback_created ON feedback(created_at);
