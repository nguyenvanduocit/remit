# Product peers: what Remit can learn about UI and UX

Research date: 2026-11-11. Question: which UI/UX features and principles of the products nearest to Remit should Remit take,
and which should it refuse. The engineering side of the same field is `docs/research-codex/notes/peers.md`.

## Method

Two passes. The first read four products' code and docs. The second raised the bar: it **ran** two of them and used
them as an owner would, read the docs of nine closed products, and observed Remit's own screens the same way, so every
comparison is screen for screen. The observations are organised by the eight journeys an owner makes. Screenshot identifiers in the observations refer to the private study archive.

| Source | How it was studied | Evidence |
|---|---|---|
| Paperclip 2026.1106.0 (MIT) | Run locally twice (`npx paperclipai onboard`), one agent, one task per pass, a question answered, pause tried | 25 screenshots reviewed, code in `references/paperclip` |
| Vibe Kanban 0.1.44 (Apache-2.0) | Run locally (`npx vibe-kanban`), two tasks on a scratch repo: approval, question, queue, review comments, merge, stop | 18 screenshots reviewed, code in `references/vibe-kanban` |
| Multica v0.8.2 (Apache-2.0 plus conditions; take ideas, not code) | Code, the live landing page, docs, changelog (v0.7.0–v0.8.2) and 21 official images. Not run: it needs Docker, Postgres and a full build, and the hosted app needs a sign-in | 15 screenshots reviewed, code in `references/multica` |
| Conductor (closed; joining Meta, Cloud shuts 2026-11-20) | Live docs and changelog | conductor.build/docs, `/changelog/rss.xml` |
| Claude Projects and agent view, Codex app, Devin, Cursor 3, Linear agents, Factory, GraphCode, Maestri, OpenHands | Official docs, changelogs, screenshots | 11 screenshots reviewed; URLs below |
| Remit | The verify pages (made-up data, no server), driven in Playwright at 1920×1080: clicks, typing, Tab, measured overlap and contrast | 25 screenshots reviewed |

Confidence: **HIGH** where seen in use, in a screenshot or in code; **MEDIUM** where read in docs only; marketing pages
alone are MEDIUM at best. The author of this file re-checked one claim per source: the Claude Projects quotes (doc lines
166, 185, 202, 244), Paperclip `DESIGN.md:13,171-180`, Vibe Kanban `ToolStatusDot.tsx:16-36` and screenshot vk-06,
Multica `comment-trigger-chips.tsx:33-50` and image multica-07, Conductor 0.93.0, and Remit's open-MR count in the store and orch code.

## The finding that matters most

**Claude Projects is Remit's nearest rival**, nearer than Paperclip (`https://code.claude.com/docs/en/claude-projects`,
HIGH). One coordinator conversation takes a stream of work and starts a thread per task; "New work goes to a new thread
or to a thread already working in that area, and Claude tells you which. Each new thread shows up under your message as a
card" (doc line 166). An Overview groups threads, and **Waiting on you** holds "Threads that need your reply or approval,
or that failed" (line 202). Next-step buttons, "**Resolve conflicts**, **Fix CI**, **Address comments**, and **Merge it**
send that instruction to the thread as a message from you" (line 185). Preferences told in words are "instructions Claude
keeps to, not enforced settings" (line 244). That is Remit's design (a Chair, cards per agent, talk instead of forms) shipped by
the maker of the harness Remit runs on. Remit's answer has to be what Projects does not do: work that finds itself through
signals, judges that rule for the customer, ship gated on telemetry, and seats that learn.

## Where Remit already leads

Seen on Remit's own screens and in no peer studied (HIGH):

- **Who decided** every permission: "Chair allowed", "Policy refused", "The session allowed itself … nobody decided in
  30 minutes" (remit-10).
