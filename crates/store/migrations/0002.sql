
CREATE INDEX events_chain ON events(chain_id, id);
CREATE INDEX events_queued ON events(id) WHERE kind='prompt' AND status='queued';
CREATE INDEX inbox_chain ON inbox(chain_id);
