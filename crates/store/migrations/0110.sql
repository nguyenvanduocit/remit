-- A record writes every field, and a time or a list it may not hold is optional or read empty when absent (ADR-0107
-- step 3b): an optional time is left out when it is none, where an earlier build wrote the zero time
-- (0001-01-01T00:00:00Z); a list or a map is read empty when absent, where an earlier build wrote null; a merge request
-- whose head runs no checks has no checks, where an earlier build wrote an object with no state. So each such key is
-- removed, at the paths the records keep it, and nothing else in a document is touched (a record's foreign JSON keeps
-- its keys and values). The keys of a document are removed one at a time, as json_remove takes one path per step here;
-- removing an object's key moves no other path. A document with none of them is not written. No event is rewritten.

UPDATE chains SET doc = (
  WITH RECURSIVE gone(n, p) AS (
    SELECT row_number() OVER (), fullkey FROM json_tree(chains.doc) WHERE (type = 'text' AND atom = '0001-01-01T00:00:00Z' AND (
      fullkey LIKE '$.repos[%].mr.updatedAt'
      OR fullkey LIKE '$.repos[%].mr.decidedAt'
      OR fullkey LIKE '$.repos[%].earlierMrs[%].updatedAt'
      OR fullkey LIKE '$.repos[%].earlierMrs[%].decidedAt'))
    OR (type = 'object' AND json_extract(value, '$.state') IS NULL AND (
      fullkey LIKE '$.repos[%].mr.checks'
      OR fullkey LIKE '$.repos[%].earlierMrs[%].checks'))),
  done(i, d) AS (
    SELECT 0, chains.doc
    UNION ALL SELECT i + 1, json_remove(d, (SELECT p FROM gone WHERE n = i + 1)) FROM done
    WHERE i < (SELECT count(*) FROM gone))
  SELECT d FROM done ORDER BY i DESC LIMIT 1)
WHERE EXISTS (SELECT 1 FROM json_tree(chains.doc) WHERE (type = 'text' AND atom = '0001-01-01T00:00:00Z' AND (
      fullkey LIKE '$.repos[%].mr.updatedAt'
      OR fullkey LIKE '$.repos[%].mr.decidedAt'
      OR fullkey LIKE '$.repos[%].earlierMrs[%].updatedAt'
      OR fullkey LIKE '$.repos[%].earlierMrs[%].decidedAt'))
    OR (type = 'object' AND json_extract(value, '$.state') IS NULL AND (
      fullkey LIKE '$.repos[%].mr.checks'
      OR fullkey LIKE '$.repos[%].earlierMrs[%].checks')));

UPDATE cards SET doc = (
  WITH RECURSIVE gone(n, p) AS (
    SELECT row_number() OVER (), fullkey FROM json_tree(cards.doc) WHERE (type = 'null' AND (
      fullkey LIKE '$.plan'
      OR fullkey LIKE '$.loadout.skills'))),
  done(i, d) AS (
    SELECT 0, cards.doc
    UNION ALL SELECT i + 1, json_remove(d, (SELECT p FROM gone WHERE n = i + 1)) FROM done
    WHERE i < (SELECT count(*) FROM gone))
  SELECT d FROM done ORDER BY i DESC LIMIT 1)
WHERE EXISTS (SELECT 1 FROM json_tree(cards.doc) WHERE (type = 'null' AND (
      fullkey LIKE '$.plan'
      OR fullkey LIKE '$.loadout.skills')));

UPDATE agents SET doc = (
  WITH RECURSIVE gone(n, p) AS (
    SELECT row_number() OVER (), fullkey FROM json_tree(agents.doc) WHERE (type = 'null' AND (
      fullkey LIKE '$.skills'))),
  done(i, d) AS (
    SELECT 0, agents.doc
    UNION ALL SELECT i + 1, json_remove(d, (SELECT p FROM gone WHERE n = i + 1)) FROM done
    WHERE i < (SELECT count(*) FROM gone))
  SELECT d FROM done ORDER BY i DESC LIMIT 1)
WHERE EXISTS (SELECT 1 FROM json_tree(agents.doc) WHERE (type = 'null' AND (
      fullkey LIKE '$.skills')));

