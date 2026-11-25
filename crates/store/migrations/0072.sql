
UPDATE playbooks SET doc = json_set(replace(doc, 'ask_human', 'ask_owner'), '$.rev', json_extract(doc, '$.rev') + 1) WHERE instr(doc, 'ask_human') > 0;
UPDATE agents SET doc = json_set(replace(doc, 'ask_human', 'ask_owner'), '$.rev', json_extract(doc, '$.rev') + 1) WHERE instr(doc, 'ask_human') > 0;
UPDATE skills SET doc = json_set(replace(doc, 'ask_human', 'ask_owner'), '$.rev', json_extract(doc, '$.rev') + 1) WHERE instr(doc, 'ask_human') > 0;
UPDATE employees SET doc = json_set(doc, '$.loadout.instructions', replace(json_extract(doc, '$.loadout.instructions'), 'ask_human', 'ask_owner'), '$.rev', json_extract(doc, '$.rev') + 1)
WHERE instr(json_extract(doc, '$.loadout.instructions'), 'ask_human') > 0;
UPDATE cards SET doc = json_set(doc, '$.loadout.instructions', replace(json_extract(doc, '$.loadout.instructions'), 'ask_human', 'ask_owner'), '$.rev', json_extract(doc, '$.rev') + 1)
WHERE instr(json_extract(doc, '$.loadout.instructions'), 'ask_human') > 0;
