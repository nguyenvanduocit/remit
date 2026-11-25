-- What fills a card's session's context, as Claude Code counts it; the newest replaces the one before.
CREATE TABLE session_contexts (card_id TEXT PRIMARY KEY, home TEXT NOT NULL, doc TEXT NOT NULL);
CREATE INDEX session_contexts_home ON session_contexts(home);
