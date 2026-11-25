-- Whom a card's permission prompt waits on is a field of its card (ADR-0107 step 7d), not the words its activity shows.
-- Each card an earlier build left in status permission gets it from the line that build wrote: "waiting for the Chair: "
-- (the Chair decides, at its step or as the driver once the time limit passed) or "waiting for permission: " (the
-- owner's inbox); a line of neither reads as the owner's, where the Assistant decides it. A card that names its wait
-- already is left as it is.
UPDATE cards SET doc = json_set(doc, '$.permissionWait', json(CASE
    WHEN json_extract(doc, '$.activity') LIKE 'waiting for the Chair: %' THEN '{"on":"chair"}'
    ELSE '{"on":"owner"}'
  END))
WHERE json_extract(doc, '$.status') = 'permission'
  AND json_type(doc, '$.permissionWait') IS NULL;
