# door

The door outside systems post event signals to (`POST /hooks/{name}`, ADR-0024): a channel like the browser's socket, thin
on purpose. A post is let in only through the door of an event signal the owner made, by the name in its address, and only
with that signal's own key (GitLab's `X-Gitlab-Token`, a bearer token, or `?token=`) or its body signed with it (GitHub,
Stripe, Slack, Grafana; a signed stamp more than five minutes from now is a replay). Every refusal reads the same, so a sender
learns nothing about which names exist. A Slack `url_verification` post is answered its challenge and kept as no event.

The door takes the key out of what it passes on (credential-looking headers, the `token` query parameter) and hands the rest
to Remit (`orch::Orchestrator::emit`), which keeps it as an event. Header names are passed in canonical form, each word
capitalised (`X-Gitlab-Event`), since a subscription's `when` reads them that way.

`door::router(keys, emitter, clock)` is the axum router the binary mounts; `Orchestrator` implements both `Keys` (key and
signature checks, `orch::KeyProof`) and `Emitter`.