- **The Assistant's decisions, overturnable**, with the overturn kept in view (remit-08).
- **Stale approvals marked**: "Approved an earlier version" on a gate verdict; goal criteria shown against the round before (remit-14, remit-16).
- **Drafts on the server**: a failed send keeps the draft and says why (remit-21).
- **Read receipts** per message, and a queued message to the Assistant marked "waiting for the current work to finish" (remit-07, remit-10).
- **A change item** shows the field as it is and as it would be, and says what Apply and Not now each do (remit-23).
- A composer whose buttons come from the server's list of what the owner may do now: Interrupt, Send, Send now,
  Stop agent…, Restart (remit-05; `CardComposer.vue:23-74`). Peers hard-code theirs.

## Journey by journey

### J1 Know what needs me

| Product | What it does | Evidence |
|---|---|---|
| Claude agent view | Top line "1 awaiting input · 1 working · 2 completed"; each row shows **the exact question** it waits on | claudecode-01, code.claude.com/docs/en/agent-view, HIGH |
| GraphCode | "4 loops need you · oldest 7d 6h · Review ⌘R"; NEEDS YOU as text on the node | graphcode-01, HIGH |
| Multica | Inbox rows "show what needs action ahead of newer updates" (v0.8.1); "Run failed" notices archive themselves when the issue returns to a person, except `blocked`; a "3 agents working" chip with icon and text. But an inbox row names the last change ("Set status to · In Review"), not what is asked | multica-00, multica-06, multica.ai/docs/inbox, HIGH |
| Paperclip | The Decisions card is its best screen: the question, inline answers, Snooze, "Ask agent for recommendation" (paperclip-34). But Decisions is in neither the sidebar nor ⌘K; the badged Inbox row shows the task title only (paperclip-33) | HIGH, observed |
| Vibe Kanban | Needs Attention / Running / Idle sections; one hand icon for both a question and an approval; "unseen" set on navigation, so a turn watched live is flagged as unseen | vk-09, vk-13, vk-18, HIGH, observed |
| Devin, Cursor, Codex | Labels that name the wait ("Awaiting instructions"); one notification batching every stopped agent; failure alerts on a backoff | docs.devin.ai/release-notes, cursor release notes 3.25–3.26, HIGH |
| **Remit** | "Waiting for you" column whose count matches the top bar (3 = 3), "waiting 40m" on each card, spoken labels on the top-bar links (remit-01) | HIGH |

Remit's gaps, observed:
- **Three words for one state.** The same chain reads "Blocked" on its card, "Needs you" on its agent row and "Running" in
  its header; an MR chain reads "Waiting on you" (remit-01, remit-02). The words come from the server:
  `crates/orch/src/standing.rs:183,194,338,407`, `crates/orch/src/agentstanding.rs:350-366`. HIGH.
- **The MR count and the board disagree about archived chains.** `outcomes::waiting` sums open MRs over every chain
  (`crates/outcomes/src/outcomes.rs:158-161`), fed by `Reader::chains`, which has no archive filter
  (`crates/store/src/store.rs:497-498`); the board skips archived chains (`standing.rs:91`). An archived chain with an
  open MR (remit-01, the Archived column) counts in "MRs to review" and in no column of Work. HIGH (read in code, not run on a server).
- The card does not name what waits (a question, an approval, an MR, a failure); there is no oldest-wait figure, no key to jump
  to the next item, and no browser notification (`rg Notification web/src`: none). HIGH.
- In the chain drawer, five waiting items stack in a box capped at 45% height with no count; items 4 and 5 sit below the fold (measured). HIGH.

**Best in class:** the question itself in the row (Claude agent view), with the oldest wait and one review key (GraphCode),
and failure counted as waiting (Claude Projects).

### J2 Give intent / start work

