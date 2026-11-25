-- What changes with each turn leaves the memory's document, and what is made from a memory is keyed so it is found
-- without a scan (ADR-0117). Every statement may run again over what it made and leaves it as it was.
--
-- `n` is each memory's own key, an alias of the rowid that VACUUM keeps, which its words are keyed by: removing a memory's
-- words is a lookup, not a scan of the index. Rows keep their order.
CREATE TABLE IF NOT EXISTS memories_next (
    n INTEGER PRIMARY KEY,
    id TEXT NOT NULL UNIQUE,
    holder TEXT NOT NULL,
    template TEXT NOT NULL,
    project TEXT NOT NULL,
    scope TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    doc TEXT NOT NULL
);
INSERT OR IGNORE INTO memories_next(id, holder, template, project, scope, status, created_at, updated_at, doc)
SELECT id, holder, template, project, scope, status, created_at, updated_at, doc FROM memories ORDER BY created_at, id;
DROP TABLE memories;
ALTER TABLE memories_next RENAME TO memories;
CREATE INDEX IF NOT EXISTS memories_holder ON memories(holder, status);
CREATE INDEX IF NOT EXISTS memories_project ON memories(project, scope, status);
CREATE INDEX IF NOT EXISTS memories_template ON memories(template, scope, status);

-- A memory's use (how many turns were given it and when last, how often it helped or misled) changes with every turn and
-- rating, while its document changes only when the librarian writes it: it is counted here by UPDATE, so a rating never
-- rewrites the document, its words or its edges, a write of the document never loses a count, and a forget the owner was
-- shown still applies after the memory was recalled. A memory with no row was never used.
CREATE TABLE IF NOT EXISTS memory_use (
    memory_id TEXT PRIMARY KEY,
    recalled INTEGER NOT NULL DEFAULT 0,
    helped INTEGER NOT NULL DEFAULT 0,
    misled INTEGER NOT NULL DEFAULT 0,
    last_recalled_at INTEGER
);
INSERT OR IGNORE INTO memory_use(memory_id, recalled, helped, misled, last_recalled_at)
SELECT id,
    max(coalesce(json_extract(doc, '$.recalled'), 0), 0),
    max(coalesce(json_extract(doc, '$.helped'), 0), 0),
    max(coalesce(json_extract(doc, '$.misled'), 0), 0),
    CASE WHEN json_extract(doc, '$.lastRecalledAt') IS NULL THEN NULL
         ELSE CAST(round((julianday(json_extract(doc, '$.lastRecalledAt')) - 2440587.5) * 86400000) AS INTEGER) END
FROM memories
WHERE coalesce(json_extract(doc, '$.recalled'), 0) > 0 OR coalesce(json_extract(doc, '$.helped'), 0) > 0
    OR coalesce(json_extract(doc, '$.misled'), 0) > 0 OR json_extract(doc, '$.lastRecalledAt') IS NOT NULL;
UPDATE memories SET doc = json_remove(doc, '$.recalled', '$.helped', '$.misled', '$.lastRecalledAt')
WHERE json_type(doc, '$.recalled') IS NOT NULL OR json_type(doc, '$.helped') IS NOT NULL
    OR json_type(doc, '$.misled') IS NOT NULL OR json_type(doc, '$.lastRecalledAt') IS NOT NULL;

-- Where each memory was learned, for its screen to link to: the chain and card of the teaching, the chain's title (the
-- work its passage names), and its source in words as a teaching recorded today says it (teach_memory.rs). A source naming a card the store still holds gives both, a
-- teaching that wrote the memory gives them otherwise; a source of neither stays as it is.
WITH src AS (
    SELECT m.id,
        coalesce(json_extract(m.doc, '$.source'), '') AS source,
        coalesce(json_extract(m.doc, '$.teacher'), '') AS teacher,
        CASE WHEN json_extract(m.doc, '$.source') LIKE 'card:%' THEN
            substr(json_extract(m.doc, '$.source'), 6, instr(substr(json_extract(m.doc, '$.source'), 6) || ' ', ' ') - 1)
        END AS card_id,
        (SELECT t.doc FROM teachings t, json_each(t.doc, '$.written') w WHERE w.value = m.id
            ORDER BY t.created_at, t.id LIMIT 1) AS taught
    FROM memories m WHERE json_type(m.doc, '$.sourceLabel') IS NULL
),
place AS (
    SELECT src.id,
        coalesce(c.id, json_extract(src.taught, '$.card'), '') AS card,
        coalesce(c.home, json_extract(src.taught, '$.chain'), '') AS chain,
        CASE
            WHEN c.id IS NOT NULL AND src.source LIKE 'card:% accepted' THEN 'an accepted card: ' || json_extract(c.doc, '$.name')
            WHEN c.id IS NOT NULL AND src.source LIKE 'card:% sent back' AND src.teacher != '' THEN
                'a card ' || src.teacher || ' sent back: ' || json_extract(c.doc, '$.name')
            WHEN c.id IS NOT NULL AND src.source LIKE 'card:% sent back' THEN 'a card sent back: ' || json_extract(c.doc, '$.name')
            WHEN c.id IS NOT NULL AND src.source LIKE 'card:% steered by % (event %' THEN
                substr(src.source, instr(src.source, ' steered by ') + 12,
                    instr(src.source, ' (event ') - instr(src.source, ' steered by ') - 12)
                || ' steered ' || json_extract(c.doc, '$.name')
            WHEN c.id IS NOT NULL AND src.teacher != '' THEN 'what ' || src.teacher || ' told ' || json_extract(c.doc, '$.name')
            WHEN c.id IS NOT NULL THEN 'what was said to ' || json_extract(c.doc, '$.name')
            WHEN src.source LIKE 'decision:%' THEN
                'the owner overturned decision ' || substr(src.source, 10, instr(src.source || ' ', ' ') - 10)
            WHEN src.source LIKE 'mr:%' THEN
                'the owner''s changes to merge request ' || substr(src.source, 4, instr(src.source || ' ', ' ') - 4)
            WHEN src.source LIKE 'review:%' THEN 'a review of memories (' || substr(src.source, 8) || ')'
            ELSE src.source
        END AS label
    FROM src LEFT JOIN cards c ON c.id = src.card_id
)
UPDATE memories SET doc = json_set(memories.doc, '$.chain', place.chain, '$.card', place.card, '$.sourceLabel', place.label,
    '$.work', coalesce(nullif(json_extract(memories.doc, '$.work'), ''),
        (SELECT json_extract(ch.doc, '$.title') FROM chains ch WHERE ch.id = place.chain), ''))
