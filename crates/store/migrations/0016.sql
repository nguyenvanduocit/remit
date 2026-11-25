
UPDATE inbox SET doc = json_remove(json_set(doc, '$.retry.promptId', (SELECT e.id FROM events AS e WHERE e.kind = 'prompt' AND e.status = 'delivered'
    AND e.card_id = json_extract(inbox.doc, '$.retry.cardId') AND e.text = json_extract(inbox.doc, '$.retry.text') ORDER BY e.id DESC LIMIT 1)),
  '$.retry.title', '$.retry.text', '$.retry.by')
WHERE json_type(doc, '$.retry.text') = 'text';