| Product | What it does | Evidence |
|---|---|---|
| Claude Projects | Paste anything; it routes to a new or existing thread and says which; Suggested threads with "start all" | doc line 166, HIGH |
| Factory Missions | "This is a conversation, not a one-shot prompt", ending in features grouped into milestones | docs.factory.com/cli/features/missions, HIGH |
| Linear | Assigning an issue to an agent makes it the **delegate**, "so humans maintain ownership while agents act on their behalf" | linear.app/developers/agent-interaction, linear-01, HIGH |
| Paperclip | `c` opens one "Describe a task…" box; ⌘Enter starts a run in about 3 s, but the box closes onto the dashboard, not the new task | paperclip-23, HIGH, observed |
| Multica | "Create with agent": one box, an agent or squad picked, the dialog closes and the result (or failure, with "Edit as advanced form") lands in the inbox; an agent mentioned in Slack files the issue | multica-10, multica-15, HIGH |
| Vibe Kanban | About 7 steps the first time (repo, branch, prompt); "Failed to fetch" on a dropped server, and the typed text is lost on reload | vk-03, HIGH, observed |
| **Remit** | The Assistant panel, plus a one-box New chain dialog ("An agent picks the playbook and the title"), reached only by `?new=1` or from Playbooks (`web/src/nav.ts:51-58`) | remit-07, remit-09, HIGH |

Gap: the Assistant panel (420×640) covers about a third of the board at 1280 wide (measured). **Best in class:** Claude
Projects. One conversation does the routing and says where each piece of work went.

### J3 Watch live work

| Product | What it does | Evidence |
|---|---|---|
| Factory | Mission Control: RUNNING 16/22, a features checklist, a timestamped log, the active worker's tool calls | factory-01, HIGH |
| Devin | The current step shows its summary instead of "Deciding action"; durations on commands; recordings open on the video | release notes Oct 19–28, HIGH |
| Multica | "Engineer is working" in the issue header opens that run; the run dialog shows "Took 28m 2s · 149 events", an event-density bar, Focus and Filter, typed rows (Bash, Edit, Read) — and also raw base64 JSON rows; an agent profile with Now, Recent work, Last 30 days | multica-05, multica-06, HIGH |
| Claude agent view | Attaching to a session "posts a short recap of what happened while you were away" | agent-view doc, HIGH |
| Paperclip | Thread folds finished steps into "Worked 22s · 4 tools" (paperclip-31); Live runs cards carry **no Stop or Pause** (paperclip-40; `ui/src/components/ActiveAgentsPanel.tsx`); the run page lists 48 timing events beside a red error (paperclip-28); org chart marks "paused" by a yellow dot only (paperclip-42) | HIGH, observed |
| Vibe Kanban | Raw rows "System: hook_started", "requesting", "System initialized…" fill the transcript (vk-06); each tool row has a tiny status dot | HIGH, observed |
| **Remit** | Agent tree with model, live doing-line, turn count, a replaced agent shown ("took over"), mentors on cards (remit-11); a Live tab interleaving every agent (remit-25); a context meter in the drawer (remit-05) | HIGH |

Gap, observed: a folded tool group reads "Used 1 tool · ran 1 command 0s" with no failure mark; only the opened row shows it
(remit-13, `web/src/components/StreamBlocks.vue:113-117`). **Best in class:** Factory's split into a progress counter,
a checklist, a log and the active worker, plus a recap on return (Claude).

### J4 Talk to and steer an agent

| Product | What it does | Evidence |
|---|---|---|
| Codex | "**Steer** adds the message to the current run. **Queue** saves the message for the next run." Queued messages sit above the composer: edit, reorder, send, delete | learn.chatgpt.com/docs/prompting, HIGH |
| Vibe Kanban | An empty box offers Approve; typing turns it into **Request changes** (vk-05, vk-06); a question replaces the composer with options and "Type a different answer…" (vk-11); one queued message, cancellable (vk-12); Stop leaves no mark in the transcript | HIGH, observed |
| Multica | "2 agents will start when sent" under the box opens a list of each agent and why (@mention) (multica-07). For a running agent: "Add to current run / Start after this run / Stop and start over / Won't start this time"; "Add to current run" only when the coding tool supports steering; after sending, "Waiting for … to read it" then "Read by …"; a comment starting `/note` wakes nobody | multica-07, multica.ai/docs/mentioning-agents, `comment-trigger-chips.tsx:33-50`, HIGH |
| Paperclip | Question card above a composer that stays usable (paperclip-32), while the status wrongly says "In Review"; Pause cancels the run with no toast, then a notice "Builder is paused… It was paused manually." with Resume (paperclip-41), while the header still says "In Progress" | HIGH, observed |
| Linear | A `stop` signal: the agent does nothing more and sends one final response or error stating its state | linear.app/developers/agent-signals, HIGH |
| **Remit** | Server-listed actions (remit-05); "The Chair asks you" above the composer with Other for free text (remit-03) | HIGH |

