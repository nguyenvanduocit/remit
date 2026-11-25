-- The owner's permission wait and the duels' agreement share carry who set them and why, as every setting does
-- (ADR-0118, point 8): a stored number becomes a setting by Remit, said so. A wait of 0 read as the default, so it is
-- dropped. Safe to run again: only a number is changed.
UPDATE settings SET doc = json_remove(doc, '$.permissionWaitMinutes') WHERE json_type(doc, '$.permissionWaitMinutes') = 'integer' AND json_extract(doc, '$.permissionWaitMinutes') <= 0;
UPDATE settings SET doc = json_set(doc, '$.permissionWaitMinutes', json_object('value', json_extract(doc, '$.permissionWaitMinutes'), 'by', json_object('kind', 'remit', 'who', '', 'via', ''), 'why', 'set before settings recorded why', 'at', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))) WHERE json_type(doc, '$.permissionWaitMinutes') = 'integer';
UPDATE settings SET doc = json_set(doc, '$.duelAgreementPercent', json_object('value', json_extract(doc, '$.duelAgreementPercent'), 'by', json_object('kind', 'remit', 'who', '', 'via', ''), 'why', 'set before settings recorded why', 'at', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))) WHERE json_type(doc, '$.duelAgreementPercent') = 'integer';
