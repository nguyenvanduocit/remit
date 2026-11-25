
UPDATE events SET priority = 3 WHERE kind = 'prompt' AND priority = 2;
UPDATE events SET priority = 2 WHERE kind = 'prompt' AND by_kind = 'mindchains' AND by_who = 'plan';
