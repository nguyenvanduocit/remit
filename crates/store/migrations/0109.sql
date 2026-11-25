-- A string enum reads only the values it names (ADR-0107): its empty text, which an earlier build wrote for a
-- field it had no value for, names none. A field that may have no value is optional now and reads absent as none; a
-- chain's origin, a subscription's flow policy and its overlap read absent as owner, each and queue, as their empty
-- text was taken. So each key below that holds the empty text is removed, at the paths the records keep it, and
-- nothing else in a document is touched (a record's foreign JSON keeps its keys). The keys of a document are removed
-- one at a time, as json_remove takes one path per step here; removing an object's key moves no other path. A
-- document with none of them is not written.

UPDATE chains SET doc = (
  WITH RECURSIVE gone(n, p) AS (
    SELECT row_number() OVER (), fullkey FROM json_tree(chains.doc) WHERE type = 'text' AND atom = '' AND (
      fullkey LIKE '$.origin'
      OR fullkey LIKE '$.unjudged'
      OR fullkey LIKE '$.repos[%].mr.priority'
      OR fullkey LIKE '$.repos[%].mr.checks.state'
      OR fullkey LIKE '$.repos[%].earlierMrs[%].priority'
      OR fullkey LIKE '$.repos[%].earlierMrs[%].checks.state'
      OR fullkey LIKE '$.repos[%].submit.priority')),
  done(i, d) AS (
    SELECT 0, chains.doc
    UNION ALL SELECT i + 1, json_remove(d, (SELECT p FROM gone WHERE n = i + 1)) FROM done
    WHERE i < (SELECT count(*) FROM gone))
  SELECT d FROM done ORDER BY i DESC LIMIT 1)
WHERE EXISTS (SELECT 1 FROM json_tree(chains.doc) WHERE type = 'text' AND atom = '' AND (
      fullkey LIKE '$.origin'
      OR fullkey LIKE '$.unjudged'
      OR fullkey LIKE '$.repos[%].mr.priority'
      OR fullkey LIKE '$.repos[%].mr.checks.state'
      OR fullkey LIKE '$.repos[%].earlierMrs[%].priority'
      OR fullkey LIKE '$.repos[%].earlierMrs[%].checks.state'
      OR fullkey LIKE '$.repos[%].submit.priority'));

UPDATE approvals SET doc = (
  WITH RECURSIVE gone(n, p) AS (
    SELECT row_number() OVER (), fullkey FROM json_tree(approvals.doc) WHERE type = 'text' AND atom = '' AND (
      fullkey LIKE '$.findings[%].blocker'
      OR fullkey LIKE '$.findings[%].code')),
  done(i, d) AS (
    SELECT 0, approvals.doc
    UNION ALL SELECT i + 1, json_remove(d, (SELECT p FROM gone WHERE n = i + 1)) FROM done
    WHERE i < (SELECT count(*) FROM gone))
  SELECT d FROM done ORDER BY i DESC LIMIT 1)
WHERE EXISTS (SELECT 1 FROM json_tree(approvals.doc) WHERE type = 'text' AND atom = '' AND (
      fullkey LIKE '$.findings[%].blocker'
      OR fullkey LIKE '$.findings[%].code'));

UPDATE inbox SET doc = (
  WITH RECURSIVE gone(n, p) AS (
    SELECT row_number() OVER (), fullkey FROM json_tree(inbox.doc) WHERE type = 'text' AND atom = '' AND (
      fullkey LIKE '$.changeKind'
      OR fullkey LIKE '$.permission.outcome')),
  done(i, d) AS (
    SELECT 0, inbox.doc
    UNION ALL SELECT i + 1, json_remove(d, (SELECT p FROM gone WHERE n = i + 1)) FROM done
    WHERE i < (SELECT count(*) FROM gone))
  SELECT d FROM done ORDER BY i DESC LIMIT 1)
WHERE EXISTS (SELECT 1 FROM json_tree(inbox.doc) WHERE type = 'text' AND atom = '' AND (
      fullkey LIKE '$.changeKind'
      OR fullkey LIKE '$.permission.outcome'));

UPDATE agents SET doc = (
  WITH RECURSIVE gone(n, p) AS (
    SELECT row_number() OVER (), fullkey FROM json_tree(agents.doc) WHERE type = 'text' AND atom = '' AND (
      fullkey LIKE '$.title')),
  done(i, d) AS (
    SELECT 0, agents.doc
    UNION ALL SELECT i + 1, json_remove(d, (SELECT p FROM gone WHERE n = i + 1)) FROM done
    WHERE i < (SELECT count(*) FROM gone))
  SELECT d FROM done ORDER BY i DESC LIMIT 1)
WHERE EXISTS (SELECT 1 FROM json_tree(agents.doc) WHERE type = 'text' AND atom = '' AND (
      fullkey LIKE '$.title'));

UPDATE objectives SET doc = (
  WITH RECURSIVE gone(n, p) AS (
    SELECT row_number() OVER (), fullkey FROM json_tree(objectives.doc) WHERE type = 'text' AND atom = '' AND (
      fullkey LIKE '$.keyResults[%].check.kind')),
  done(i, d) AS (
    SELECT 0, objectives.doc
    UNION ALL SELECT i + 1, json_remove(d, (SELECT p FROM gone WHERE n = i + 1)) FROM done
    WHERE i < (SELECT count(*) FROM gone))
  SELECT d FROM done ORDER BY i DESC LIMIT 1)
WHERE EXISTS (SELECT 1 FROM json_tree(objectives.doc) WHERE type = 'text' AND atom = '' AND (
      fullkey LIKE '$.keyResults[%].check.kind'));

UPDATE subscriptions SET doc = (
  WITH RECURSIVE gone(n, p) AS (
    SELECT row_number() OVER (), fullkey FROM json_tree(subscriptions.doc) WHERE type = 'text' AND atom = '' AND (
      fullkey LIKE '$.flow.policy'
      OR fullkey LIKE '$.overlap')),
  done(i, d) AS (
    SELECT 0, subscriptions.doc
    UNION ALL SELECT i + 1, json_remove(d, (SELECT p FROM gone WHERE n = i + 1)) FROM done
    WHERE i < (SELECT count(*) FROM gone))
  SELECT d FROM done ORDER BY i DESC LIMIT 1)
WHERE EXISTS (SELECT 1 FROM json_tree(subscriptions.doc) WHERE type = 'text' AND atom = '' AND (
      fullkey LIKE '$.flow.policy'
      OR fullkey LIKE '$.overlap'));
