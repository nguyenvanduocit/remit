-- What Remit started a card's session with, recorded as it started it; the newest start replaces the one before.
CREATE TABLE session_launches (card_id TEXT PRIMARY KEY, home TEXT NOT NULL, doc TEXT NOT NULL);
CREATE INDEX session_launches_home ON session_launches(home);
