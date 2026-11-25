-- The bytes of the images a message holds, stored once by the id the channel made (`model::ImageRef` names them from a
-- draft and from the message event it becomes). An image goes with its home.
CREATE TABLE IF NOT EXISTS images (id TEXT PRIMARY KEY, home TEXT NOT NULL, media_type TEXT NOT NULL, bytes BLOB NOT NULL, at INTEGER NOT NULL);
CREATE INDEX IF NOT EXISTS images_home ON images(home);
