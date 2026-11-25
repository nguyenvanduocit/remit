# The Paseo bridge: people take part in Remit from their own Paseo, signed in with Google

Status: design, 2026-11-03. Source: the owner's words of 2026-11-03:

- "For now we lean on Paseo and its plugins: design the mechanism and the UI/UX that use Paseo as the bridge that brings people
  into the system's loop."
- "A Paseo plugin can even load a script into the daemon with a Node import, so it can do a great deal; look at
  paseo-on-compass."
- "Do the Google login now, and use that identity to identify the human employee on the server. Through the Paseo plugin's
  client, show popups that ask the person and show the tasks assigned to them, because people cannot reach Remit's UI: it is
  anonymous to them, they are only a part of the system."

Builds on the human-employees design (`2026-11-03-human-employees-design.md`, "the spec" below), ADR-0051 and ADR-0052, built
through slice 2a.

## Intent

A person is a part of the system, not a user of it. Remit gives them work and asks them things; they answer from where they
already are. They never open Remit, never see its name, its chains, its Chairs or its other employees. What reaches them is
what an employee needs to do the work: the task, who it is for, what done means, by when, and the messages of the person
leading it.

Each person has an always-on Paseo host (`paseo-<handle>.example.com`, a pod per handle, `paseo-on-compass/README.md`) that
they reach from the Paseo app on desktop and phone, where they already run their Claude Code and Codex agents. Remit
reaches them there, through:

1. **A Paseo plugin, `work`**, that shows the person what is assigned to them and what they are asked, as popups over the app,
   a list, and one tab per task, and carries their answers back.
2. **A daemon shim**, loaded into the Paseo daemon with `NODE_OPTIONS=--import` as the team's `team-access` shim is, for what the
   plugin API does not give: which app is calling and whether the person is looking.
3. **Their Google account** as their identity. Remit verifies it itself; the owner names the account when he hires them.

Remit's core names no Paseo (AGENTS.md). The server speaks the spec's generic worker protocol and Google's OAuth; all Paseo
code lives in `paseo/` in this repository. The spec's CLI and Claude Code plugin become later channels on the same protocol.

## Goals

1. A person signs in once per device with their Google account, from a popup in Paseo, and Remit knows them as the person
   employee the owner hired with that account.
2. What is assigned to them and what they are asked appears as a popup over whatever they are doing in Paseo, stays in a list
   until they act, and reaches their phone as a push when Paseo is closed.
3. They accept or decline work, answer, talk to the person leading it, work with their own AI agent, and hand in, all from Paseo.
4. Only the signed-in person, from their Paseo app, makes their acts. A request from an agent or a shell on the host (an agent
   acting on its own initiative or by mistake), or from an app nobody signed in on, is refused. An agent set on impersonating
   the person is the hardening named under *Revisit when*.
5. What their AI does in the agent bound to a task reaches the card's screen in Remit as its transcript.
6. The owner sees, in Remit, whether each person is in Paseo now and on which device, what they hold, and where each
   message to them stands (queued, delivered, seen, forwarded).

## Non-goals

- Any screen of Remit for people. The owner's web app stays the owner's.
- A CLI or a Claude Code plugin (later channels, same protocol).
- How a person's pod reaches the server. Devops provides a URL a team pod reaches; the host image hands it to the plugin.
- Team chat as a channel, and Paseo Hub triggers.
- Patching Paseo's files. The shim changes the daemon at load time only, as `team-access/shim/owner-shim.mjs` does.

## Anti-goals

- A person seeing the product's name, a chain, a Chair, another employee, the owner's dashboard, a transcript or the server's
  address. The plugin shows only the person's own work.
- Server code that names Paseo, or a rule that reads which channel a person uses.
- Trusting a name the host asserts. The server learns who a person is from Google, never from the plugin's word.
- A message from the lead reaching the person's AI without the person forwarding it.
- The plugin or the shim holding state Remit needs. Both rebuild from the server after a reinstall or a pod restart.
- A notification path beside Paseo's. Every alert is a turn of a task tab, which Paseo's own attention logic turns into an in-app
  notification or a phone push, so nobody is told twice.

## Options considered

**The bridge.**

