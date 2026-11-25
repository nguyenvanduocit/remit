
UPDATE events SET title = CASE
  WHEN text LIKE 'You are the Chair%' THEN 'Start the chain'
  WHEN text LIKE 'You judge whether a team%' THEN 'Check the finish against the goal'
  WHEN text LIKE 'You are %' THEN COALESCE((SELECT NULLIF(substr(trim(substr(l, 1, instr(l || char(10), char(10)) - 1)), 1, 70), '')
    FROM (SELECT replace(replace(replace(trim(COALESCE(NULLIF(json_extract(c.doc, '$.deliverable'), ''), json_extract(c.doc, '$.brief'), '')),
      '#', ''), '*', ''), char(96), '') AS l FROM cards AS c WHERE c.id = events.card_id)), 'Pick up work')
  WHEN text LIKE '[handoff from the Chair · about %''s work · %' OR text LIKE '[feedback from the Chair · about %''s work · %'
    OR text LIKE '[question from the Chair · about %''s work · %' THEN COALESCE(NULLIF(
      substr(substr(text, instr(text, '''s work · ') + 10), 1, instr(substr(text, instr(text, '''s work · ') + 10), ']') - 1), ''),
      CASE WHEN text LIKE '[feedback%' THEN 'Address feedback' WHEN text LIKE '[question%' THEN 'Answer a question' ELSE 'Receive results' END)
  WHEN text LIKE '[report from % ·%' THEN 'Read the report from ' || substr(text, 14, instr(text, ' ·') - 14)
  WHEN text LIKE '[FYI: the owner talked to % ·%' THEN 'You talked to ' || substr(text, 27, instr(text, ' ·') - 27)
  WHEN text LIKE '[message from the owner]' || char(10) || 'Review of my change%' THEN 'Your review'
  WHEN text LIKE '[message from the owner]' || char(10) || 'The owner replied on C%' THEN 'Your reply on C' || CAST(substr(text, 48) AS INTEGER)
  WHEN text LIKE '[message from the owner]%' THEN 'Your message'
  WHEN text LIKE '[question from % ·%' THEN 'Question from ' || substr(text, 16, instr(text, ' ·') - 16)
  WHEN text LIKE '[feedback from the Chair · rework]%' THEN 'Rework after feedback'
  WHEN text LIKE '[goal check stopped%' THEN 'Goal check stopped'
  WHEN text LIKE '[goal check%' THEN 'Goal check: something is missing'
  WHEN text LIKE '[finish refused]%' THEN 'Not allowed to finish yet'
  WHEN text LIKE '[finish!_chain failed]%' ESCAPE '!' THEN 'Finish failed'
  WHEN text LIKE '[submit!_mr result%' ESCAPE '!' THEN 'MR submit result'
  WHEN text LIKE '[Mindchains: make your plan true%' THEN 'Update the plan'
  WHEN text LIKE '[Mindchains: nothing in this chain has moved%' THEN 'Nudged for standing still'
  WHEN text LIKE '[Mindchains: checking back as you planned%' THEN 'Check back as planned'
  WHEN text LIKE '[Mindchains: your check-back at%' THEN 'Check-back cancelled'
  WHEN text LIKE '[budget reached%' THEN 'Wrap up at the budget limit'
  WHEN text LIKE '[The server restarted%' THEN 'Resume after a restart'
  WHEN text LIKE '[the chain was paused and is now resumed]%' THEN 'Resume the chain'
  WHEN text LIKE '[the owner answered your question%' THEN 'Your answer'
  ELSE COALESCE(NULLIF(substr(trim(substr(text, 1, instr(text || char(10), char(10)) - 1)), 1, 70), ''), 'Work turn')
END
WHERE kind = 'prompt' AND title = '';
UPDATE inbox SET doc = json_set(doc, '$.retry.title', (SELECT e.title FROM events AS e WHERE e.kind = 'prompt'
    AND e.card_id = json_extract(inbox.doc, '$.retry.cardId') AND e.text = json_extract(inbox.doc, '$.retry.text') ORDER BY e.id DESC LIMIT 1))
WHERE json_type(doc, '$.retry.text') = 'text' AND EXISTS (SELECT 1 FROM events AS e WHERE e.kind = 'prompt'
    AND e.card_id = json_extract(inbox.doc, '$.retry.cardId') AND e.text = json_extract(inbox.doc, '$.retry.text'));