UPDATE employees SET doc = (
  WITH RECURSIVE gone(n, p) AS (
    SELECT row_number() OVER (), fullkey FROM json_tree(employees.doc) WHERE (type = 'null' AND (
      fullkey LIKE '$.loadout.skills'))),
  done(i, d) AS (
    SELECT 0, employees.doc
    UNION ALL SELECT i + 1, json_remove(d, (SELECT p FROM gone WHERE n = i + 1)) FROM done
    WHERE i < (SELECT count(*) FROM gone))
  SELECT d FROM done ORDER BY i DESC LIMIT 1)
WHERE EXISTS (SELECT 1 FROM json_tree(employees.doc) WHERE (type = 'null' AND (
      fullkey LIKE '$.loadout.skills')));

UPDATE inbox SET doc = (
  WITH RECURSIVE gone(n, p) AS (
    SELECT row_number() OVER (), fullkey FROM json_tree(inbox.doc) WHERE (type = 'null' AND (
      fullkey LIKE '$.options'
      OR fullkey LIKE '$.change.fields'
      OR fullkey LIKE '$.replay.briefs'))),
  done(i, d) AS (
    SELECT 0, inbox.doc
    UNION ALL SELECT i + 1, json_remove(d, (SELECT p FROM gone WHERE n = i + 1)) FROM done
    WHERE i < (SELECT count(*) FROM gone))
  SELECT d FROM done ORDER BY i DESC LIMIT 1)
WHERE EXISTS (SELECT 1 FROM json_tree(inbox.doc) WHERE (type = 'null' AND (
      fullkey LIKE '$.options'
      OR fullkey LIKE '$.change.fields'
      OR fullkey LIKE '$.replay.briefs')));

UPDATE playbooks SET doc = (
  WITH RECURSIVE gone(n, p) AS (
    SELECT row_number() OVER (), fullkey FROM json_tree(playbooks.doc) WHERE (type = 'null' AND (
      fullkey LIKE '$.seeds'))),
  done(i, d) AS (
    SELECT 0, playbooks.doc
    UNION ALL SELECT i + 1, json_remove(d, (SELECT p FROM gone WHERE n = i + 1)) FROM done
    WHERE i < (SELECT count(*) FROM gone))
  SELECT d FROM done ORDER BY i DESC LIMIT 1)
WHERE EXISTS (SELECT 1 FROM json_tree(playbooks.doc) WHERE (type = 'null' AND (
      fullkey LIKE '$.seeds')));

UPDATE review_threads SET doc = (
  WITH RECURSIVE gone(n, p) AS (
    SELECT row_number() OVER (), fullkey FROM json_tree(review_threads.doc) WHERE (type = 'null' AND (
      fullkey LIKE '$.messages'))),
  done(i, d) AS (
    SELECT 0, review_threads.doc
    UNION ALL SELECT i + 1, json_remove(d, (SELECT p FROM gone WHERE n = i + 1)) FROM done
    WHERE i < (SELECT count(*) FROM gone))
  SELECT d FROM done ORDER BY i DESC LIMIT 1)
WHERE EXISTS (SELECT 1 FROM json_tree(review_threads.doc) WHERE (type = 'null' AND (
      fullkey LIKE '$.messages')));

UPDATE session_inits SET doc = (
  WITH RECURSIVE gone(n, p) AS (
    SELECT row_number() OVER (), fullkey FROM json_tree(session_inits.doc) WHERE (type = 'null' AND (
      fullkey LIKE '$.tools'
      OR fullkey LIKE '$.mcpServers'
      OR fullkey LIKE '$.skills'
      OR fullkey LIKE '$.plugins'
      OR fullkey LIKE '$.pluginErrors'
      OR fullkey LIKE '$.agents'
      OR fullkey LIKE '$.slashCommands'))),
  done(i, d) AS (
    SELECT 0, session_inits.doc
    UNION ALL SELECT i + 1, json_remove(d, (SELECT p FROM gone WHERE n = i + 1)) FROM done
    WHERE i < (SELECT count(*) FROM gone))
  SELECT d FROM done ORDER BY i DESC LIMIT 1)
