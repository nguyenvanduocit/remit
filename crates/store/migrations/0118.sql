-- A playbook is reference, and a chain's kind is code (ADR-0113). Each statement rewrites only the documents that still
-- hold an earlier shape, so a second run changes nothing.

-- A chain takes its kind from the playbook it started from; its playbook, owner-waits and gates go, read from the kind
-- now. Its capabilities stay as stored: they are what it holds.
UPDATE chains SET doc = json_set(doc, '$.kind', CASE json_extract(doc, '$.playbookId')
    WHEN 'chore' THEN 'chore'
    WHEN 'objective' THEN 'objective'
    WHEN 'okr-review' THEN 'objective'
    WHEN 'okr-checkin' THEN 'objective'
    WHEN 'playbook-author' THEN 'author'
    WHEN 'release' THEN 'release'
    WHEN 'coach' THEN 'coach'
    WHEN 'analyst' THEN 'analyst'
    WHEN 'seeded-fault' THEN 'trial'
    ELSE 'work' END)
WHERE json_type(doc, '$.kind') IS NULL;

UPDATE chains SET doc = json_remove(doc, '$.playbook', '$.playbookId', '$.ownerWaits', '$.gates')
WHERE json_type(doc, '$.playbook') IS NOT NULL OR json_type(doc, '$.playbookId') IS NOT NULL
   OR json_type(doc, '$.ownerWaits') IS NOT NULL OR json_type(doc, '$.gates') IS NOT NULL;

-- A card does not remember where its Chair read its brief: a seed is a hire.
UPDATE cards SET doc = json_set(doc, '$.origin', 'hired') WHERE json_extract(doc, '$.origin') = 'seed';

-- A playbook keeps its text as its brief; the Chair it named and the capabilities it listed go.
UPDATE playbooks SET doc = json_set(doc, '$.brief', json_extract(doc, '$.chairBrief'))
WHERE json_type(doc, '$.chairBrief') IS NOT NULL;

UPDATE playbooks SET doc = json_remove(doc, '$.chairBrief', '$.chair', '$.capabilities')
WHERE json_type(doc, '$.chairBrief') IS NOT NULL OR json_type(doc, '$.chair') IS NOT NULL
   OR json_type(doc, '$.capabilities') IS NOT NULL;

-- The same in a playbook write an inbox item still holds open or decided. Its record is JSON held as text
-- (`PendingChange.record`), so it is read with json(), edited, and written back as text (`|| ''` drops the JSON subtype
-- that would store it as an object). Settled changes keep what they recorded: they are history.
UPDATE inbox SET doc = json_set(doc, '$.change.record', json_remove(
        CASE WHEN json_type(json(json_extract(doc, '$.change.record')), '$.chairBrief') IS NOT NULL
            THEN json_set(json(json_extract(doc, '$.change.record')), '$.brief',
                json_extract(json(json_extract(doc, '$.change.record')), '$.chairBrief'))
            ELSE json(json_extract(doc, '$.change.record')) END,
        '$.chairBrief', '$.chair', '$.capabilities') || '')
WHERE status IN ('open', 'decided') AND json_extract(doc, '$.change.kind') = 'playbook'
  AND json_valid(json_extract(doc, '$.change.record'))
  AND (json_type(json(json_extract(doc, '$.change.record')), '$.chairBrief') IS NOT NULL
    OR json_type(json(json_extract(doc, '$.change.record')), '$.chair') IS NOT NULL
    OR json_type(json(json_extract(doc, '$.change.record')), '$.capabilities') IS NOT NULL);

-- A routine's or an event's start names a kind. One that named a playbook a kind now stands for takes that kind; one
-- that named another keeps its intent in its goal, where its Chair reads it.
UPDATE subscriptions SET doc = json_remove(json_set(doc,
    '$.action.start.kind', CASE json_extract(doc, '$.action.start.playbookId')
        WHEN 'chore' THEN 'chore'
        WHEN 'objective' THEN 'objective'
        WHEN 'okr-review' THEN 'objective'
        WHEN 'okr-checkin' THEN 'objective'
        WHEN 'playbook-author' THEN 'author'
        WHEN 'release' THEN 'release'
        WHEN 'coach' THEN 'coach'
        WHEN 'analyst' THEN 'analyst'
        WHEN 'seeded-fault' THEN 'trial'
        ELSE 'work' END,
    '$.action.start.goal', CASE
        WHEN json_extract(doc, '$.action.start.playbookId') IN ('', 'chore', 'objective', 'okr-review', 'okr-checkin',
            'playbook-author', 'release', 'coach', 'analyst', 'seeded-fault')
            THEN json_extract(doc, '$.action.start.goal')
        ELSE coalesce(json_extract(doc, '$.action.start.goal'), '') || char(10) || char(10) || 'The playbook '
            || coalesce((SELECT json_extract(p.doc, '$.name') FROM playbooks p
                WHERE p.id = json_extract(subscriptions.doc, '$.action.start.playbookId')),
                json_extract(doc, '$.action.start.playbookId'))
            || ' (' || json_extract(doc, '$.action.start.playbookId') || ') fits this work: read it with read_playbook.'
        END),
    '$.action.start.playbookId')
WHERE json_type(doc, '$.action.start.playbookId') IS NOT NULL;

-- A client key starts work chains by its count: one whose list of playbooks let it start nothing starts none.
UPDATE client_keys SET doc = json_remove(json_set(doc, '$.chainsPerDay', CASE
        WHEN json_type(doc, '$.playbooks') = 'array' AND json_array_length(doc, '$.playbooks') > 0
            THEN coalesce(json_extract(doc, '$.chainsPerDay'), 0)
        ELSE 0 END),
    '$.playbooks')
WHERE json_type(doc, '$.playbooks') IS NOT NULL;

-- A duel's kind of work is its chain's kind, in its document and in the column agreement_of filters by; a duel whose chain
-- is gone reads work.
UPDATE duels SET
    doc = json_set(doc, '$.work', coalesce((SELECT json_extract(c.doc, '$.kind') FROM chains c WHERE c.id = duels.chain), 'work')),
    work = coalesce((SELECT json_extract(c.doc, '$.kind') FROM chains c WHERE c.id = duels.chain), 'work')
WHERE work IS NOT coalesce((SELECT json_extract(c.doc, '$.kind') FROM chains c WHERE c.id = duels.chain), 'work')
   OR json_extract(doc, '$.work') IS NOT coalesce((SELECT json_extract(c.doc, '$.kind') FROM chains c WHERE c.id = duels.chain), 'work');
