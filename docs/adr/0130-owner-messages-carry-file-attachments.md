# ADR-0130 · Owner messages carry file attachments

Status: accepted. Supersedes ADR-0111.

## Context

The owner sends documents, archives, data files and images to agents from the same composer. The Claude Code message protocol takes inline images. Other formats need a file the agent can read with its tools.

## Options

- Encode every format in the prompt text. This makes binary data hard to use and spends context on encoding.
- Store attachments by reference and stage files for the receiving session. This preserves bytes and lets the agent select the tool for the format.

## Decision

`attachment_put` accepts any file type up to 5 MiB per upload. A draft names uploaded files by reference, validated against the upload in its home. Sending or withdrawing a message carries those same references. Migration 0131 moves existing images and their draft and event references to the attachment shape, including packed prompt events.

Supported browser images retain their inline preview and image fitting. The runtime receives other attachments with the prompt over contract 5 and stages them as read-only files outside the checkout. Both prompt and steer give the agent their paths. Staged copies last for the Claude Code process's session. The original remains in the home's attachment store.

## Revisit when

- Messages need to reference attachments from an earlier process. Add a card-scoped resolver for stored attachments that stages them on demand.
- Attachments require stronger host isolation. Scope filesystem visibility per agent session at the launcher boundary.
- Mentors need uploads. Scope the RPCs to accepted helps before granting access.
- Uploads need to exceed 5 MiB or abandoned uploads accumulate. Use the upload RPC boundary to move bytes to a streaming transport and add reclamation for unreferenced files.
