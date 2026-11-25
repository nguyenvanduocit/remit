-- What a channel acts on beyond a failure's kind (orch::FailureDetail), kept so a retry of the request is answered with it.
ALTER TABLE requests ADD COLUMN error_detail TEXT;
