
CREATE TABLE usage (id INTEGER PRIMARY KEY AUTOINCREMENT, run_id TEXT NOT NULL, chain_id TEXT NOT NULL, card_id TEXT NOT NULL,
  day TEXT NOT NULL, reported REAL NOT NULL, usd REAL NOT NULL, at INTEGER NOT NULL);
CREATE INDEX usage_card ON usage(card_id);
CREATE INDEX usage_chain ON usage(chain_id);
CREATE INDEX usage_day ON usage(day);
WITH
  days AS (SELECT chain_id, day, SUM(usd) OVER w - usd AS lo, SUM(usd) OVER w AS hi FROM spend
    WINDOW w AS (PARTITION BY chain_id ORDER BY day)),
  costs AS (SELECT chain_id, id AS card_id, json_extract(doc, '$.costUsd') AS usd FROM cards WHERE json_extract(doc, '$.costUsd') > 0),
  card_spans AS (SELECT chain_id, card_id, SUM(usd) OVER w - usd AS lo, SUM(usd) OVER w AS hi FROM costs
    WINDOW w AS (PARTITION BY chain_id ORDER BY card_id)),
  day_totals AS (SELECT chain_id, MAX(hi) AS total FROM days GROUP BY chain_id),
  card_totals AS (SELECT chain_id, MAX(hi) AS total FROM card_spans GROUP BY chain_id),
  carried AS (
    SELECT d.chain_id, c.card_id, d.day, MIN(d.hi, c.hi) - MAX(d.lo, c.lo) AS usd FROM days d JOIN card_spans c USING (chain_id)
    UNION ALL
    SELECT d.chain_id, '', d.day, d.hi - MAX(d.lo, COALESCE(t.total, 0)) FROM days d LEFT JOIN card_totals t USING (chain_id)
    UNION ALL
    SELECT c.chain_id, c.card_id, '', c.hi - MAX(c.lo, COALESCE(t.total, 0)) FROM card_spans c LEFT JOIN day_totals t USING (chain_id))
INSERT INTO usage(run_id, chain_id, card_id, day, reported, usd, at) SELECT '', chain_id, card_id, day, 0, usd, 0 FROM carried WHERE usd > 1e-9;
UPDATE cards SET doc = json_remove(doc, '$.costUsd', '$.costBase');
UPDATE chains SET doc = json_remove(doc, '$.costUsd');
DROP TABLE spend;
