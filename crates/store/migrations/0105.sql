-- A judge Remit starts for a stated check signs the check and is named after it: before, it signed the goal under the goal
-- judge's name, and only its why told it apart. Its verdicts carry its name.
UPDATE cards SET doc = json_set(doc,
    '$.signs', 'check',
    '$.name', replace(json_extract(doc, '$.name'), 'Goal judge · review', 'Check judge · review'))
WHERE json_extract(doc, '$.origin') = 'judge' AND json_extract(doc, '$.agent') = 'judge'
  AND json_extract(doc, '$.why') = 'Reviews how the chain''s result will be checked, before the work starts.';

UPDATE approvals SET doc = json_set(doc,
    '$.name', replace(json_extract(doc, '$.name'), 'Goal judge · review', 'Check judge · review'))
WHERE json_extract(doc, '$.role') = 'check_review';
