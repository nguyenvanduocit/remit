// What a writer needs to write to an event signal through its door, and what the owner sees of the key that lets it in.

// The key is never held where it is not shown: this stands in for it, with its prefix and none of its characters.
export const KEY_MASK = 'evk_••••••••••••';

// Where on the server a writer posts an event of a signal: the signal's name is the last part of the path, and a post may
// carry a query.
export const pathOf = (name: string, query?: string): string => `/hooks/${name}${query ? `?${query}` : ''}`;
// The whole URL a writer posts to.
export const addressOf = (origin: string, name: string): string => origin + pathOf(name);

// A post to the door the signal lets in: its key as a bearer token, any JSON as the body.
export const curlOf = (address: string, key: string): string =>
  [`curl -X POST '${address}' \\`, `  -H 'Authorization: Bearer ${key}' \\`, "  -H 'Content-Type: application/json' \\", `  -d '{"event":"test"}'`].join('\n');

// The same event written by `remit emit` (crates/remit/src/emit.rs) from a script or a shell: the server it posts to, the key
// from a file only its owner can read (never an argument, which every user of the machine can list), the body from a file.
export const keyFileOf = (name: string): string => `~/.remit/${name}.key`;
export const emitOf = (origin: string, name: string): string => `remit emit -name ${name} -url ${origin} -key-file ${keyFileOf(name)} event.json`;
