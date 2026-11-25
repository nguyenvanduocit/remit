
WITH RECURSIVE lines(id, rest, line) AS (
  SELECT d.id, rtrim(json_extract(c.doc, '$.output'), ' ' || char(9, 10, 13)) || char(10), NULL
  FROM deliveries AS d JOIN cards AS c ON c.id = json_extract(d.doc, '$.cardId')
  WHERE d.status = 'handled' AND json_type(d.doc, '$.outcome') IS NULL
    AND rtrim(coalesce(json_extract(c.doc, '$.output'), ''), ' ' || char(9, 10, 13)) <> ''
  UNION ALL
  SELECT id, substr(rest, instr(rest, char(10)) + 1), substr(rest, 1, instr(rest, char(10)) - 1) FROM lines WHERE rest <> ''
)
UPDATE deliveries SET doc = json_set(doc, '$.outcome', trim(last.line, ' ' || char(9, 13)))
FROM (SELECT id, line FROM lines WHERE rest = '') AS last
WHERE deliveries.id = last.id;
