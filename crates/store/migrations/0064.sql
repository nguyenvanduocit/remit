
UPDATE cards SET doc = json_set(doc, '$.account', 'default') WHERE json_extract(doc, '$.account') = 'main';
UPDATE inbox SET doc = json_set(doc, '$.account', 'default') WHERE json_extract(doc, '$.account') = 'main';
DELETE FROM accounts WHERE id = 'main' AND EXISTS (SELECT 1 FROM accounts WHERE id = 'default');
UPDATE accounts SET id = 'default', doc = json_set(doc, '$.id', 'default') WHERE id = 'main';
