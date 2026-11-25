
CREATE TABLE IF NOT EXISTS worker_keys (id TEXT PRIMARY KEY, employee TEXT NOT NULL, device TEXT NOT NULL, sealed BLOB NOT NULL, key_id TEXT NOT NULL DEFAULT '', doc TEXT NOT NULL, UNIQUE(employee, device));
CREATE TABLE IF NOT EXISTS worker_signins (id TEXT PRIMARY KEY, addr TEXT NOT NULL, status TEXT NOT NULL, device_code BLOB NOT NULL, device_code_key_id TEXT NOT NULL DEFAULT '', key BLOB, key_key_id TEXT NOT NULL DEFAULT '', doc TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS worker_signins_open ON worker_signins(status, addr);
