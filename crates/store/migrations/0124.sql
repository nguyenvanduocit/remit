-- What lands is released by its repository's own procedure, in code (ADR-0116): the Release seat goes. Each statement is
-- safe to run again.

-- A repository's releases may be paused, by the owner or the Assistant for him, recorded with who and why: one row per
-- repository, its last pause, kept once lifted with who lifted it and why until the next pull replaces it.
CREATE TABLE IF NOT EXISTS release_pauses (repo TEXT PRIMARY KEY, doc TEXT NOT NULL);

-- What fed a Release seat ends, as a deleted subscription ends in code (store signal.rs `delete_subscriptions`): its
-- firings still to do are cancelled, its near misses go, an item held on it is answered by no one, and it goes. That is
-- every subscription of a chain of kind release, its watches (Remit's) and its waits (the chain's), as a finish in code
-- ends both (apply_subs.rs `put_waits`, `put_own_subs`); every one that starts a chain of kind release, a kind no build
-- reads now (the routine that started the seat, `sub_start_release_seat`, and any other whose start named it); and the
-- watches on the newest release of this server's repository, whose poll goes: a release is fact:repo.release now.
CREATE TEMP TABLE ending(id TEXT PRIMARY KEY);
INSERT INTO ending
SELECT id FROM subscriptions WHERE owner_kind IN ('remit', 'chain')
    AND owner_id IN (SELECT id FROM chains WHERE json_extract(doc, '$.kind') = 'release')
UNION SELECT id FROM subscriptions WHERE id = 'sub_start_release_seat'
    OR json_extract(doc, '$.action.start.kind') = 'release' OR "match" = 'fact:remit.released';
UPDATE firings SET status = 'cancelled', doc = json_set(doc, '$.status', 'cancelled')
WHERE status IN ('pending', 'failed') AND subscription_id IN (SELECT id FROM ending);
DELETE FROM near_misses WHERE subscription_id IN (SELECT id FROM ending);
UPDATE inbox SET status = 'cancelled', doc = json_set(doc, '$.status', 'cancelled', '$.answer', 'the subscription is gone',
    '$.rev', json_extract(doc, '$.rev') + 1)
WHERE status = 'open' AND json_extract(doc, '$.subscription') IN (SELECT id FROM ending);
DELETE FROM subscriptions WHERE id IN (SELECT id FROM ending);
DROP TABLE ending;

-- A chain of kind release was a Release seat: it is finished as a finish in code finishes one (step.rs `finish_to`), and
-- becomes a chore, a kind every build knows, holding no capability that is gone. Its sensors end with it; its cards,
-- queued messages and inbox items are settled by the next start's recovery (chains/src/recover.rs), as any finished
-- chain's are.
DELETE FROM sensors WHERE chain IN (SELECT id FROM chains WHERE json_extract(doc, '$.kind') = 'release');
UPDATE chains SET doc = json_remove(json_set(doc, '$.status', 'done', '$.kind', 'chore', '$.summary',
    CASE WHEN coalesce(json_extract(doc, '$.summary'), '') = ''
    THEN 'Finished: releases are run by each repository''s own .remit/release/, in code (ADR-0116).'
    ELSE json_extract(doc, '$.summary') END), '$.finishing')
WHERE json_extract(doc, '$.kind') = 'release';

-- The capability release goes from every chain that holds it.
UPDATE chains SET doc = json_set(doc, '$.capabilities', json((
    SELECT json_group_array(value) FROM json_each(chains.doc, '$.capabilities') WHERE value <> 'release')))
WHERE EXISTS (SELECT 1 FROM json_each(chains.doc, '$.capabilities') WHERE value = 'release');
