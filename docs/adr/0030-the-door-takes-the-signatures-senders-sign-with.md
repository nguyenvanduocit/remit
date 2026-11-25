# 0030. The door takes the signatures senders sign with, and refuses a replay

Status: accepted, 2026-11-02. Source: the owner's principle of 2026-11-02 that outside systems of any company must be reachable without
forking the engine (`docs/remit-design.md`, K5), ADR-0024 (the door), ADR-0029 (extensions).

## Context

The door lets a post in with the event signal's own key: in GitLab's header, a bearer token, the query, or GitHub's HMAC of the body
(`X-Hub-Signature-256`). Payment and chat systems (Stripe, Slack and the ones that sign like them) never send a key: they sign a stamp and the
body, and Slack asks a new address to answer a challenge before it posts anything. A signature of the body alone is also replayable for ever;
these senders put the time in what they sign so that a replay can be told.

## Options

1. Leave it to the sender to be one of the three ways in. A payment system cannot be pointed at the door.
2. A per-sender plugin in the door. The door is a rail (an entrance): code a user writes does not run there.
3. A closed set of signing schemes in the door, each a pure function from the request to the message that was signed. **Chosen.**

## Decision

- `KeyProof` is the key a post carries and a list of signatures, each a message and its HMAC-SHA256 (`orch.Signed`); a post opens the door
  when it carries the key or one signature is of its message made with the signal's key. GitHub's is the body; Stripe's
  (`Stripe-Signature: t=<unix>,v1=<hex>`) is `<t>.<body>`; Slack's (`X-Slack-Signature: v0=<hex>` with `X-Slack-Request-Timestamp`) is
  `v0:<stamp>:<body>`; Grafana's (`X-Grafana-Alerting-Signature: <hex>`, no prefix) is the body, or `<stamp>:<body>` when the owner sets the
  contact point's Timestamp Header to `X-Grafana-Alerting-Signature-Timestamp` (Grafana names none of its own; https://grafana.com/docs/grafana/latest/alerting/configure-notifications/manage-contact-points/integrations/webhook-notifier/).
  The set is closed: a new scheme is a change to the door and its tests.
- A stamped signature counts only when its stamp is within five minutes of now, either way; any other is no proof, and the post is refused
  as any post with a wrong key. The door is given the clock for it, as every part of Remit is.
- A post that only asks the address to prove itself (`{"type":"url_verification","challenge":"…"}`) is answered with its challenge and kept as
  no event, and only after it was let in: an unsigned challenge is refused and never echoed.
- Headers named like a credential (`signature`, `token`, `key`…) are still kept out of the event, so the signatures do not reach agents.

## Consequences

- The owner points Stripe, Slack or a Grafana webhook contact point at `/hooks/<name>` and sets the signal's key as the signing secret (on
  Grafana, the HMAC Signature's Secret, with the Timestamp Header above so a replay can be told); no program of anyone's runs in the door.
- A Grafana post signed without the stamp is replayable for ever, as GitHub's is; one signed with it cannot be made a stampless one, since
  a stamped signature is of the stamp and the body and never verifies as the body's.
- A replay within five minutes still opens the door; the event's sender id (the sender's own delivery number, when it sends one) makes the
  retry the same event.
- Threat model (an entrance an outsider can reach; STRIDE):

| Threat | Control |
|---|---|
| Spoofing: a post claims to be Stripe, Slack or Grafana | The HMAC under the signal's own key over the scheme's message; every failure answers the same 401 |
| Tampering: the body is changed after it was signed | The signed message holds the body; a changed body does not verify |
| Replay: a captured signed post is sent again later | The stamp is part of the signed message and must be within five minutes; a stamp in the future is refused the same |
| Information disclosure: the challenge or a signature reaches an agent or an unsigned caller | The challenge is answered only to a post that opened the door; credential-looking headers are dropped from the event |
| Denial of service: a flood of signed posts | Unchanged: the body is read within a limit, the event cap per signal answers 503 (ADR-0024) |
| Timing: the key is learned by comparing | HMAC comparison and the key comparison take the same time whatever they compare |

## Revisit when

A sender signs with a scheme that is not an HMAC-SHA256 of a message the door can build from the request (an asymmetric signature, a
signature of a header set).

Enforced by: TestADoorTakesTheSignaturesOfStripeAndSlackAndRefusesAReplay, TestADoorTakesGrafanasSignatureAndRefusesAReplayOfAStampedOne, TestADoorAnswersASignedChallengeAndKeepsNoEventOfIt, TestAPostIsLetInOnlyWithTheKeyOfTheSignalItNames, TestABodySignedWithTheKeyOpensTheWebhook
