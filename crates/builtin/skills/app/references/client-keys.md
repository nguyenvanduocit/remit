# Client keys

A **client key** is a way in for one outside agent, such as the doctor that evaluates Remit. It holds a key that only that agent presents, at `/mcp/client` on this machine, and the limits he set. With it the agent reads what he sees, starts `work` chains within its limits, and pauses, finishes, stops or messages what runs. It never resumes a chain, answers the inbox, merges, or changes a cap, credential, key, event signal or definition: those stay his and yours.

- When he asks for one, make it with `create_client_key`, named as he would say it (lowercase, dashes), and say what the agent is for.
- You are never shown a key and must never ask for one: tell him to open the Client keys page, copy the key there and put it in a file of mode 0600 for the agent. A key never goes in a message, a prompt or an argument.
- Limits: `chainsPerDay` (default 3: leave it out to take it; 0 starts none) and `actionsPerHour` (default 60). A key starts `work` chains only: they start with no capability, hire no engineer and publish nothing. When an experiment follows a playbook, the agent names it in the goal.
- `list_client_keys` shows each key with its limits, when and from where it was last used, and the chains it started, in the shape `update_client_key` takes. `update_client_key` replaces the settings (start from the list and change what he asked); a name never changes.
- `pause_client_key` stops its next act at once and `resume_client_key` lets it in again. `rotate_client_key` only when he says a key leaked: the old one stops working at once. `delete_client_key` removes it. Pausing or deleting a key does not stop the chains it started: `list_client_keys` names them, and `pause_chain` pauses one if he asks.
- Every act of a key is on the chain it touched, as "Client · <key>", with the reason the agent gave. A chain it started shows as an outside agent's, and is left out of the per-kind counts of the Outcomes page.
