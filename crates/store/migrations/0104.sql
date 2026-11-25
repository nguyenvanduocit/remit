-- ADR-0050, ADR-0087: a stage is kept under the chain whose change it carries and that change's repository,
-- chain:<id>.stage.<repo> (the repository as the chain opened it), and names the chain whose Chair merged it ($.by). A stage
-- kept before under chain:<id>.stage was merged by that chain itself, on the repository whose MR it names, else on its one
-- repository. One whose repository cannot be told is no stage any merge could read, and goes.
CREATE TEMP TABLE stage_moves AS
SELECT r.key AS old_key, r.rev AS rev, c.id AS chain_id,
  COALESCE(
    (SELECT json_extract(w.value, '$.repo') FROM json_each(c.doc, '$.repos') AS w
      WHERE json_extract(w.value, '$.mr.ref') = json_extract(r.doc, '$.value.mr') LIMIT 1),
    CASE WHEN json_array_length(c.doc, '$.repos') = 1 THEN json_extract(c.doc, '$.repos[0].repo') END
  ) AS repo
FROM readings AS r JOIN chains AS c ON r.key = 'chain:' || c.id || '.stage';

UPDATE readings SET
  doc = json_set(json_remove(doc, '$.value.repo'),
    '$.key', key || '.' || (SELECT m.repo FROM stage_moves AS m WHERE m.old_key = readings.key AND m.rev = readings.rev),
    '$.value.by', (SELECT m.chain_id FROM stage_moves AS m WHERE m.old_key = readings.key AND m.rev = readings.rev)),
  key = key || '.' || (SELECT m.repo FROM stage_moves AS m WHERE m.old_key = readings.key AND m.rev = readings.rev)
WHERE EXISTS (SELECT 1 FROM stage_moves AS m WHERE m.old_key = readings.key AND m.rev = readings.rev AND m.repo IS NOT NULL);

-- what is left under the old key: a chain id holds no dot, so the key is chain:<id>.stage and nothing else
DELETE FROM readings
WHERE key GLOB 'chain:*.stage' AND instr(substr(key, 7, length(key) - 12), '.') = 0;

DROP TABLE stage_moves;