WHERE EXISTS (SELECT 1 FROM json_tree(session_inits.doc) WHERE (type = 'null' AND (
      fullkey LIKE '$.tools'
      OR fullkey LIKE '$.mcpServers'
      OR fullkey LIKE '$.skills'
      OR fullkey LIKE '$.plugins'
      OR fullkey LIKE '$.pluginErrors'
      OR fullkey LIKE '$.agents'
      OR fullkey LIKE '$.slashCommands')));

UPDATE session_contexts SET doc = (
  WITH RECURSIVE gone(n, p) AS (
    SELECT row_number() OVER (), fullkey FROM json_tree(session_contexts.doc) WHERE (type = 'null' AND (
      fullkey LIKE '$.parts'
      OR fullkey LIKE '$.memoryFiles'
      OR fullkey LIKE '$.skills'
      OR fullkey LIKE '$.agents'
      OR fullkey LIKE '$.mcpTools'
      OR fullkey LIKE '$.toolCalls'))),
  done(i, d) AS (
    SELECT 0, session_contexts.doc
    UNION ALL SELECT i + 1, json_remove(d, (SELECT p FROM gone WHERE n = i + 1)) FROM done
    WHERE i < (SELECT count(*) FROM gone))
  SELECT d FROM done ORDER BY i DESC LIMIT 1)
WHERE EXISTS (SELECT 1 FROM json_tree(session_contexts.doc) WHERE (type = 'null' AND (
      fullkey LIKE '$.parts'
      OR fullkey LIKE '$.memoryFiles'
      OR fullkey LIKE '$.skills'
      OR fullkey LIKE '$.agents'
      OR fullkey LIKE '$.mcpTools'
      OR fullkey LIKE '$.toolCalls')));

UPDATE changes SET doc = (
  WITH RECURSIVE gone(n, p) AS (
    SELECT row_number() OVER (), fullkey FROM json_tree(changes.doc) WHERE (type = 'null' AND (
      fullkey LIKE '$.fields'))),
  done(i, d) AS (
    SELECT 0, changes.doc
    UNION ALL SELECT i + 1, json_remove(d, (SELECT p FROM gone WHERE n = i + 1)) FROM done
    WHERE i < (SELECT count(*) FROM gone))
  SELECT d FROM done ORDER BY i DESC LIMIT 1)
WHERE EXISTS (SELECT 1 FROM json_tree(changes.doc) WHERE (type = 'null' AND (
      fullkey LIKE '$.fields')));

UPDATE objectives SET doc = (
  WITH RECURSIVE gone(n, p) AS (
    SELECT row_number() OVER (), fullkey FROM json_tree(objectives.doc) WHERE (type = 'null' AND (
      fullkey LIKE '$.keyResults'
      OR fullkey LIKE '$.horizon.text'))
    OR (type = 'text' AND atom = '0001-01-01T00:00:00Z' AND (
      fullkey LIKE '$.keyResults[%].check.expiry'
      OR fullkey LIKE '$.surrogates[%].expires'
      OR fullkey LIKE '$.horizon.until'))),
  done(i, d) AS (
    SELECT 0, objectives.doc
    UNION ALL SELECT i + 1, json_remove(d, (SELECT p FROM gone WHERE n = i + 1)) FROM done
    WHERE i < (SELECT count(*) FROM gone))
  SELECT d FROM done ORDER BY i DESC LIMIT 1)
WHERE EXISTS (SELECT 1 FROM json_tree(objectives.doc) WHERE (type = 'null' AND (
      fullkey LIKE '$.keyResults'
      OR fullkey LIKE '$.horizon.text'))
    OR (type = 'text' AND atom = '0001-01-01T00:00:00Z' AND (
      fullkey LIKE '$.keyResults[%].check.expiry'
      OR fullkey LIKE '$.surrogates[%].expires'
      OR fullkey LIKE '$.horizon.until')));