Gaps, observed: a permission offers Allow and Reject only, so a refusal carries no reason (remit-04; `InboxCallout.vue`
answers with `text: ''`); Pause acts at once with no confirmation, and a paused chain's composer does not say what a message
will do while paused (remit-06, remit-16); the permission command wraps mid-token (remit-04). **Best in class:** Codex's
named Steer and Queue with a visible queue; Vibe Kanban's typed-text-turns-Approve-into-Request-changes.

### J5 Review a result and decide

| Product | What it does | Evidence |
|---|---|---|
| Claude Projects | Next-step buttons that are messages from the owner: Resolve conflicts, Fix CI, Address comments, Merge it | doc line 185, HIGH |
| Devin Review | Findings on Devin's own PR go back to the session first; "only the findings it could not fix are posted" | release notes Sep 30, HIGH |
| Multica | A Reviewer agent posts findings by severity with a "Verified:" paragraph; the owner answers "@Engineer fix all"; the squad leader hands back "ready for your acceptance… Two things worth your judgement… Nothing merges until you say so". The merge itself happens outside, as in Remit | multica-08, multica-09, tutorial-squad-delivery.webp, HIGH |
| Conductor | Checks tab: git, PR, CI, deployments, review threads, todos; merge blocked while any is open | conductor.build/docs/reference/checks, MEDIUM |
| Vibe Kanban | Comments on diff lines batch into the next message with file, line and quoted code (vk-08, vk-10). Merge confirms without naming branches, then archives silently; the squash commit message was the raw prompt plus a UUID (vk-14, vk-15) | HIGH, observed |
| **Remit** | Gate strip (Review, QC, Goal check) with verdict counts; stale approvals marked; file tree with thread counts; batched drafts "Send 1 to the Chair" (remit-12, remit-14, remit-15) | HIGH |

Gap, observed: the board's ask is "merge MR !12, !13", and the only control is the external "Open !12" link, so the decision
leaves Remit (remit-15). MRs, CI and threads sit on a different tab from the gates; no next-comment or mark-viewed key.
**Best in class:** Claude Projects' next-step buttons. A button sends a message as the owner and the agent acts, so the
browser still never edits the domain.

### J6 Configure and teach

| Product | What it does | Evidence |
|---|---|---|
| Devin | A knowledge suggestion appears as an event in the worklog; Session Insights sorts Useful and Misleading knowledge and offers an improved prompt that starts a new session | docs.devin.ai/product-guides/session-insights, devin-02, HIGH |
| GraphCode | "The daemon records facts, the agent records lessons" | graphcode.app, HIGH |
| Multica | A helper agent creates agents, squads ("47 steps") and autopilots by chat (multica-12, multica-14). But a form is the default path: the chat builder is reached from the form through "Let AI write it", and only on an online, non-cloud runtime; workflows are dropdowns ("On entering, hand to", "When done, move to") | multica-11, multica-15, multica.ai/docs/agents-create, HIGH (builder gate MEDIUM) |
| Paperclip, Vibe Kanban, OpenHands, Linear | Forms: Paperclip's agent has 15 sub-pages and a routine form (paperclip-38); Vibe Kanban's settings are a schema form whose modal is not `role=dialog` (vk-17) | HIGH, observed |
| **Remit** | Change items with before and after; credentials in a write-only field "no agent is told it" (remit-23, remit-24) | HIGH |

Gap, observed: key results render as raw JSON (`check: {"kind":"submit",…}`, remit-23). **Best in class:** Devin's lesson
proposed at the moment it is learned, in the stream, and a later review of which knowledge helped and which misled.

### J7 Failure and recovery

