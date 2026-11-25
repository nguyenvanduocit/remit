-- A judge's kind is a field of its card, set where Remit starts it (ADR-0107 step 7b), not read from the agent it runs.
-- Each card an earlier build started as a judge gets its kind from the agent it was started with (and, for the agent
-- both the goal and the check judge run, what it signs). A card that names its kind already is left as it is.
UPDATE cards SET doc = json_set(doc, '$.judge', CASE json_extract(doc, '$.agent')
    WHEN 'intake' THEN 'intake'
    WHEN 'duel_judge' THEN 'duel'
    WHEN 'second_opinion' THEN 'second'
    WHEN 'judge' THEN CASE json_extract(doc, '$.signs') WHEN 'check' THEN 'check' ELSE 'goal' END
  END)
WHERE json_extract(doc, '$.origin') = 'judge'
  AND json_type(doc, '$.judge') IS NULL
  AND json_extract(doc, '$.agent') IN ('intake', 'duel_judge', 'second_opinion', 'judge');
