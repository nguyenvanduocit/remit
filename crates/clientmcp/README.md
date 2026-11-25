# clientmcp

The door an outside agent holding a client key reaches Remit through (`/mcp/client`, ADR-0009): a channel, thin on purpose.
A request is let in only from this machine and only with a key the owner handed out; what the holder may then do is the MCP
server the application layer gives that key (`Keys::client_server`), whose limits and rails the orchestrator keeps, not this
door. MCP is `rmcp`'s streamable HTTP service on axum, built per request for the key that opened it (ADR-0072, P7).

The door is stateless (no `Mcp-Session-Id`): every request is checked and served by its own key's server, so a session opened
by one key cannot be used by another.
