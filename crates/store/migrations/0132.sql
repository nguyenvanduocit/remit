-- A chain's work on a repository says where it lives (ADR-0137): a worktree of a clone, or a folder worked in place, and a
-- landing's queue is none for a version of a folder, which has no queue. Every stored repository of a chain, and of a
-- deleted chain's cleanup, is a worktree: its host, project, path, branch and base move into `place`. Each statement
-- changes only what still holds the earlier shape.
UPDATE chains SET doc = json_set(doc, '$.repos', json((
    SELECT json_group_array(json(w)) FROM (
        SELECT json_object(
            'repo', coalesce(json_extract(r.value, '$.repo'), ''),
            'place', json_object('kind', 'worktree',
                'project', coalesce(json_extract(r.value, '$.project'), ''),
                'host', coalesce(json_extract(r.value, '$.host'), ''),
                'path', coalesce(json_extract(r.value, '$.path'), ''),
                'branch', coalesce(json_extract(r.value, '$.branch'), ''),
                'base', coalesce(json_extract(r.value, '$.base'), '')),
            'submit', json(json_extract(r.value, '$.submit'))) AS w
        FROM json_each(chains.doc, '$.repos') r ORDER BY r.key))))
WHERE EXISTS (SELECT 1 FROM json_each(chains.doc, '$.repos') r WHERE json_type(r.value, '$.place') IS NULL);

UPDATE cleanups SET doc = json_set(doc, '$.repos', json((
    SELECT json_group_array(json(w)) FROM (
        SELECT json_object(
            'repo', coalesce(json_extract(r.value, '$.repo'), ''),
            'place', json_object('kind', 'worktree',
                'project', coalesce(json_extract(r.value, '$.project'), ''),
                'host', coalesce(json_extract(r.value, '$.host'), ''),
                'path', coalesce(json_extract(r.value, '$.path'), ''),
                'branch', coalesce(json_extract(r.value, '$.branch'), ''),
                'base', coalesce(json_extract(r.value, '$.base'), '')),
            'submit', json(json_extract(r.value, '$.submit'))) AS w
        FROM json_each(cleanups.doc, '$.repos') r ORDER BY r.key))))
WHERE EXISTS (SELECT 1 FROM json_each(cleanups.doc, '$.repos') r WHERE json_type(r.value, '$.place') IS NULL);

-- The queue a landing waits in is its `queue`: the table is made again with the columns the queue reads by generated from
-- it, so a database that has them already is made again the same.
DROP TABLE IF EXISTS landings_new;
CREATE TABLE landings_new (id TEXT PRIMARY KEY, chain_id TEXT NOT NULL, repo TEXT NOT NULL, created_at INTEGER NOT NULL, doc TEXT NOT NULL, release TEXT GENERATED ALWAYS AS (json_extract(doc, '$.release')) VIRTUAL, host TEXT GENERATED ALWAYS AS (json_extract(doc, '$.queue.host')) VIRTUAL, project TEXT GENERATED ALWAYS AS (json_extract(doc, '$.queue.project')) VIRTUAL, base TEXT GENERATED ALWAYS AS (json_extract(doc, '$.queue.base')) VIRTUAL, state TEXT GENERATED ALWAYS AS (json_extract(doc, '$.state.kind')) VIRTUAL);
INSERT INTO landings_new(id, chain_id, repo, created_at, doc)
SELECT id, chain_id, repo, created_at,
    CASE WHEN json_type(doc, '$.queue') IS NULL AND json_type(doc, '$.host') IS NOT NULL
        THEN json_remove(json_set(doc, '$.queue', json_object(
            'host', coalesce(json_extract(doc, '$.host'), ''),
            'project', coalesce(json_extract(doc, '$.project'), ''),
            'base', coalesce(json_extract(doc, '$.base'), ''))), '$.host', '$.project', '$.base')
        ELSE doc END
FROM landings;
DROP TABLE landings;
ALTER TABLE landings_new RENAME TO landings;
CREATE INDEX landings_chain ON landings(chain_id, created_at);
CREATE INDEX landings_repo ON landings(repo, created_at);
CREATE INDEX landings_release ON landings(release) WHERE release IS NOT NULL;
CREATE INDEX landings_queue ON landings(host, project, base, state);
