-- The approval a chain's finish waits on is named by its id under its own key (ADR-0107 step 7a): `$.finishing.verdict`,
-- which held that id, or "" while the goal judge had given none, becomes `$.finishing.approvalId`, left out while there
-- is none. Nothing else in a document is touched; a document without the old key is not written, so a second run
-- changes nothing.

UPDATE chains SET doc = CASE
    WHEN json_type(doc, '$.finishing.verdict') = 'text' AND json_extract(doc, '$.finishing.verdict') <> ''
      THEN json_set(json_remove(doc, '$.finishing.verdict'), '$.finishing.approvalId',
        json_extract(doc, '$.finishing.verdict'))
    ELSE json_remove(doc, '$.finishing.verdict')
  END
WHERE json_type(doc, '$.finishing.verdict') IS NOT NULL;
