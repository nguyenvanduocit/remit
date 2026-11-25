
CREATE INDEX events_running ON events(home, kind, id) WHERE status='running';
