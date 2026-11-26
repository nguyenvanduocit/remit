# 0136. The owner selects the Assistant's runtime between turns

The Assistant's card holds its model and thinking effort. A running Claude Code process receives effort when it starts. The
owner needs to choose both where they send a message, and keep the choice across restarts.

Changing the live process would need separate provider controls for both settings. Restarting immediately during a turn
would interrupt work. The composer sends an owner-only runtime action between turns. The action validates against the capped
provider catalog and supported efforts, stores the updated card and an attributed stream note together, and closes the old
session. The next message opens a session with the conversation recap. Existing cards keep their saved choices. Flags supply
the initial pair and the model fallback if a narrower catalog excludes the saved model.

## Revisit when

When the provider can switch both settings atomically at a turn boundary, evaluate keeping its full session context. If
channels delegate runtime selection, define a permission for that action and keep the owner and the applied choices in the
stream.
