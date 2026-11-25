# 0089. A mentor works from the web app, as a scoped user of the owner's contract

Status: accepted, 2026-11-12. Supersedes ADR-0056 (the Work plugin in Paseo); amends ADR-0060 (a code is pasted into, or
carried by a link to, Remit's own sign-in page) and ADR-0072 P14 (`remit-client worker`, its JSON lines and `wire::worker`
are gone).
Source: the owner's words of 2026-11-12 (translated): "today the system is for the owner alone; I want mentors to use it
too. They see something like a kanban, and clicking one of their tasks opens a chat to work in." "We no longer use Paseo,
delete it." "Make the kanban, and when I click a task show it exactly like the owner's Assistant: reuse the UI and the
layout." "Which way is right, long term and cleanest?"

## Context

A mentor (ADR-0062) worked from a code the owner had made for them (a worker key, `wk_`, ADR-0060) over a contract of its
own, `wire::worker`, spoken only by the Work plugin in Paseo. The owner no longer uses Paseo. The web app holds one client
core (`crates/replica`) and one set of screens over `wire::app`: the board's columns and cards, and an agent's drawer
(`AgentDrawer`: its stream, where it stands, its composer) that the Assistant's panel and a chain's page both use. A second
contract for mentors means a second core, a second set of screens and a second copy of every rule about what a card shows.

## Options

1. **A mentor's own contract and core beside the owner's** (`wire::worker`, `replica::Mentor`, `Mentor*.vue`). Built first
   and rejected on sight: its board and its chat looked like nothing else in the app, and every screen would be built twice.
2. **One contract; a session's principal decides what it may call and what it is pushed.** A mentor is a scoped user of
   `wire::app`: the same core, the same stores, the same columns, cards and drawer. **Chosen.**
3. **A separate mentor app**: a second bundle to build, ship and hold pages to.

## Decision

- **Who a session is.** `/link` admits the owner key or a worker key (`orch::authorize_worker_key`): the session's
  principal is the owner, or a mentor (their id and the key's). Both are held to the bundle the server serves. A revoked
  key closes its sessions.
- **What a mentor may call: deny by default, by construction.** A mentor's session is answered by `rpc::MentorApi`, a
  second `wire::app::Handler`: every call it does not name refuses (`PERMISSION_DENIED`), and the calls it names are
  answered by the orchestrator's mentor functions, which read the mentor's own helps on every call: `app_snapshot`,
  `chain_get` and `card_events` (scoped below), `draft_save` and `draft_delete` (the mentor's own drafts), `card_prompt`
  (a card of an accepted help, sent as `by_mentor(help)` with the mentor's header), and `help_accept` and `help_decline`,
  which only a mentor may call.
- **What a mentor sees.** The snapshot says who the session is (`Snapshot::who`). A mentor's carries no Assistant (it is
  the owner's: `Snapshot::assistant` is absent) and holds the chains of their helps and a board of three columns, Asked, Helping and Done, of those chains (`orch::mentor_board`), each chain's standing
  the mentor's own (what is asked of them, by when). A chain's view holds the cards they help, the decisions about those
  cards, their helps and drafts, and where those agents stand, with only `message` among what they may ask of one, while
  their help is accepted.
- **What a mentor is pushed.** Every push names its audience (`rpc::Audience`): the owner's, a chain's, a card's, or one
  mentor's. A mentor's session is pushed what is addressed to the chains and cards of their helps or to them, a standing
  scoped as above, and its board laid out again whenever a help of theirs moves; nothing addressed to the owner.
- **Drafts have a writer.** A draft is the owner's or one mentor's (`Draft::mentor`, empty for the owner's): a box holds
  one draft of each writer, and a writer reads, saves, sends and is pushed only their own.
- **The screens are the owner's.** A mentor's app is the board (`BoardColumn`, `ChainCard`) and, beside it, the helped
  card's drawer (`CardDrawer` over `AgentDrawer`), with Accept and Decline in its actions while the help is offered.
- **The agent asks.** When a mentor accepts, the card's agent is told to put to its mentor, in its next message, what it
  needs from them, so the mentor's chat opens with the agent's question.
- **Removed.** `wire::worker`, `crates/workerrpc` and its routing, `replica::Mentor` and `client-web`'s `MentorClient`, the
  `Mentor*.vue` screens, the apps' own presence reports (`set_presence`, a code's `on`), `paseo/`, `remit-client worker`, `make work-paseo` and the release's `work-paseo.tar.gz`.

## Consequences

- A mentor needs only a browser that reaches `/link`.
- Every new call is refused to a mentor until `MentorApi` answers it, and every new push needs an audience before it
  compiles.
- A mentor is in the app while a session of theirs is open (`device_online`); which device they are on is not known, and
  stated availability, which the Paseo app set, is no longer set.
- The owner and a mentor on one browser share its one credential: signing in as one signs the other out.

## Revisit when

- Mentors need to be told while the app is closed: Web Push behind the same worker key.
- A mentor reaches the server from outside the owner's network: TLS, the address they dial, and rate limits on `/link`
  for worker keys are the hardening not built yet.
- A mentor should state when they can help: a mentor-only call setting `Mentor::availability`.
