# 0111. An image in a message is stored once and named by reference

Status: accepted

## Context

The owner shows an agent what he sees (a screenshot of a broken page, a chart, a design) by putting an image in his
message. Claude Code reads images as content blocks of a user message on its stdin (`{"type":"image","source":{"type":
"base64","media_type":…,"data":…}}`, the Messages API's `ImageBlockParam`). Before this, every layer from the owner's draft to
that frame carried one `String`.

A draft is saved as the owner types and pushed whole to every page (`draft_updated`); a message is an event pushed whole
(`event_added`, `event_updated`). A link peer that falls 4 MiB behind on pushes is closed (`crates/link/src/conn.rs`). An
image is up to a few MiB.

## Options

1. **Bytes inline** in `Draft` and the message event's data. One shape, no new table, but every keystroke re-pushes the
   images, every page that shows the chain is sent them, and two images pass the push limit.
2. **Files in the chain's work dir**, named in the text for the agent to `Read`. The agent sees the image only if it chooses
   to read it, the runtime may run on another machine than the files, and the Assistant has no work dir.
3. **Bytes stored once by id, records carry references.** A new `images` table; `Draft.images` and `EventData.images` hold
   `ImageRef {id, mediaType}`; the bytes travel only to the runtime with the turn (`model::Prompt`) and to a page that shows
   one (`image_get`).

## Decision

Option 3.

- The channel fits an image to what a model reads (long side at most 1568 px, PNG/JPEG/GIF/WebP), puts it with
  `image_put(id, home, image)` under an id it makes (idempotent: an id put again keeps its first bytes) and names it in the
  draft. The server refuses an image over `IMAGE_MAX_BYTES` (5 MiB) and a draft naming an image not put in its home.
- Sending the draft copies its references into the message's data; taking the message back returns them to the draft.
- When the agent takes the message (a turn, or a steer), `chains::images::prompt_of` reads the bytes and hands
  `model::Prompt {text, images}` to `agent::Session::prompt`/`steer`; the runtime contract carries it (`RUNTIME_CONTRACT` 4) and
  `claudecode` writes the text block, then one image block per image.
- An image goes with its home (`HOME_RECORDS`).

## Consequences

- A draft and a message stay small whatever they hold; a page reads an image's bytes once, by id.
- Core and runtime must speak contract 4: they ship in one binary.
- Mentors cannot put or read images yet (`image_put`/`image_get` are denied on the mentor channel).

## Revisit when

- A mentor needs to send images: scope `image_put`/`image_get` to the homes of the cards their accepted helps are on.
- Stored images grow large: images put but never sent (a draft deleted, a picture removed from the box) stay until their
  home goes; a sweep of images no draft or event names is the control not built yet.
- An agent needs to send images back to the owner: the same table and `image_get` serve it.
