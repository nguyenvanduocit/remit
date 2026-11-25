# grant

Whether an agent may act on a connection. An agent holds grants, never a credential: `decide` is the one rule, a pure function
over the grants a chain holds and the action it asks for, and `Effect` is the one scale of what an action does to the outside
world. `needs_owner` says which grants only the owner gives. It stores nothing and depends on no crate (ADR-0018).