FROM place WHERE memories.id = place.id;

-- A waiting teaching says its source in words too, which the memories it writes carry.
UPDATE teachings SET doc = json_set(teachings.doc, '$.sourceLabel', CASE
        WHEN json_extract(teachings.doc, '$.source') LIKE 'review:%' THEN
            'a review of memories (' || substr(json_extract(teachings.doc, '$.source'), 8) || ')'
        WHEN c.id IS NULL THEN coalesce(json_extract(teachings.doc, '$.source'), '')
        WHEN json_extract(teachings.doc, '$.source') LIKE 'card:% accepted' THEN 'an accepted card: ' || json_extract(c.doc, '$.name')
        WHEN json_extract(teachings.doc, '$.source') LIKE 'card:% sent back' THEN
            'a card ' || coalesce(nullif(json_extract(teachings.doc, '$.teacher'), ''), 'a judge') || ' sent back: ' || json_extract(c.doc, '$.name')
        ELSE 'what ' || coalesce(nullif(json_extract(teachings.doc, '$.teacher'), ''), 'someone') || ' told ' || json_extract(c.doc, '$.name')
    END)
FROM (SELECT id, home, doc FROM cards) c
WHERE c.id = json_extract(teachings.doc, '$.card') AND json_type(teachings.doc, '$.sourceLabel') IS NULL;

-- The words of each memory, keyed by its row (memories.n): only the columns searched, which bm25 weighs in this order.
DROP TABLE IF EXISTS memory_text;
CREATE VIRTUAL TABLE memory_text USING fts5(
    title,
    body,
    evidence,
    about,
    tokenize = 'unicode61 remove_diacritics 2'
);
INSERT INTO memory_text(rowid, title, body, evidence, about)
SELECT n, coalesce(json_extract(doc, '$.title'), ''), coalesce(json_extract(doc, '$.body'), ''),
    coalesce(json_extract(doc, '$.evidence'), ''),
    coalesce((SELECT group_concat(value, ' ') FROM json_each(doc, '$.about')), '')
FROM memories;

-- The meaning of each memory, with a hash of the passage it was made from (`store::passage_hash`): a vector made before
-- the memory changed is never stored over the change, and one whose passage is not the memory's is made again. The
-- vectors made before go: a passage now carries its kind, its project and the work that taught it, so every memory is
-- embedded again by the job.
DROP TABLE IF EXISTS memory_vectors;
CREATE TABLE IF NOT EXISTS memory_embeddings (
    memory_id TEXT NOT NULL,
    model TEXT NOT NULL,
    passage_hash TEXT NOT NULL,
    vec BLOB NOT NULL,
    PRIMARY KEY (memory_id, model)
);

-- A memory the embedding model refused (a 4xx for that passage): left out of the job for a day, or until its passage
-- changes, so one memory never holds back the others.
CREATE TABLE IF NOT EXISTS memory_unembedded (
    memory_id TEXT NOT NULL,
    model TEXT NOT NULL,
    passage_hash TEXT NOT NULL,
    at INTEGER NOT NULL,
    why TEXT NOT NULL,
    PRIMARY KEY (memory_id, model)
);

-- A teaching is read by its holder (a memory screen) and by its card (an overturn): columns with an index each.
CREATE TABLE IF NOT EXISTS teachings_next (
    id TEXT PRIMARY KEY,
    state TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    holder TEXT NOT NULL DEFAULT '',
    card TEXT NOT NULL DEFAULT '',
    doc TEXT NOT NULL
);
INSERT OR IGNORE INTO teachings_next(id, state, created_at, holder, card, doc)
SELECT id, state, created_at, coalesce(json_extract(doc, '$.holder'), ''), coalesce(json_extract(doc, '$.card'), ''), doc
FROM teachings;
DROP TABLE teachings;
ALTER TABLE teachings_next RENAME TO teachings;
CREATE INDEX IF NOT EXISTS teachings_state ON teachings(state, created_at);
CREATE INDEX IF NOT EXISTS teachings_holder ON teachings(holder, created_at);
CREATE INDEX IF NOT EXISTS teachings_card ON teachings(card, created_at);
