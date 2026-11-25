# library

What a Remit agent session is given and whether a definition may be stored. `access` is the one place that decides a
session's tools, skills, plugins and instructions from a card's kind and loadout; `validate_agent` and `validate_loadout`
say what to change for the library to store an agent or a card to hold a loadout; `diff_fields` names the fields two
stored records differ in, and `json_text` is the one way a change writes a value. Pure: the store, the sealed secrets and
the launcher stay outside and hand it plain values. It uses the pure part of `skills` (its plugin names) only.
