-- Two judges Remit started for itself are gone (ADR-0109): the intake, whose choice of an owner's chain's title is its
-- Chair's in its first turn, and the check judge, whose review of a stated check is the goal judge's at the finish. Their
-- cards go, with what is recorded for each card (its events, session init, context and launches), and so do the check
-- judge's verdicts. The shipped subscriptions that hired them are ended as a subscription is (store/src/signal.rs
-- delete_subscriptions): their firings stay as the record of what they did, the ones still to be done are cancelled, and
-- an item of the owner's that one was held is answered by no one now. The decisions they made stay in the chains'
-- history. Run twice, it finds nothing left to remove.

DELETE FROM events WHERE card_id IN (SELECT id FROM cards WHERE json_extract(doc, '$.judge') IN ('intake', 'check'));
DELETE FROM session_inits WHERE card_id IN (SELECT id FROM cards WHERE json_extract(doc, '$.judge') IN ('intake', 'check'));
DELETE FROM session_contexts WHERE card_id IN (SELECT id FROM cards WHERE json_extract(doc, '$.judge') IN ('intake', 'check'));
DELETE FROM session_launches WHERE card_id IN (SELECT id FROM cards WHERE json_extract(doc, '$.judge') IN ('intake', 'check'));
DELETE FROM approvals WHERE json_extract(doc, '$.role') = 'check_review';
DELETE FROM cards WHERE json_extract(doc, '$.judge') IN ('intake', 'check');

UPDATE firings SET status = 'cancelled', doc = json_set(doc, '$.status', 'cancelled')
WHERE status IN ('pending', 'failed') AND subscription_id IN ('sub_hire_intake', 'sub_hire_check_reviewer');
DELETE FROM near_misses WHERE subscription_id IN ('sub_hire_intake', 'sub_hire_check_reviewer');
UPDATE inbox SET status = 'cancelled', doc = json_set(doc, '$.status', 'cancelled', '$.answer', 'the subscription is gone', '$.rev', json_extract(doc, '$.rev') + 1)
WHERE status = 'open' AND json_extract(doc, '$.subscription') IN ('sub_hire_intake', 'sub_hire_check_reviewer');
DELETE FROM subscriptions WHERE id IN ('sub_hire_intake', 'sub_hire_check_reviewer');

-- A chain no longer waits on a choice before it starts: its Chair titles it in its first turn, so "chosen" is not kept,
-- and the decisions that recorded a choice read as the chain titled.
UPDATE chains SET doc = json_remove(doc, '$.chosen') WHERE json_type(doc, '$.chosen') IS NOT NULL;
UPDATE decisions SET doc = json_set(doc, '$.kind', 'title') WHERE json_extract(doc, '$.kind') = 'choose';
