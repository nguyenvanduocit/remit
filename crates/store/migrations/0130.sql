-- Where each event shape starts (ADR-0107 step 7e). From these ids on, an owner's message stores his words alone, marked
-- as his in its data (`ownerWords`), and a stopped background task keeps its cut in its data (`cut`). An event below its
-- shape's id was written in the older shape, and the store's event reader alone converts it as it reads it: the line
-- that headed the owner's words taken off, the cut read from the task's words. Events are never rewritten for it.
-- AUTOINCREMENT keeps an id once given, so the next event's id is past the sequence; INSERT OR IGNORE leaves a shape
-- already recorded where it is.
CREATE TABLE IF NOT EXISTS event_shapes (shape TEXT PRIMARY KEY, from_id INTEGER NOT NULL);
INSERT OR IGNORE INTO event_shapes(shape, from_id)
  SELECT 'owner_words', coalesce((SELECT seq FROM sqlite_sequence WHERE name = 'events'), 0) + 1;
INSERT OR IGNORE INTO event_shapes(shape, from_id)
  SELECT 'task_cut', coalesce((SELECT seq FROM sqlite_sequence WHERE name = 'events'), 0) + 1;