UPDATE client_keys SET doc = (
  WITH RECURSIVE gone(n, p) AS (
    SELECT row_number() OVER (), fullkey FROM json_tree(client_keys.doc) WHERE (type = 'null' AND (
      fullkey LIKE '$.playbooks'))
    OR (type = 'text' AND atom = '0001-01-01T00:00:00Z' AND (
      fullkey LIKE '$.lastUsedAt'))),
  done(i, d) AS (
    SELECT 0, client_keys.doc
    UNION ALL SELECT i + 1, json_remove(d, (SELECT p FROM gone WHERE n = i + 1)) FROM done
    WHERE i < (SELECT count(*) FROM gone))
  SELECT d FROM done ORDER BY i DESC LIMIT 1)
WHERE EXISTS (SELECT 1 FROM json_tree(client_keys.doc) WHERE (type = 'null' AND (
      fullkey LIKE '$.playbooks'))
    OR (type = 'text' AND atom = '0001-01-01T00:00:00Z' AND (
      fullkey LIKE '$.lastUsedAt')));

UPDATE worker_keys SET doc = (
  WITH RECURSIVE gone(n, p) AS (
    SELECT row_number() OVER (), fullkey FROM json_tree(worker_keys.doc) WHERE (type = 'text' AND atom = '0001-01-01T00:00:00Z' AND (
      fullkey LIKE '$.lastUsedAt'))),
  done(i, d) AS (
    SELECT 0, worker_keys.doc
    UNION ALL SELECT i + 1, json_remove(d, (SELECT p FROM gone WHERE n = i + 1)) FROM done
    WHERE i < (SELECT count(*) FROM gone))
  SELECT d FROM done ORDER BY i DESC LIMIT 1)
WHERE EXISTS (SELECT 1 FROM json_tree(worker_keys.doc) WHERE (type = 'text' AND atom = '0001-01-01T00:00:00Z' AND (
      fullkey LIKE '$.lastUsedAt')));

UPDATE skills SET doc = (
  WITH RECURSIVE gone(n, p) AS (
    SELECT row_number() OVER (), fullkey FROM json_tree(skills.doc) WHERE (type = 'null' AND (
      fullkey LIKE '$.files'))),
  done(i, d) AS (
    SELECT 0, skills.doc
    UNION ALL SELECT i + 1, json_remove(d, (SELECT p FROM gone WHERE n = i + 1)) FROM done
    WHERE i < (SELECT count(*) FROM gone))
  SELECT d FROM done ORDER BY i DESC LIMIT 1)
WHERE EXISTS (SELECT 1 FROM json_tree(skills.doc) WHERE (type = 'null' AND (
      fullkey LIKE '$.files')));

UPDATE retrospectives SET doc = (
  WITH RECURSIVE gone(n, p) AS (
    SELECT row_number() OVER (), fullkey FROM json_tree(retrospectives.doc) WHERE (type = 'null' AND (
      fullkey LIKE '$.issues'
      OR fullkey LIKE '$.memories'))),
  done(i, d) AS (
    SELECT 0, retrospectives.doc
    UNION ALL SELECT i + 1, json_remove(d, (SELECT p FROM gone WHERE n = i + 1)) FROM done
    WHERE i < (SELECT count(*) FROM gone))
  SELECT d FROM done ORDER BY i DESC LIMIT 1)
WHERE EXISTS (SELECT 1 FROM json_tree(retrospectives.doc) WHERE (type = 'null' AND (
      fullkey LIKE '$.issues'
      OR fullkey LIKE '$.memories')));

UPDATE workflows SET doc = (
  WITH RECURSIVE gone(n, p) AS (
    SELECT row_number() OVER (), fullkey FROM json_tree(workflows.doc) WHERE (type = 'null' AND (
      fullkey LIKE '$.steps'))
    OR (type = 'text' AND atom = '0001-01-01T00:00:00Z' AND (
      fullkey LIKE '$.endedAt'))),
  done(i, d) AS (
    SELECT 0, workflows.doc
    UNION ALL SELECT i + 1, json_remove(d, (SELECT p FROM gone WHERE n = i + 1)) FROM done
    WHERE i < (SELECT count(*) FROM gone))
  SELECT d FROM done ORDER BY i DESC LIMIT 1)
