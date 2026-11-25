
UPDATE chains SET doc = json_set(doc, '$.repos', json((
  SELECT json_group_array(json(CASE
    WHEN json_type(r.value, '$.mr') = 'object' AND json_type(r.value, '$.earlierMrs') = 'array'
      THEN json_set(r.value, '$.mr.ref', '!' || json_extract(r.value, '$.mr.iid'), '$.earlierMrs', json((
        SELECT json_group_array(json_set(e.value, '$.ref', '!' || json_extract(e.value, '$.iid')) ORDER BY e.key) FROM json_each(r.value, '$.earlierMrs') AS e)))
    WHEN json_type(r.value, '$.mr') = 'object'
      THEN json_set(r.value, '$.mr.ref', '!' || json_extract(r.value, '$.mr.iid'))
    WHEN json_type(r.value, '$.earlierMrs') = 'array'
      THEN json_set(r.value, '$.earlierMrs', json((
        SELECT json_group_array(json_set(e.value, '$.ref', '!' || json_extract(e.value, '$.iid')) ORDER BY e.key) FROM json_each(r.value, '$.earlierMrs') AS e)))
    ELSE r.value END) ORDER BY r.key)
  FROM json_each(doc, '$.repos') AS r)))
WHERE json_type(doc, '$.repos') = 'array';