| Product | What it does | Evidence |
|---|---|---|
| Devin | Issues detected with an impact rating, a timeline showing the retry loop, then "Start new session" with a better prompt | devin-01, HIGH |
| Multica | A table of failure reasons, each with its fix (`runtime_offline`, `provider_auth_or_access`); passing faults retry by themselves, agent errors wait for a manual retry; a failed task moves the issue back to `todo` and "leaves a failure note on the Issue" | multica.ai/docs/tasks, changelog v0.7.0, MEDIUM |
| GraphCode | STALLED state; "Nothing runs it and it hands to nothing" with **Wire it up**; edges that fire on failure | graphcode-01, HIGH |
| Paperclip | Resume refused with 409 and 500 and nothing on screen (paperclip-17); "ACP agent reported a terminal access failure" standing for "the CLI is too old for this model"; an old failure kept as the agent's own message after a later success (paperclip-32, paperclip-37) | HIGH, observed |
| **Remit** | Stale banner explains itself (remit-18); "Remit could not finish" with Retry (remit-03) | HIGH |

Gaps, observed: the failure line is raw JSON, `Chair failed: {"code":-32603,"message":"Internal error: You've hit your session
limit…` (remit-17); "Newer version, reload the page" shows a spinner and no Reload button (remit-19); a dropped link blanks the
whole app (remit-20). The last is deliberate: `web/src/components/ConnectionBanner.vue:7-8` says Shell draws no page while
the link is down. **Best in class:** Devin's retrospective. It detects the problem, explains it and starts again from a
better brief.

### J8 Navigate

All four run peers have ⌘K. Vibe Kanban's two-key sequences work (`g s` opens Settings, vk-16), but its ⌘K finds commands,
not work; Multica's ⌘K groups pages, commands, people and issues and always ends with **Hand to agent** (multica.ai/docs/shortcuts); Paperclip's `?` opens a shortcut sheet with `j/k`, `g i`, `x` (paperclip-11); Claude agent view peeks with Space
and attaches with →. **Remit:** every Tab stop shows a 2px outline and nothing traps focus (remit-22), but ⌘K, Ctrl+K, `/`
and `g` do nothing, there are no arrow keys between cards, and the last of 17 cards is about the 28th Tab stop. HIGH.

### J9 How agents relate to each other

GraphCode types its edges: **hand-off** ("fires when the source resolves"), **message** ("staged if it's asleep"),
**spawn** ("children die with their parent"), each with a condition (always, on success, on failure), and messages are
addressed by name (graphcode.app, HIGH). Multica ships the same idea per workflow step, "When done, move to" and "If rejected, send back to"; assigning an issue to a squad wakes only its leader, which hands on with an @-mention ("@Engineer — taking this one… Reviewer goes next"), so every hand-off is a comment in one timeline (multica-13, multica-15, multica.ai/docs/workflows, HIGH). Devin nests child sessions and closes their PRs when the parent is archived; Claude
labels a message from another session with that session's title. Remit already draws every interaction as an edge; typing
and conditioning them is the step GraphCode shows.

## Principles

Stated by the peers, quoted from their sources (HIGH), and kept because they match Remit's own rules:

1. "Every screen should answer, in order: *what is happening, does it need me, what do I do about it.*" (Paperclip `DESIGN.md:13`)
2. "Status is systematic… An operator learns the vocabulary once." (Paperclip `DESIGN.md:32`) Remit breaks this today (J1).
3. "Validation lives in the daemon — a rule only one client enforces isn't a rule." (graphcode.app) Remit's *Server-driven*, said by a peer.
4. "Reported, never estimated." A figure nobody measured shows as "not reported", never as zero (graphcode.app); "unknown is
   not none" (Multica `workspace-agent-working-chip.tsx:25-43`).
5. "The verifier stays outside the verified." / "A loop may not change its own stop condition." (graphcode.app)
6. "humans maintain ownership while agents act on their behalf" (Linear agent-interaction).
7. "Missions are not fire-and-forget. The orchestrator is an agent, and you can talk to it." (Factory Missions)
8. "The team does not review status theatre. It reviews the work itself." (Multica `VISION.md:67-70`)

