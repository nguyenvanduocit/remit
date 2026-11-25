-- Remit lands every change itself (ADR-0114): the merge queue's own fields on each landing, and the merge requests, review
-- threads and comment drafts go. Migration 0119 carried each merge request over as a landing; this one gives each the
-- queue's address (the forge's host and path the chain works on), the count of its version, its title and its time of
-- change, keeps the draft merge requests it carried to be closed once on their forge (shipping's close_carried_mrs), and
-- takes from the chains what the merge requests were. Each statement changes only what still holds the earlier shape.

-- The queue a landing waits in is its forge's host, its path there and its base; the queue reads by them and its state.
-- The table is made again with those columns generated from each document, so a database that has them already is
-- made again the same.
CREATE TABLE landings_new (id TEXT PRIMARY KEY, chain_id TEXT NOT NULL, repo TEXT NOT NULL, created_at INTEGER NOT NULL, doc TEXT NOT NULL, release TEXT GENERATED ALWAYS AS (json_extract(doc, '$.release')) VIRTUAL, host TEXT GENERATED ALWAYS AS (json_extract(doc, '$.host')) VIRTUAL, project TEXT GENERATED ALWAYS AS (json_extract(doc, '$.project')) VIRTUAL, base TEXT GENERATED ALWAYS AS (json_extract(doc, '$.base')) VIRTUAL, state TEXT GENERATED ALWAYS AS (json_extract(doc, '$.state.kind')) VIRTUAL);
INSERT INTO landings_new(id, chain_id, repo, created_at, doc) SELECT id, chain_id, repo, created_at, doc FROM landings;
DROP TABLE landings;
ALTER TABLE landings_new RENAME TO landings;
CREATE INDEX landings_chain ON landings(chain_id, created_at);
CREATE INDEX landings_repo ON landings(repo, created_at);
CREATE INDEX landings_release ON landings(release) WHERE release IS NOT NULL;
CREATE INDEX landings_queue ON landings(host, project, base, state);

-- Every merge request a chain's repository holds, the current one and those before it, by its URL.
CREATE TEMP TABLE carried(chain_id TEXT, repo TEXT, host TEXT, project TEXT, url TEXT, iid INTEGER, state TEXT,
    title TEXT, description TEXT);
INSERT INTO carried
SELECT c.id, json_extract(r.value, '$.repo'), json_extract(r.value, '$.host'), json_extract(r.value, '$.project'),
    json_extract(r.value, '$.mr.url'), json_extract(r.value, '$.mr.iid'), json_extract(r.value, '$.mr.state'),
    json_extract(r.value, '$.mr.title'), json_extract(r.value, '$.mr.description')
FROM chains c, json_each(c.doc, '$.repos') r
WHERE json_type(r.value, '$.mr') = 'object'
UNION ALL
SELECT c.id, json_extract(r.value, '$.repo'), json_extract(r.value, '$.host'), json_extract(r.value, '$.project'),
    json_extract(e.value, '$.url'), json_extract(e.value, '$.iid'), json_extract(e.value, '$.state'),
    json_extract(e.value, '$.title'), json_extract(e.value, '$.description')
FROM chains c, json_each(c.doc, '$.repos') r, json_each(r.value, '$.earlierMrs') e
WHERE json_type(e.value) = 'object';

-- Each carried landing's host and path, from the chain's work on its repository, and its title and body from its merge
-- request.
UPDATE landings SET doc = json_set(doc,
    '$.host', coalesce((SELECT k.host FROM carried k WHERE k.chain_id = landings.chain_id AND k.repo = landings.repo
        LIMIT 1), ''),
    '$.project', coalesce((SELECT k.project FROM carried k WHERE k.chain_id = landings.chain_id
        AND k.repo = landings.repo LIMIT 1), ''),
    '$.version.title', coalesce((SELECT k.title FROM carried k WHERE k.chain_id = landings.chain_id
        AND k.url = json_extract(landings.doc, '$.reference') LIMIT 1), ''),
    '$.version.description', coalesce((SELECT k.description FROM carried k WHERE k.chain_id = landings.chain_id
        AND k.url = json_extract(landings.doc, '$.reference') LIMIT 1), ''))
WHERE json_type(doc, '$.host') IS NULL;

-- A version's number counts the chain's versions on the repository, oldest first; its time of change is its creation.
UPDATE landings SET doc = json_set(doc, '$.version.number', (
    SELECT count(*) FROM landings o WHERE o.chain_id = landings.chain_id AND o.repo = landings.repo
        AND (o.created_at < landings.created_at OR (o.created_at = landings.created_at AND o.rowid <= landings.rowid))))
WHERE json_type(doc, '$.version.number') IS NULL;
UPDATE landings SET doc = json_set(doc, '$.updatedAt', json_extract(doc, '$.createdAt'))
WHERE json_type(doc, '$.updatedAt') IS NULL;

-- The draft merge requests carried over, which a one-time job closes on their forge once.
CREATE TABLE IF NOT EXISTS carried_mrs (
    landing_id TEXT PRIMARY KEY,
    host TEXT NOT NULL,
    project TEXT NOT NULL,
    iid INTEGER NOT NULL,
    url TEXT NOT NULL
);
INSERT OR IGNORE INTO carried_mrs(landing_id, host, project, iid, url)
SELECT l.id, coalesce(k.host, ''), coalesce(k.project, ''), k.iid, k.url
FROM carried k JOIN landings l ON l.chain_id = k.chain_id AND json_extract(l.doc, '$.reference') = k.url
WHERE k.state IN ('opened', 'locked') AND k.iid > 0;
DROP TABLE carried;
-- A repository's work keeps where the chain works; what it pushed and its merge requests go, and a submit at work keeps
-- what the Chair asked (its checks run again when the server starts).
UPDATE chains SET doc = json_set(doc, '$.repos', json((
    SELECT json_group_array(json_remove(json(r.value), '$.mr', '$.earlierMrs', '$.pushedSha', '$.pushedFrom',
        '$.submit.step', '$.submit.from', '$.submit.head', '$.submit.by', '$.submit.change', '$.submit.gates',
        '$.submit.body'))
    FROM json_each(chains.doc, '$.repos') r)))
WHERE EXISTS (
    SELECT 1 FROM json_each(chains.doc, '$.repos') r
    WHERE json_type(r.value, '$.mr') IS NOT NULL OR json_type(r.value, '$.earlierMrs') IS NOT NULL
       OR json_type(r.value, '$.pushedSha') IS NOT NULL OR json_type(r.value, '$.pushedFrom') IS NOT NULL
       OR json_type(r.value, '$.submit.step') IS NOT NULL OR json_type(r.value, '$.submit.head') IS NOT NULL);

-- ship goes: every change lands through the queue, and no agent merges.
UPDATE chains SET doc = json_set(doc, '$.capabilities', json((
    SELECT json_group_array(value) FROM json_each(chains.doc, '$.capabilities') WHERE value <> 'ship')))
WHERE EXISTS (SELECT 1 FROM json_each(chains.doc, '$.capabilities') WHERE value = 'ship');

-- A live mark names the landing it marks by when it landed, as it named the merge by when it was merged.
UPDATE readings SET doc = json_remove(json_set(doc, '$.value.landed', json_extract(doc, '$.value.merged')), '$.value.merged')
WHERE key LIKE 'chain:%.live' AND json_type(doc, '$.value.merged') IS NOT NULL;

-- A submit, an MR moving, a merged stage and a resolved review comment were the ways a change went out: each is a landing's.
UPDATE decisions SET doc = json_set(doc, '$.kind', 'landing') WHERE json_extract(doc, '$.kind') IN ('mr', 'ship', 'resolve');

-- The owner's review threads and his unsent comments on an MR's diff go with the MR; a message draft keeps only what a
-- message is.
DROP TABLE IF EXISTS review_threads;
DELETE FROM drafts WHERE json_extract(doc, '$.kind') IN ('comment', 'reply');
UPDATE drafts SET doc = json_remove(doc, '$.kind', '$.repo', '$.path', '$.range', '$.head', '$.threadId')
WHERE json_type(doc, '$.kind') IS NOT NULL OR json_type(doc, '$.threadId') IS NOT NULL
   OR json_type(doc, '$.head') IS NOT NULL OR json_type(doc, '$.range') IS NOT NULL;

-- The merge-request sense goes; the landing sense is made for each chain whose landings' pipeline has not settled.
DELETE FROM sensors WHERE json_extract(doc, '$.kind') = 'plugin:forge.merge-request';

-- The subscriptions that shipped for merge requests go, unless the owner changed them.
DELETE FROM subscriptions
WHERE id IN ('sub_run_sync_merge_requests', 'sub_run_record_merge_request', 'sub_teach_edit')
  AND coalesce(json_extract(doc, '$.edited'), 0) = 0;

-- A watch on a chain's merge requests watches its landings; one on a merge watches a landing; a rule that looked at a
-- chain's merge requests looks at its landings.
UPDATE subscriptions SET "match" = replace("match", '.mr.', '.landing.'),
    doc = json_set(doc, '$.match', replace(json_extract(doc, '$.match'), '.mr.', '.landing.'))
WHERE "match" LIKE 'chain:%.mr.%';
UPDATE subscriptions SET "match" = 'fact:chain.landed', doc = json_set(doc, '$.match', 'fact:chain.landed')
WHERE "match" = 'fact:chain.merged';
UPDATE subscriptions SET doc = json_set(doc, '$.action.job', 'sync-landing')
WHERE json_extract(doc, '$.action.job') = 'sync-merge-request';
UPDATE subscriptions SET doc = json_set(doc, '$.action.job', 'sync-landings')
WHERE json_extract(doc, '$.action.job') = 'sync-merge-requests';
