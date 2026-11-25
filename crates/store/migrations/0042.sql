
DROP TABLE usage;
UPDATE chains SET doc = json_remove(doc, '$.budgetUsd', '$.wrapUp');
UPDATE playbooks SET doc = json_remove(doc, '$.budgetUsd');
UPDATE client_keys SET doc = json_remove(doc, '$.maxBudgetUsd');
DELETE FROM decisions WHERE kind = 'budget';
