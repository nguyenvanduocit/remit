-- A key result with no check holds none, and a check always names its kind: where an earlier build wrote a check with no
-- kind (a direct request's key result before its Chair stated one, written as an object of blank fields) or with an empty
-- kind, that check is removed. So it is, at the paths an objective keeps one: in an objective's document, and in the
-- objective a change item of the inbox carries as its record (JSON in a string, kept a string). A check that names its
-- kind is left as it is, and nothing else in a document is touched. The checks of a document are removed one at a time,
-- as json_remove takes one path per step here; removing an object's key moves no other path. A document with none of
-- them is not written. No event and no change of the history is rewritten.

UPDATE objectives SET doc = (
  WITH RECURSIVE gone(n, p) AS (
    SELECT row_number() OVER (), fullkey FROM json_tree(objectives.doc) WHERE type = 'object'
      AND coalesce(json_extract(value, '$.kind'), '') = '' AND fullkey LIKE '$.keyResults[%].check'),
  done(i, d) AS (
    SELECT 0, objectives.doc
    UNION ALL SELECT i + 1, json_remove(d, (SELECT p FROM gone WHERE n = i + 1)) FROM done
    WHERE i < (SELECT count(*) FROM gone))
  SELECT d FROM done ORDER BY i DESC LIMIT 1)
WHERE EXISTS (SELECT 1 FROM json_tree(objectives.doc) WHERE type = 'object'
      AND coalesce(json_extract(value, '$.kind'), '') = '' AND fullkey LIKE '$.keyResults[%].check');

UPDATE inbox SET doc = json_set(doc, '$.change.record', (
  WITH RECURSIVE gone(n, p) AS (
    SELECT row_number() OVER (), fullkey FROM json_tree(json_extract(inbox.doc, '$.change.record')) WHERE type = 'object'
      AND coalesce(json_extract(value, '$.kind'), '') = '' AND fullkey LIKE '$.objective.keyResults[%].check'),
  done(i, d) AS (
    SELECT 0, json_extract(inbox.doc, '$.change.record')
    UNION ALL SELECT i + 1, json_remove(d, (SELECT p FROM gone WHERE n = i + 1)) FROM done
    WHERE i < (SELECT count(*) FROM gone))
  -- '' || d: the record stays a string, never the object json_remove's answer would be read as
  SELECT '' || d FROM done ORDER BY i DESC LIMIT 1))
WHERE json_valid(json_extract(doc, '$.change.record'))
  AND EXISTS (SELECT 1 FROM json_tree(json_extract(inbox.doc, '$.change.record')) WHERE type = 'object'
      AND coalesce(json_extract(value, '$.kind'), '') = '' AND fullkey LIKE '$.objective.keyResults[%].check');
