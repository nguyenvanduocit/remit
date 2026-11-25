-- Remit lands every change itself and releases what lands (ADR-0114, ADR-0116). Each statement is safe to run again: a
-- table is made if it is not there, a landing is carried over once (keyed by its chain, repository and merge request),
-- and a grant once.

-- A landing: one version of a chain's change to one repository, on its way to the base and maybe back. A release reads
-- its landings by the column generated from each one's document.
CREATE TABLE IF NOT EXISTS landings (id TEXT PRIMARY KEY, chain_id TEXT NOT NULL, repo TEXT NOT NULL, created_at INTEGER NOT NULL, doc TEXT NOT NULL, release TEXT GENERATED ALWAYS AS (json_extract(doc, '$.release')) VIRTUAL);
CREATE INDEX IF NOT EXISTS landings_chain ON landings(chain_id, created_at);
CREATE INDEX IF NOT EXISTS landings_repo ON landings(repo, created_at);
CREATE INDEX IF NOT EXISTS landings_release ON landings(release) WHERE release IS NOT NULL;

-- A release: one run of a repository's procedure on a landed commit.
CREATE TABLE IF NOT EXISTS releases (id TEXT PRIMARY KEY, repo TEXT NOT NULL, created_at INTEGER NOT NULL, doc TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS releases_repo ON releases(repo, created_at);

-- Each merge request a chain's repository work holds, its current one and the earlier ones, becomes a landing (ADR-0114,
-- Stored data and the cut-over): a merged one a landed landing, its URL its reference and its head its commit; an open
-- one (or a locked one, still open on its forge) whose version every gate it needs approves a ready landing at the
-- commit its push was made of, ready when the last of those verdicts came; an open one not approved the version that
-- waits for its gates. A gate approves a version when its latest verdict on the chain's repository approves that
-- version's patch; a version pushed before its gates were recorded needs review and QC. A closed one becomes no
-- landing: its chain's decisions keep it. The merge requests stay on the chains, which read them until the queue
-- replaces them.
WITH
mrs(chain_id, chain_at, chain_time, repo, base, version_from, mr) AS (
    SELECT c.id, c.created_at, json_extract(c.doc, '$.createdAt'), json_extract(r.value, '$.repo'),
        json_extract(r.value, '$.base'),
        coalesce(nullif(json_extract(r.value, '$.pushedFrom'), ''), json_extract(r.value, '$.mr.head')),
        json_extract(r.value, '$.mr')
    FROM chains c, json_each(c.doc, '$.repos') r
    WHERE json_type(r.value, '$.mr') = 'object'
    UNION ALL
    SELECT c.id, c.created_at, json_extract(c.doc, '$.createdAt'), json_extract(r.value, '$.repo'),
        json_extract(r.value, '$.base'), json_extract(e.value, '$.head'), e.value
    FROM chains c, json_each(c.doc, '$.repos') r, json_each(r.value, '$.earlierMrs') e
    WHERE json_type(e.value) = 'object'
),
gated AS (
    SELECT m.*, json_extract(m.mr, '$.state') AS mr_state, coalesce(json_extract(m.mr, '$.change'), '') AS change,
        CASE WHEN json_array_length(m.mr, '$.gates') > 0 THEN json_extract(m.mr, '$.gates')
            ELSE '["reviewer","qc"]' END AS gates
    FROM mrs m
    WHERE json_extract(m.mr, '$.state') IN ('merged', 'opened', 'locked')
),
latest(chain_id, repo, role, id, verdict, change, at) AS (
    SELECT a.chain_id, json_extract(a.doc, '$.repo'), json_extract(a.doc, '$.role'), a.id, json_extract(a.doc, '$.verdict'),
        json_extract(a.doc, '$.change'), json_extract(a.doc, '$.at')
    FROM approvals a
    WHERE NOT EXISTS (SELECT 1 FROM approvals b WHERE b.chain_id = a.chain_id
        AND json_extract(b.doc, '$.repo') IS json_extract(a.doc, '$.repo')
        AND json_extract(b.doc, '$.role') IS json_extract(a.doc, '$.role')
        AND (b.at > a.at OR (b.at = a.at AND b.rowid > a.rowid)))
),
judged AS (
    SELECT g.*,
        (SELECT json_group_array(id) FROM (SELECT l.id FROM json_each(g.gates) q JOIN latest l
            ON l.chain_id = g.chain_id AND l.repo = g.repo AND l.role = q.value AND l.verdict = 'approve'
            AND l.change = g.change AND g.change != '' ORDER BY q.key)) AS verdicts,
        (SELECT max(l.at) FROM json_each(g.gates) q JOIN latest l
            ON l.chain_id = g.chain_id AND l.repo = g.repo AND l.role = q.value AND l.verdict = 'approve'
            AND l.change = g.change AND g.change != '') AS approved_at
    FROM gated g
),
landing AS (
    SELECT j.*,
        CASE WHEN j.mr_state = 'merged' THEN 'landed'
            WHEN json_array_length(j.verdicts) = json_array_length(j.gates) THEN 'ready'
            ELSE 'judging' END AS state,
        coalesce(json_extract(j.mr, '$.updatedAt'), j.chain_time) AS created,
        nullif(json_extract(j.mr, '$.url'), '') AS reference
    FROM judged j
)
INSERT INTO landings(id, chain_id, repo, created_at, doc)
SELECT 'ln_' || lower(hex(randomblob(6))), l.chain_id, l.repo,
    coalesce(CAST(unixepoch(l.created, 'subsec') * 1000 AS INTEGER), l.chain_at),
    json_patch(
        json_object(
            'chainId', l.chain_id,
            'repo', l.repo,
            'base', coalesce(l.base, ''),
            'version', json_object(
                'from', coalesce(l.version_from, ''),
                'change', l.change,
                'gates', json(l.gates),
                'verdicts', json(CASE WHEN l.state = 'judging' THEN '[]' ELSE l.verdicts END),
                'by', json(coalesce(json_extract(l.mr, '$.by'), '[]'))),
            'priorityWhy', coalesce(json_extract(l.mr, '$.priorityWhy'), ''),
            'state', json_object('kind', l.state),
            'rev', 1),
        json_object(
            'priority', nullif(json_extract(l.mr, '$.priority'), ''),
            'commit', CASE WHEN l.state = 'landed' THEN nullif(json_extract(l.mr, '$.head'), '') END,
            'reference', l.reference,
            'createdAt', l.created,
            'readyAt', CASE WHEN l.state = 'ready' THEN l.approved_at END,
            'landedAt', CASE WHEN l.state = 'landed' THEN json_extract(l.mr, '$.decidedAt') END))