| | Shape | Verdict |
|---|---|---|
| A | A Paseo plugin and a daemon shim on each person's host; the plugin is a client of Remit's `/worker` namespace | **Chosen.** Server stays Paseo-free; later channels reuse the protocol; the pod is always on; the shim gives who calls and who looks |
| B | Remit drives each daemon through the Paseo SDK with the owner's host credentials | Rejected. The owner's credentials to every host inside Remit, a Node sidecar beside a Go server, the person's acts polled back from 14 daemons |
| C | Team chat (`chat_send`, replies as prompts) | Rejected. No structure for offer, answer or hand-in; storage any host can rewrite |

**The Google sign-in.**

| | Shape | Verdict |
|---|---|---|
| G1 | Remit runs Google's OAuth device flow itself: the popup shows a code, the person enters it at `google.com/device`, the server receives the ID token straight from Google | **Chosen.** No Remit page shown to anyone; no redirect URL to expose; the client secret stays on the server; the pod never holds a Google token; works on any device |
| G2 | Reuse team-access's `owner-login` (Firebase page served by a plugin on each pod, 6-digit code back in the app), then the plugin vouches for the email | Rejected. The server would trust the plugin's word for who a person is; every pod needs a login ingress and a Firebase authorized domain |
| G3 | A Remit sign-in page with Google's redirect flow | Rejected. A page of Remit that people open, which the owner rules out |

## Identity

### Who a person is

- `Employee.Email`: the Google account of a person employee, given by the owner when he hires them through the Assistant ("hire
  Lan, lan@example.com, backend"). It is required for a person and refused for an agent, so the type carries the rule. Two
  active people never share one. Employees are stored as JSON documents (`internal/store/employees.go:21`), so a stored person
  reads with an empty `Email` and no schema change; the owner gives each one's account to the Assistant.
- An account the owner has not hired gets "This Google account has no work here", and nothing more: no names, no list.

### Signing in (once per device)

```
 Paseo app (Lan's phone)          Lan's pod: plugin `work`            Remit                    Google
        │  popup: Sign in with Google       │                              │                           │
        │──── signin.start (caller:app, ───▶│── signInStart ──────────────▶│── device/code ───────────▶│
        │      clientId c1)                 │                              │◀── user_code, url ────────│
        │◀── "Go to google.com/device,      │◀─ { signInId, userCode, url }│                           │
        │     enter KQBD-MXTW"              │                              │                           │
        │                                   │── signInWait(signInId) ─────▶│── poll token ────────────▶│
        │  (Lan enters the code, picks      │                              │◀── id_token ──────────────│
        │   lan@example.com)               │                              │  verify, email → Lan       │
        │                                   │◀─ { key: wk_…, name: "Lan" } │  issue key for (Lan, c1)  │
        │◀── "Signed in as Lan"             │  keeps c1 → key (0600 file)  │                           │
```

- **The server runs the flow** with an OAuth client of type *TVs and Limited Input devices*, scopes `openid email`. Its client id
  and secret are Remit flags (`--google-client-id`, `--google-client-secret`), so the pod never holds them. The client is
  created by the owner in the company's Google Cloud project, before P1 runs end to end; its consent screen decides which
  Workspace organisations may sign in. Tests use a fake token endpoint and a test signing key behind the same verification
  function, so P1 is built and tested without it.
- **The server verifies** the ID token it received from Google's token endpoint: signature against Google's keys, `iss`, `aud`
  equal to its client id, `exp`, `email_verified`. It maps the lowercased email to an active person employee.
- **It issues a `wk_` key** (256 random bits, sealed and listed as client keys are, ADR-0009) for that employee and that device,
  and hands it to the waiting `signInWait`, once. `signInStart` and `signInWait` are a start and a wait
  (`docs/superpowers/plans/2026-11-04-employee-runtime/02-worker-channel.md`, Task 6): the owner's approval in the inbox is replaced by Google's answer, because hiring
  the account was the approval.
- **A device is one Paseo app on one host**: the shim's `caller.clientId` (the id the app sends in its `hello`, read by the shim
  as `owner-shim.mjs` does in `trackHello`). The plugin keeps `clientId → key` in `$PASEO_HOME/work/keys.json` (`0600`), beside its cursor and the
  shim's `presence.json`: a file, because a pod has no OS keychain, in a directory whose name says nothing of the product. Lan's
  desktop and phone sign in once each.
