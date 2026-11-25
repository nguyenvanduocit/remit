
UPDATE deliveries SET doc = json_set(doc, '$.burst', read.home)
FROM (SELECT c.id AS card, c.home FROM cards AS c
      JOIN deliveries AS d ON json_extract(d.doc, '$.cardId') = c.id
      GROUP BY c.id HAVING count(*) > 1) AS read
WHERE json_extract(deliveries.doc, '$.cardId') = read.card;
