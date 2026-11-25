// What the owner sees of the keys he handed to outside agents, and how an agent holds one.

// The sentence the owner finishes with a name when he hands a key out through the Assistant.
export const NEW_CLIENT_KEY_WORDS = 'Create a client key named ';
// The key is never held where it is not shown: this stands in for it, with its prefix and none of its characters.
export const CLIENT_KEY_MASK = 'ck_••••••••••••';
// Where the endpoint is on the machine the server runs on: it answers that machine only.
export const CLIENT_ENDPOINT = '/mcp/client';
