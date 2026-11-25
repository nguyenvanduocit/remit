
UPDATE cards SET doc = json_set(doc,
  '$.offer.ended', CASE WHEN json_extract(doc, '$.activity') = 'accepted' THEN 'accepted'
                        WHEN json_extract(doc, '$.activity') LIKE 'handed over to %' THEN 'handed_over' ELSE 'stopped' END,
  '$.rev', json_extract(doc, '$.rev') + 1)
WHERE json_extract(doc, '$.closed') = 1 AND json_type(doc, '$.offer') = 'object' AND json_type(doc, '$.offer.ended') IS NULL;
UPDATE cards SET doc = json_set(doc,
  '$.offer.replied', (SELECT strftime('%Y-%m-%dT%H:%M:%fZ', min(e.created_at) / 1000.0, 'unixepoch') FROM events AS e
    WHERE e.home = cards.home AND e.kind = 'prompt' AND e.text LIKE '[report from %'
      AND instr(e.text, ' · card ' || cards.id || ' · stop ') > 0 AND instr(e.text, ' · stop declined') = 0),
  '$.rev', json_extract(doc, '$.rev') + 1)
WHERE json_extract(doc, '$.offer.state') = 'accepted' AND json_type(doc, '$.offer.replied') IS NULL
  AND EXISTS (SELECT 1 FROM events AS e WHERE e.home = cards.home AND e.kind = 'prompt' AND e.text LIKE '[report from %'
      AND instr(e.text, ' · card ' || cards.id || ' · stop ') > 0 AND instr(e.text, ' · stop declined') = 0);
UPDATE cards SET doc = json_set(json_remove(doc, '$.closed'), '$.rev', json_extract(doc, '$.rev') + 1)
WHERE json_type(doc, '$.closed') IS NOT NULL;