Learned from watching the peers fail (HIGH, observed): **every outcome leaves a mark** (Vibe Kanban's silent merge, archive
and stop; Paperclip's silent 409); **an error says the cause in the owner's words** (Paperclip's "terminal access failure");
**one status tells the truth** (Paperclip's "In Progress" beside "paused"); **"seen" means seen, not navigated to** (Vibe
Kanban); **the place that holds what waits is where the badge points** (Paperclip's hidden Decisions page).

## What not to copy

Each conflicts with `AGENTS.md` (HIGH): forms and settings screens (all run peers; OpenHands, Linear); a message box docked at
the page foot (Vibe Kanban vk-04, Paperclip, Multica, Conductor); grouping and derived state in the browser (Multica,
Vibe Kanban); sections under headings inside one list (Vibe Kanban's sidebar); state by colour alone (Paperclip's org chart,
Devin's impact timeline, Conductor's status grid) and a glow on a card (GraphCode); cost and token counters (Factory,
OpenHands, Multica's token panel on every issue and its Usage page, Paperclip budgets); a form kept beside every chat path (Multica); a human as the splitter of work and the merge gate (Conductor, Vibe Kanban); revert
that deletes history (Conductor); a forced new sign-in when a working login exists (Paperclip).

## Proposals for Remit, ranked

Ranked by how often the owner meets the gap and how many products converge on the answer. Each was tested against its
strongest objection.

1. **One vocabulary for what a chain needs, and the wait named on the card.** One word per state across card, agent row and
   header; the card names the wait ("asks: …", "approve: Bash …", "merge !12", "failed: …"), shows the oldest wait, and a
   failure counts as waiting. *Objection:* chain and agent states differ in scope. *Answer:* the owner sees one thing; scope
   can show as the subject ("the Chair asks"), not as a different word. Owner of the change: `chain-runtime`
   (`standing.rs`, `agentstanding.rs`), then `web-ui`. Evidence: remit-01, remit-02, Claude agent view, Paperclip principle 2;
   Multica shows the cost of not doing it (an inbox row names the last change, not the ask) and the reward (failure notices
   that leave the list by themselves once the work returns to a person).
2. **Next-step buttons that are messages from the owner**, on the chain and its MRs: Fix CI, Address comments, Merge it.
   This brings the merge decision back into Remit (remit-15) without the browser editing the domain: "Merge it" goes to the
   Chair, and a chain holding `ship` merges. *Objection:* it duplicates typing the same words. *Answer:* one click on the one
   decision the board already asks for, and the message is recorded as the owner's. Owner: `review-and-ship`, `rpc-contract`, `web-ui`. Multica's squad hand-back ("Two things worth
   your judgement… Nothing merges until you say so") is the message the button answers; Multica itself leaves the owner typing.
3. **A refusal carries a reason, and the composer says what a message will do.** Typed text turns Allow into "Refuse with
   this reason" (Vibe Kanban, observed); a paused chain's composer says the message waits for Resume; Pause asks first.
   Name Steer and Queue, with the queue visible and editable, only once `crates/claudecode` is shown to take input in the
   middle of a turn; until then Remit's Send and Send now are the honest pair. Multica does exactly this gating: "Add to current run" appears
   only when the coding tool supports steering, and "Waiting for … to read it" becomes "Read by …" (multica-07). Owner: `rpc-contract`, `chain-runtime`, `web-ui`.
4. **Every outcome leaves a mark, in plain words.** Replace raw JSON in failure lines (remit-17) and key results (remit-23)
   with sentences the server writes; a Reload button on "newer version" (remit-19); a merge, archive or stop always lands
   in the stream with who did it. Owner: `chain-runtime` for the messages, `web-ui`.
5. **A transcript that scans.** A failed count and an alert mark on a folded tool group (`web/src/stream.ts`, sent by the
   server), durations per step, and a recap on return ("while you were away: …"). Owner: `chain-runtime`, `web-ui`.
6. **⌘K and go-to keys**, plus a key to jump to the next thing that waits and a browser notification for owner items only.
   Rows are the browser verbs (go, watch, prompt, answer, pause, stop, start) and "ask the Assistant to …"; Enter never edits
   the domain. *Objection:* notifications become noise. *Answer:* only items in "Waiting for you", one per item, batched like
   Cursor's. Owner: `web-ui`, with the server's waiting list as the source. Multica's ⌘K ends every search with "Hand to agent":
   the row Remit gives to the Assistant.
7. **"Finished since you last looked", kept by the server from what the owner viewed**, never from page navigation (Vibe
   Kanban's false unseen). Owner: `store-and-migrations`, `rpc-contract`, `web-ui`.
8. **Typed, conditioned edges.** Hand-off, message and spawn, each on success or on failure; a message to a sleeping agent
   shown as staged; a card that hands to nothing flagged. Multica already ships them per workflow step ("When done, move to", "If rejected, send back to") behind dropdowns;
   in Remit the owner tells the Chair. Needs an ADR first: `architect`.
9. **A retrospective that teaches.** After a chain, the issues found with their impact in words, which memory helped and which
   misled, and an improved brief with "start again"; a lesson proposed as an event in the stream for the coach or the owner
   to accept. This joins Remit's teaching design (`docs/research-agent-learning`). Owner: `objectives-and-crew`.

Also seen and left for later: a cross-chain live view with Stop on each run (Paperclip has the view, without the Stop);
Cursor's video replay as review evidence; Conductor's turn rewind that shows its blast radius first.

## Defects found on the way

- **The MR count includes archived chains** (J1). Decide whether an archived chain's open MR still waits for the owner; if it
  does, the board must show it, and if not, `outcomes::waiting` must skip it. Owner: `chain-runtime`.
- **The board seed contradicts the server**: `web/verify/app.ts:204` puts the `author` chain under Running although it has open
  inbox items, which the server always places in Waiting for you (`standing.rs:97,153-154`); `mrsWaiting: 4` is hard-coded at
  `app.ts:301`. Owner: `web-ui`.
- **Contrast**: `--mut` on `--blue-bubble` is 4.49:1, under 4.5:1 (resolved-thread text on the Changes tab). Every other text
  pair scanned on 7 pages passes. Owner: `web-ui`.
- **Overlap**: the Assistant button's tooltip covers the search box while focused; the Assistant panel covers about a third of
  the board at 1280 wide. Owner: `web-ui`.

## Coverage and limits

- Paperclip: run twice; review and merge never seen, because the run after the answer failed (its Projects accept GitHub
  repositories only). Agents ran only after three manual database changes, since its onboarding forces a new Claude sign-in;
  two `claude setup-token` processes it started were killed before any code was entered. Approvals and budgets not exercised.
- Vibe Kanban: run without a cloud sign-in, so its kanban board and PR creation were not exercised; folded tool groups never
  appeared, so its worst-status dot is untested. This seat's hooks reached its agent through `CLAUDE_CONFIG_DIR`, so its
  permission behaviour is MEDIUM.
- Multica: not run (Docker down, disk 23 GiB free; the hosted app needs a sign-in and none was made). Its UI was read from
  the live landing page (v0.8.2), docs, the changelog from v0.7.0 and 21 official images; the doc tutorial images show the
  sidebar from before v0.8.0, whose screens for issues, runs and chat the v0.8.0 changelog leaves unchanged.
- Closed products: Cursor's site refused connections, so its claims come from the release-notes mirror and search snippets
  (MEDIUM where marked); no primary page found for Devin Desktop; Maestri is marketing only (MEDIUM).
- Remit: verify pages only. Real latency, pause and finish outcomes, browser notifications, how long a callout stays and
  multi-tab sync cannot be seen on made-up data; the Results, Signals and Crew areas were not covered.
- Everything run was removed afterwards: scratch folders, data directories, npx caches, ports 3100, 4777 and 54329, and the
  Claude session folders the runs wrote. The owner's earlier Vibe Kanban and Paperclip data (May and June) was left untouched.