- **A host serves one person.** Every engineering host is one person's pod. On a host several people share, task tabs and their
  pushes would reach every app connected to it (Paseo pushes every agent's attention to every registered device,
  `team-access/SECURITY.md`); that case is under *Revisit when*.
- **Signing out** in the popup's menu deletes the key on the server and the host. The owner revokes a person's devices by telling
  the Assistant; retiring a person revokes them all, and every socket of theirs closes.

## The mechanism

```
 Person's phone / laptop                 Person's pod (paseo-<handle>)                         Remit
 ┌──────────────────────┐   Paseo    ┌────────────────────────────────────────┐   Socket.IO   ┌──────────────────────┐
 │ Paseo app            │◀──────────▶│ Paseo daemon worker                    │   /worker     │  /worker namespace   │
 │  · popups            │ heartbeat, │  └ shim (NODE_OPTIONS)                 │◀─────────────▶│  Google sign-in      │
 │  · Work list         │ requests,  │      · tags caller (app/local, client) │  one socket   │  PersonLauncher      │
 │  · task tabs         │ attention  │      · reads app heartbeats            │               │  card queue (outbox) │
 │  · phone push ◀──────┼── Expo ────┤  └ Paseo's attention on tab turns      │               └──────────────────────┘
 └──────────────────────┘            │  └ plugin `work` (subprocess)          │
                                     │      · keys per signed-in app          │
                                     │      · provider `work` (task tabs)     │
                                     │      · tools + transcript of bound     │
                                     │        agents                          │
                                     │  └ work agents (Claude Code, Codex)    │
                                     └────────────────────────────────────────┘
```

### The daemon shim

`paseo/shim.mjs`, loaded into the daemon worker with `NODE_OPTIONS="--import /opt/work-paseo/shim.mjs"` as
`team-access/shim/owner-shim.mjs` is: active only in `daemon-worker`, the variable deleted after load so agents, terminals and
plugin subprocesses never load it, kept in the image's read-only directory so no agent can edit it. It changes no Paseo file.

