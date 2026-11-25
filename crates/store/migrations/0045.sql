
CREATE TABLE subscriptions (id TEXT PRIMARY KEY, owner_kind TEXT NOT NULL, owner_id TEXT NOT NULL, key TEXT NOT NULL, paused INTEGER NOT NULL, due_at INTEGER, seen_rev INTEGER NOT NULL, doc TEXT NOT NULL);
CREATE INDEX subscriptions_key ON subscriptions(key, paused);
CREATE INDEX subscriptions_owner ON subscriptions(owner_kind, owner_id);
CREATE INDEX subscriptions_due ON subscriptions(due_at) WHERE paused = 0 AND due_at IS NOT NULL;
CREATE TABLE readings (key TEXT NOT NULL, rev INTEGER NOT NULL, at INTEGER NOT NULL, doc TEXT NOT NULL, PRIMARY KEY(key, rev));
CREATE TABLE firings (subscription_id TEXT NOT NULL, key TEXT NOT NULL, rev INTEGER NOT NULL, at INTEGER NOT NULL, doc TEXT NOT NULL, PRIMARY KEY(subscription_id, key, rev));
CREATE INDEX firings_at ON firings(at);