FROM landing l
WHERE NOT EXISTS (SELECT 1 FROM landings o WHERE o.chain_id = l.chain_id AND o.repo = l.repo
    AND json_extract(o.doc, '$.reference') IS l.reference);

-- A landing's document names its id, as every record's does.
UPDATE landings SET doc = json_set(doc, '$.id', id) WHERE json_extract(doc, '$.id') IS NOT id;

-- A grant is held by a chain or by a repository (ADR-0116): chain_grants becomes grants, each row held by its chain. The
-- grants a chain holds are named by a column generated from the holder, which a chain's removal deletes by.
CREATE TABLE IF NOT EXISTS grants (holder_kind TEXT NOT NULL CHECK (holder_kind IN ('chain', 'repository')), holder TEXT NOT NULL, source TEXT NOT NULL, connection TEXT NOT NULL, effect TEXT NOT NULL CHECK (effect IN ('read', 'reversible', 'compensable', 'irreversible')), granted_by TEXT NOT NULL, granted_at INTEGER NOT NULL, chain_id TEXT GENERATED ALWAYS AS (CASE holder_kind WHEN 'chain' THEN holder END) VIRTUAL, PRIMARY KEY(holder_kind, holder, source, connection));
CREATE INDEX IF NOT EXISTS grants_chain ON grants(chain_id) WHERE chain_id IS NOT NULL;
CREATE TABLE IF NOT EXISTS chain_grants (chain_id TEXT NOT NULL, source TEXT NOT NULL, connection TEXT NOT NULL, effect TEXT NOT NULL, granted_by TEXT NOT NULL, granted_at INTEGER NOT NULL, PRIMARY KEY(chain_id, source, connection));
INSERT OR IGNORE INTO grants(holder_kind, holder, source, connection, effect, granted_by, granted_at)
SELECT 'chain', chain_id, source, connection, effect, granted_by, granted_at FROM chain_grants;
DROP TABLE chain_grants;