1. **Tags who calls the plugin.** Every `plugin.rpc.invoke.request` for the `work` plugin gets `caller` written
   into its input, which no client can forge, as `owner-shim.mjs:999` does for the admin plugin: `kind` is `app` for a socket
   that came in through the ingress and `local` for one opened on the pod (an agent's `paseo` CLI, a terminal, `kubectl exec`),
   and `clientId` is the app's id from its `hello`.
2. **Reads what the person is doing.** Each app socket's `client_heartbeat` carries `deviceType` (web or mobile), `appVisible`,
   `focusedAgentId` and `lastActivityAt` (`@getpaseo/protocol/dist/messages.d.ts:4473-4481`). The shim keeps the latest per
   `clientId` and writes them to `$PASEO_HOME/work/presence.json` on change, the way owner-access shares state with its plugins
   through files (`owner-shim.mjs` header).

The shim is written against Paseo 0.15.0's internal names, the image's pinned release. Like `team-access` (`SECURITY.md`,
limit 7), `paseo/tests/shim-smoke.sh <version>` starts a real daemon with the shim and checks both before every Paseo upgrade.
The daemon starts without the shim when it fails to load (it is not a security layer); the plugin then shows *Unavailable on
this host* and does not connect.

### The plugin's parts

| Part | Paseo API | What it does |
|---|---|---|
| Connection | server entry (Node subprocess) | One Socket.IO client to `<url>/worker` per signed-in person on the host, with their key. The URL comes from the image's environment; nobody types it. Resumes each stream from the last event id; on reconnect asks `tasks` again. Its memory is a cache of the server. |
| Sign-in | client entry + RPCs `signin.start`, `signin.wait`, `signout` | The sign-in popup (below). |
| Popups | `addSidebarHeaderItem` component polling RPC `inbox` every 3 s + host `Modal` | A window over the app for what needs the person (below), the way `ask-person` mounts its question window from its sidebar row (`ask-person/index.client.tsx`, `client/inbox.tsx`). Paseo 0.15 has no plugin subscriptions (`ref.md`, *Older hosts*), so the client polls as `ask-person` does. Only an app signed in as a person gets its inbox; others get the sign-in popup. |
| Work list | `addScreen` + `addSidebarHeaderItem` | **Work** in the sidebar with the count of what waits for the person; opens their list. |
| Task tabs | `registerProvider` (`work`) | One agent tab per task in the workspace `~/Work`, the pattern the team's chat plugin uses for conversations (`chat-plugin/server/provider.ts`). The tab is the task's thread; what the person types there goes to the lead. |
| Acts | popup buttons, `addComposerPill`, `addSlashCommand`, host `Modal` | Accept, decline, answer, message the lead, start work, bind, forward, hand in, verdict, dispute, availability. Each RPC is refused unless `caller.kind` is `app` and `caller.clientId` is signed in; it acts as that person. |
| Work agent | `paseo.workspaces.create` + `agents.create` | Start work makes a workspace and an agent and sends `bind`. **Bind to a task** (⌘K, agent context) binds an agent the person already runs. |
| Worker tools | `addHook("agent.open.before")` + `defineAgentTool` | An agent the server lists as bound to a card gets `update_plan`, `ask_lead`, `remember`, `recall`, `send_feedback` and `draft_report`, each a pass-through to `toolCall` on that card (`ask_lead` is the card's `ask_chair`). No tool accepts, answers, forwards, hands in or judges. Needs `daemon.mcp.injectIntoAgents: true`, which the image sets. |
| Transcript | `addHook("agent.turn.after")` + timeline refetch; tool calls live | See *Transcript keys*. |
| Presence | reads `presence.json` | Sends each signed-in app's presence to the server on change. |
| Attention | provider `session.turn` | Each event that needs the person arrives in its task tab as a turn (`started`, the event's message, `completed`), as the chat plugin's tabs do (`chat-plugin/server/provider.ts:156-167`). Paseo then marks the tab, notifies the device in use, or pushes to the phone when no app is active, with the event's text as the body (the tab's last assistant message, `@getpaseo/protocol/dist/agent-attention-notification.js`). A tab whose session is closed (after a restart, or never opened) is opened first with `paseo.agents.ref(id).reload()`. |

### What the server adds to the spec's slice 2b

- **Google sign-in** as above: `signInStart`, `signInWait`, `signOut` on `/worker` (the first two callable without a key), the
  flags and `Employee.Email`. Migration 74 creates the key and sign-in tables. A sign-in waits at most the device code's lifetime; at most
  3 open per host address and 20 server-wide. This replaces 2b's pairing: no pairing table, no inbox item.
- **What a person may see.** The worker contract's `Task` is the person's view, built by one function on the server: the task's
  title, who it is for (the chain's customer, ADR-0016), the brief written for a person, deliverable, criteria, inputs as their
  content, repositories, deadline or ETA, state, and the thread. It carries no chain title, no card name of another employee, no
  agent name and no owner name. Messages to the person come from **the lead**, whoever wrote them (their Chair or the owner).
- **`bind(request, cardId, session)` and `unbind(request, cardId)`**: which agent of the person's works on the card. `accept`
  keeps its `session` argument for accepting and starting in one press. The plugin decides which agents get worker tools and
  which timelines go up from `tasks`, never from a label of its own.
- **`say(request, cardId, text)`**: the person's message to the lead, from a popup or a task tab, before or after accepting, a
  question or an answer. It reaches the Chair through `o.AskChair` (`internal/orch/tools.go:381`) as "[from Lan · card …]", and
  is recorded on the person's card so the thread rebuilds from the server. The agent's `ask_lead` reaches the Chair as "[from
  Lan's AI · card …]". Neither ends the turn.
- **`answer(request, cardId, text)`**: accept and hand in at once, for an offer whose deliverable is an answer. It is `accept`
  with an ETA of now followed by `reply`, in one change.
- **`presence(request, devices)`**: each signed-in app the person has open: `deviceType`, `appVisible`, the card whose tab is
  focused, `lastActivityAt`. Kept in memory, never stored. The plan's `seen(request, cardId, eventIds)` stays the one way a
  message becomes *seen*: the plugin calls it when a popup shows the message or its tab is focused in a visible app.
- **Changes to the worker contract are additive** (a new call, a new optional field) and keep its version, so a server release
  never cuts people off before their host image is updated. A change that is not additive bumps the version and ships with the
  image rollout that carries the matching plugin.

The rest of 2b stands: the `/worker` namespace, ordered resumable events, calls that act once, `forward` delivery marks,
`draft` and `reply`, `verdict`, `availability`, `transcriptAppend`, `toolsList` and `toolCall`. The CLI and pairing tasks of 2b
leave the slice. The Go client `internal/workerclient` stays: the server's tests drive `/worker` with it.

### Asking a person

A Chair asks a person something by putting them to work on a card whose deliverable is the answer (`hire_agent` with
`employee`, ADR-0052): one concept, the card, for work and questions alike. The person sees it as a popup with **Answer now**
(`answer`), **Accept** to answer later, and **Decline**. A lead's message on a task they hold pops up with **Reply** (`say`).

### Transcript keys

ADR-0051 keeps one transcript event per entry id, replaced when the same id is sent again. Paseo gives a stable identity only to
tool calls (`callId`) and plugin rows (`@getpaseo/protocol/dist/timeline-identity.js`); messages carry an optional
`messageId`, and reasoning, todo and error items carry none (`agent-types.d.ts:309-331`). So:

- **Tool calls go up live**, keyed `<agent>/<callId>`, sent again as their status changes.
- **Every other item goes up when its turn ends** (`agent.turn.after`): the plugin refetches the turn's items and keys each
  `<agent>/<turn>/<n>`, where `<turn>` is the `messageId` (else `clientMessageId`) of the user message that opened the turn and
  `<n>` the item's position in it. A finished turn's items do not change, so a resend replaces, never doubles.
- The transcript starts at the bind. Earlier, unrelated conversation stays on the person's host.

PoC before relying on it: every user message of a Claude Code and a Codex agent on Paseo 0.15 carries `messageId` or
`clientMessageId`. If one does not, `<turn>` is the hook's `turnId` (`@getpaseo/plugin/dist/server/lifecycle.d.ts:100`) with the
turn's start time, since a `turnId` can repeat after a session reopens.

## The person's UI in Paseo

Copy follows the team's other plugins' language. The product's name appears nowhere; the plugin's name is **Work**.

### Sign in (a popup, once per device)

```
┌ Sign in to see your work ───────────────────────────────┐
│ Use your company Google account.                         │
│                                                          │
│   1. Open google.com/device          [Open]              │
│   2. Enter   KQBD-MXTW               [Copy]              │
│                                                          │
│   Waiting for Google…  · expires in 14:32                │
│                                              [Later]     │
└──────────────────────────────────────────────────────────┘
        ↓ when Google answers
┌ Signed in as Lan (lan@example.com) ────────────────────┐
│ Work assigned to you will appear here.          [Done]   │
└──────────────────────────────────────────────────────────┘
```

**Later** closes it; the sidebar row reads *Sign in to see your work* until they do.

### Popups: what needs the person, over whatever they are doing

```
┌ New task · answer within 24h ───────────────────────────────────────┐
│ Billing API: idempotent refunds                                      │
│ For: merchants refunding an order twice by mistake                   │
│ ──────────────────────────────────────────────────────────────────── │
│ Make refunds idempotent per key, so a retried refund never charges   │
│ twice. …                                                    [More]   │
│ Done when: 1. … 2. … 3. …                                            │
│ Repository: gitlab.com/acme/billing                                  │
│                                                                      │
│ [Decline]                          [Open task]  [Accept…]            │
└──────────────────────────────────────────────────────────────────────┘

┌ Question · answer within 4h ────────────────────────────────────────┐
│ Which payment provider does the Shopee store use for refunds?        │
│ ┌──────────────────────────────────────────────────────────────────┐ │
│ │                                                                  │ │
│ └──────────────────────────────────────────────────────────────────┘ │
│ [Decline]                                        [Answer now]        │
└──────────────────────────────────────────────────────────────────────┘

┌ Lead · Billing API: idempotent refunds ─────────────────────────────┐
│ backend!42 at v2 is the API, build on it.                            │
│ [Reply…]                    [Forward to my agent]   [Open task]      │
└──────────────────────────────────────────────────────────────────────┘
```

- A popup opens for: a new task or question, a message from the lead, a hand-in sent back, a task cancelled with its grace
  window, and the sign-in. Several queue one after another.
- Closing it with ✕ means *later*: it stays in the Work list and the sidebar count, as `ask-person`'s questions stay in its
  sidebar row (`team-access/plugins/ask-person/OVERVIEW.md`).
- The same event runs as a turn of the task's tab, so a person with Paseo closed gets the push and the popup is waiting when the
  app opens.

### The Work list

```
Sidebar                         Work (screen)
┌──────────────────┐            ┌──────────────────────────────────────────────────────────────┐
│ ◆ Work         2 │            │ Your work                                   Lan · lan@example.com│
│ ◎ Chat         5 │            │                                                              │
│ ...              │            │ ▸ New · answer within 18h                                    │
└──────────────────┘            │   Billing API: idempotent refunds    task                    │
                                │ ▸ Sent back · 2 notes                                        │
                                │   Retry banner copy                  task                    │
                                │ ▸ Working · due Fri 17:00 · hand-in owed                     │
                                │   Checkout: tax line                 task                    │
                                │ ▸ Handed in · waiting for review                             │
                                │   Refund webhook                     task                    │
                                └──────────────────────────────────────────────────────────────┘
```

One list of one kind of row, ordered by what needs the person first. Each row's state is text. Pressing a row opens its tab.

### A task tab (the task's thread)

```
Task: Billing API: idempotent refunds                                      [Accept…] [⋯]
────────────────────────────────────────────────────────────────────────────────────────────
┌ New task · for merchants refunding an order twice by mistake ───────────────────────────┐
│ Make refunds idempotent per key… · Done when: 1. … 2. … 3. … · Due: answer within 24h   │
└───────────────────────────────────────────────────────────────────────────────────────────┘
  Lan: Is the v1 endpoint still in scope, or only v2?
┌ Lead ────────────────────────────────────────────────────────────────────────────────────┐
│ Only v2. v1 is frozen until the migration ends.                                           │
└───────────────────────────────────────────────────────────────────────────────────────────┘
┌ Accepted · due Fri 17:00 ────────────────────────────────────────────────────────────────┐
┌ Lead ─────────────────────────────────────────────────────────── [Forward to agent] ─────┐
│ backend!42 at v2 is the API, build on it.                                                 │
└───────────────────────────────────────────────────────────────────────────────────────────┘
────────────────────────────────────────────────────────────────────────────────────────────
 [◆ My agent: Claude · running] [Hand in] [⋯]
 > Message the lead…                                                    /accept /decline /handin
```

- Everything from the server is an assistant message of the tab's provider, a plain line that names its sender ("Lead: Only
  v2…"), its `messageId` the server's event id; a timeline transformer (`addTimelineTransformer`) draws it as the row above. The
  plain text is the notification's body. What the person typed is their own bubble. The provider rebuilds the thread from the
  server whenever its session opens.
- The composer messages the lead (`say`) and never hands in.
- Pills follow the task's state: *Accept…*, then *Start work*, then *◆ My agent*, and *Hand in* while a hand-in is owed. **⋯**
  holds Decline, Dispute, Bind an agent, Unbind. Slash commands mirror them: `/accept <due>`, `/decline <why>`, `/handin`,
  `/dispute <why>`, `/away <until>`.

### Accept and start work

```
┌ Accept "Billing API: idempotent refunds" ──────────────┐
│ When will you hand it in?                               │
│ ( ) Today 18:00   (•) Fri 17:00   ( ) Pick…             │
│                                                         │
│ Start work now with   [Claude Code ▾]                   │
│ [x] Send the task as the agent's first prompt           │
│                                   [Cancel] [Accept]     │
└─────────────────────────────────────────────────────────┘
```

With **Start work now** the plugin creates the workspace (a worktree of the task's repository at its current head, see *Code*;
the person's home directory for a task with no repository) and an agent of the chosen provider, and binds it in the same
`accept`. An agent bound while running gets its tools when idle: the plugin reloads it then, never mid-turn.

### The work agent

```
Claude · Billing API: idempotent refunds                        (an ordinary Paseo agent tab)
  …the person and their AI work as they always do…
┌ ◆ Lead · forwarded by you ─────────────────────────────────────────────────────┐
│ backend!42 at v2 is the API, build on it.                                       │
└─────────────────────────────────────────────────────────────────────────────────┘
 [◆ Task: Billing API · hand-in owed] [Tasks] [Subagents]
```

A forwarded message is sent to the agent as a prompt (`agent.send`) and recorded with `forward`. With `draft_report` the agent
writes the report the person will hand in.

### Hand in

```
┌ Hand in "Billing API: idempotent refunds" ─────────────────────┐
│ Report (drafted by your agent 4 min ago, edit freely)           │
│ ┌──────────────────────────────────────────────────────────────┐│
│ │ 1. Refund is idempotent per key: MET, test refund_test.go:88 ││
│ └──────────────────────────────────────────────────────────────┘│
│ Commits: 3 on top of a1b2c3d (slice P3)                         │
│                                          [Cancel] [Hand in]     │
└─────────────────────────────────────────────────────────────────┘
```

**Hand in** is `reply` on the draft's revision. A reviewer's or QC's dialog has **Approve** and **Request changes**, pinned to
the head they reviewed (`verdict`).

### Code

Slice 3 of the spec (bundles, one writer per repository) applies with the plugin as the device: one clone per repository on the
pod (`~/Work/repos/<remote-path>`), the work workspace a Paseo worktree on the head it received, the person's commits bundled
on hand-in. Until P3 ships, a person takes tasks with no repository: questions, research, a decision, a review of an MR they
open in their own forge account.

## What the owner sees in Remit

- **Crew.** A person's row: person icon, the word *person*, their Google account, *in Paseo now · phone*, *active 2h ago* or
  *away until Fri*, *not signed in yet*, and their open tasks across chains.
- **A person's card screen.** Each message to the person with its delivery state as text (*queued*, *delivered*, *seen*,
  *forwarded*); the person's messages; transcript entries marked *Lan* or *Lan's AI*; offer state and due time.
- **Hiring.** "Hire Lan, lan@example.com" to the Assistant. Revoking devices and retiring, the same way.
- **Chair line.** "Waiting on Lan (in Paseo now)" (`peopleAtWork`, `agentstanding.go:253`).

## Delivery and install

- `paseo/` holds the plugin and the shim. The plugin's contract types are generated from `web/src/rpc/worker/define.ts` by
  `make gen`.
- Both ship in the paseo-on-compass host image at `/opt/work-paseo/`, as `/opt/chat-plugin` does: the shim must sit in a
  read-only directory named in `NODE_OPTIONS`. The plan carries the image changes as a task in paseo-on-compass: the
  `Dockerfile` copies `paseo/`, `entrypoint.sh` adds the shim to the daemon's `NODE_OPTIONS`, `seed-paseo-config.py` registers
  the plugin, `k8s/host.yaml.tmpl` passes the server URL. Building and rolling the image out is a step the owner confirms when
  it comes.
- `requirements.paseo` is `>=0.15.0`, the image's pinned release.
- Checks: `npm test` (provider, popups' queue, tools, transcript keys, over a fake server); `test:runtime` (the pinned release
  compiles both bundles and lists provider, RPCs and tools); `tests/shim-smoke.sh <version>` (a loopback client is
  tagged `local`, a non-loopback one `app` with its `clientId`; a heartbeat reaches `presence.json`); a check that an event for a
  task tab whose session is closed reaches a client as `agent_attention_required`; `test:daemon` (a loopback daemon with both, against the dev Remit on
  4320 with Google replaced by a fake token endpoint the tests own: sign in, offer, popup, accept, say, bind, forward, hand in,
  accepted). Server tests verify ID tokens signed by a test key behind the same verification function.
- Looking at it: the Paseo web app against that daemon, at desktop and phone width, in both themes; every `Text` coloured from
  `theme.colors`.

## Slices (replacing the spec's 2b and the plugin half of slice 6)

| # | Ships | Done when |
|---|---|---|
| P1 | Server: Google sign-in, `Employee.Email`, the person's view of a task, 2b minus CLI and pairing, plus `bind`, `say`, `answer`, `presence`. Shim: caller, presence. Image changes in paseo-on-compass. Plugin: sign-in popup, popups, Work list, task tabs, accept, decline, answer, say, hand in (report only) | Lan signs in with Google on a phone-width window, a question popup reaches her, she answers; a task's push reaches her with Paseo closed, she accepts, messages the lead, hands in and sees *accepted*; the same RPC from an agent's `paseo` CLI on the pod or from an app not signed in is refused; the popups show no chain, Chair or product name |
| P2 | Plugin: start work and bind, worker tools, transcript, forward, the *◆ Task* pill | The card screen in Remit shows the person's AI's transcript; a forwarded message reaches the agent only after the press |
| P3 | Spec slice 3 (bundles) with the plugin's clone and worktree | A person's commits land in the chain's worktree and pass the same review and QC gate as an AI's |
| 4, 5 | Spec slices 4 and 5; the plugin renders dispute, cancellation and the grace window | as the spec |
| 6 | Spec slice 6's Remit surfaces (Crew, card screen) with account, presence and delivery states | as the spec |

## Changes to the spec this design makes

- Intent: a person sees no Remit screen; their only surface is the channel's, and the server decides what they see.
- *The plugins*: Paseo (a plugin and a daemon shim) is the first channel; the CLI and the Claude Code plugin are later channels.
- Decision 2: a person employee carries `Email`, their Google account, given at hire.
- Decision 4: "whether their plugin is connected" becomes presence from the person's signed-in apps.
- Decision 5: a person signs in with Google through the server's device flow; one key per signed-in app; no device pairing and no
  pairing item in the owner's inbox.
- Decision 6: adds `signInStart`, `signInWait`, `signOut`, `bind`, `unbind`, `say`, `answer`, `presence`; messages to a person
  come from *the lead*.
- Decision 7: the person's acts are refused unless their signed-in Paseo app made them.
- Decision 9: the transcript starts at the bind.
- Decision 15: no pairing requests in the inbox.
- Slice order: 2b becomes P1 and P2.

## Revisit when

- **An agent must not be able to act as the person at all.** The plugin refuses acts from anything on the pod, but an agent with
  a shell can read the key file and speak `/worker` itself. The seam: the person's acts are one named set on the server, so a
  signature from a key the Paseo app keeps on the person's device can be required for that set alone.
- **A Google account is suspended or leaves the company.** Today the key lives until the owner revokes it. The seam: the server
  keeps the account per key, so a periodic check against Google (or a short key life renewed by a refresh token) goes in one
  place.
- **Accounts outside the company sign in.** The server accepts any verified account the owner hired; a hosted-domain rule (`hd`)
  is one check in the verification function.
- **People without a Paseo host** join: the CLI and the Claude Code plugin speak the same contract and sign in with the same
  device flow.
- **Several people share one host.** Task tabs and their pushes need a per-person owner, which `team-access` gives where it runs:
  the plugin records each task tab's owner there, and its shim hides the tab and its pushes from everyone else.
- **Paseo exposes callers or heartbeats to plugins**: the shim's part moves into the plugin and the shim goes. A Paseo release
  with plugin subscriptions replaces the popups' polling.

## Calls made in this design, not by the owner

- The server runs Google's device flow (G1); the pod never holds a Google token or the client secret.
- A device is one Paseo app on one host (`clientId`); each signs in once.
- Everything addressed to a person comes from *the lead*, whether their Chair or the owner wrote it.
- Asking a person is putting them to work on a card whose deliverable is the answer; **Answer now** accepts and hands in at once.
- Every alert is a turn of a task tab, so Paseo's own attention does in-app notification and phone push; popups show what needs
  the person.
- The person's acts are guarded against an agent acting by mistake or on its own; a deliberate impersonator is later hardening.
- Which agent works on a card is the server's record (`bind`); the transcript starts at the bind.
- The plugin is named **Work**; the plugin and shim ship in the host image; worker contract changes are additive.