WHERE EXISTS (SELECT 1 FROM json_tree(workflows.doc) WHERE (type = 'null' AND (
      fullkey LIKE '$.steps'))
    OR (type = 'text' AND atom = '0001-01-01T00:00:00Z' AND (
      fullkey LIKE '$.endedAt')));

UPDATE workflow_steps SET doc = (
  WITH RECURSIVE gone(n, p) AS (
    SELECT row_number() OVER (), fullkey FROM json_tree(workflow_steps.doc) WHERE (type = 'text' AND atom = '0001-01-01T00:00:00Z' AND (
      fullkey LIKE '$.endedAt'))),
  done(i, d) AS (
    SELECT 0, workflow_steps.doc
    UNION ALL SELECT i + 1, json_remove(d, (SELECT p FROM gone WHERE n = i + 1)) FROM done
    WHERE i < (SELECT count(*) FROM gone))
  SELECT d FROM done ORDER BY i DESC LIMIT 1)
WHERE EXISTS (SELECT 1 FROM json_tree(workflow_steps.doc) WHERE (type = 'text' AND atom = '0001-01-01T00:00:00Z' AND (
      fullkey LIKE '$.endedAt')));

UPDATE helps SET doc = (
  WITH RECURSIVE gone(n, p) AS (
    SELECT row_number() OVER (), fullkey FROM json_tree(helps.doc) WHERE (type = 'text' AND atom = '0001-01-01T00:00:00Z' AND (
      fullkey LIKE '$.answeredAt'
      OR fullkey LIKE '$.endedAt'))),
  done(i, d) AS (
    SELECT 0, helps.doc
    UNION ALL SELECT i + 1, json_remove(d, (SELECT p FROM gone WHERE n = i + 1)) FROM done
    WHERE i < (SELECT count(*) FROM gone))
  SELECT d FROM done ORDER BY i DESC LIMIT 1)
WHERE EXISTS (SELECT 1 FROM json_tree(helps.doc) WHERE (type = 'text' AND atom = '0001-01-01T00:00:00Z' AND (
      fullkey LIKE '$.answeredAt'
      OR fullkey LIKE '$.endedAt')));

UPDATE subscriptions SET doc = (
  WITH RECURSIVE gone(n, p) AS (
    SELECT row_number() OVER (), fullkey FROM json_tree(subscriptions.doc) WHERE (type = 'text' AND atom = '0001-01-01T00:00:00Z' AND (
      fullkey LIKE '$.schedule.at'
      OR fullkey LIKE '$.schedule.last'
      OR fullkey LIKE '$.until'
      OR fullkey LIKE '$.dueAt'
      OR fullkey LIKE '$.lastFired'))),
  done(i, d) AS (
    SELECT 0, subscriptions.doc
    UNION ALL SELECT i + 1, json_remove(d, (SELECT p FROM gone WHERE n = i + 1)) FROM done
    WHERE i < (SELECT count(*) FROM gone))
  SELECT d FROM done ORDER BY i DESC LIMIT 1)
WHERE EXISTS (SELECT 1 FROM json_tree(subscriptions.doc) WHERE (type = 'text' AND atom = '0001-01-01T00:00:00Z' AND (
      fullkey LIKE '$.schedule.at'
      OR fullkey LIKE '$.schedule.last'
      OR fullkey LIKE '$.until'
      OR fullkey LIKE '$.dueAt'
      OR fullkey LIKE '$.lastFired')));

UPDATE firings SET doc = (
  WITH RECURSIVE gone(n, p) AS (
    SELECT row_number() OVER (), fullkey FROM json_tree(firings.doc) WHERE (type = 'text' AND atom = '0001-01-01T00:00:00Z' AND (
      fullkey LIKE '$.nextAttempt'))),
  done(i, d) AS (
    SELECT 0, firings.doc
    UNION ALL SELECT i + 1, json_remove(d, (SELECT p FROM gone WHERE n = i + 1)) FROM done
    WHERE i < (SELECT count(*) FROM gone))
  SELECT d FROM done ORDER BY i DESC LIMIT 1)
WHERE EXISTS (SELECT 1 FROM json_tree(firings.doc) WHERE (type = 'text' AND atom = '0001-01-01T00:00:00Z' AND (
      fullkey LIKE '$.nextAttempt')));
