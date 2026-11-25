# 0118. Remit judges fast in typed questions, records every judgment against what happened, and learns which decider to trust

Status: accepted, 2026-11-19; S1–S7 built: S1 (the layer and its view), S2 (memory), S3 (the hire as a cascade of can-it questions, and the climb), S4 (the depth of a review), S5 (an item for the owner, with targeted second opinions), S6 (the screen of a subscription that starts a chain), S7 (the watchdog, `ask_decider` and the steering). Source: the owner, 2026-11-19 (translated): "the models in use are extremely
expensive, mostly opus, reallocate them", then, of TypeSafe's Jev: "it is a model that makes choices quite well; what use
does it have in the current system to improve quality?", then, of the five places proposed: "agree with all of them, 1 to
5, but push it a bit further". Extends ADR-0117 (the librarian's duplicate rule), ADR-0087 (the Assistant's decisions and
their second opinion), ADR-0110 (the gates) and the Chair's hiring (`orch/src/chair_hire.rs`).

## Context

Remit makes many small judgments, and it makes each one of two ways: a whole LLM turn on the strongest model, or a fixed
number in code. Nothing in between reads the content, and nothing records whether a judgment was right.

- **Which model a hire runs on.** The Chair decides in its own turn (`hiree_of`, `orch/src/chair_hire.rs:277-289`). In the
  owner's database on 2026-11-19, 17 of 20 cards ran on `fable` (14 of them at `high`) and the other 3 on `opus`. No card
  ran on anything cheaper, and nothing ties the choice to the card's outcome.
- **Whether a reading deserves a chain.** Matching is structural: a key pattern, plus an optional CEL condition
  (`signal/src/evaluate.rs:258-283`). Nothing judges what the reading means.
- **Whether two memories say the same.** The rule is cosine ≥ 0.84 (`crew/src/consolidate.rs:25`).
- **How deep a review goes.** Depth follows the chain kind and one path match (`shipping/src/gate.rs:113-127`). What the
  diff contains changes nothing.
- **How an item for the owner is decided.** The Assistant decides it on `fable` at `high` (`chains/src/assistant.rs`), and
  a second opinion re-judges a random 8% (`orch/src/second_opinion.rs:36`).

A new class of model fits exactly between these two ways. A *decision model* (TypeSafe calls it System One) reads a state
and typed questions and returns a calibrated answer with no text: a choice with a probability for each option, a score on a
rubric, or a yes/no probability. OpenRouter serves about 44 of them behind one endpoint, `POST /api/alpha/decisions`
(guide: `openrouter.ai/docs/guides/features/decisions.md`; listed with `/api/v1/models?output_modalities=decisions`). They
cost about $0.00002–0.04 per million input tokens, output is free, and an answer takes about 0.1 s. Remit already holds an
OpenRouter key (ADR-0117).

Measured for this decision (2026-11-19). The data: the owner's 70 active memories, every pair at cosine ≥ 0.70 (62 pairs),
and the 30 closest of them, each labelled by hand after reading both memories in full. 8 pairs are the same lesson, 20 are
not, and 2 are ambiguous and not scored. The question asked: "Do memory A and memory B teach the same lesson?", once bare
and once with criteria saying what counts as yes ("the same advice, in other words or with different detail: one could be
merged into the other without losing advice") and what counts as no ("related topics, different advice").

| Decider | AUC, bare | AUC, with criteria | Same lessons found at p ≥ 0.5, with criteria | False alarms | Cost of 30 pairs |
| --- | --- | --- | --- | --- | --- |
| `liquid/d1.2-ultra` | 1.000 | **1.000** | 8/8 | 1 | $0.0003 (one 502, retried) |
| `typesafe/jev-1.14` | 0.912 | **0.994** | 7/8 | 1 | $0.0007 |
| `perplexity/pplx-decider-v1.3:free` | 0.963 | 0.979 | 7/8 | 2 | $0 (one 429) |
| `microsoft/microsoft-decision-1.1` | 0.950 | 0.950 | 6/8 | 1 | $0.0003 |
| cosine ≥ 0.84 (today's rule) | — | — | 2/8 | 0 | — |

Every decider beats today's rule. Two things decide the result more than the vendor. **The question's words:** without
criteria Jev gave the plainest duplicate (two memories that both say `record_verdict` needs a clean worktree) p = 0.28, and
with them 0.63, its AUC rising from 0.912 to 0.994. **The question itself:** the deciders rank differently, and one
labeller with 28 pairs cannot separate the top two (MEDIUM evidence). Which decider is best can only be known by measuring
it on Remit's own outcomes, question by question, as ADR-0117 measured six embedding models before choosing one.

## Options

1. **Hard-wire Jev at the five points.** It is cheap and quick, and with criteria it ties for the best on the one question
   we measured. But it ties every point to one vendor, and nothing would tell us when a judgment goes wrong or when another
   decider answers a question better. Rejected.
2. **Use a small LLM (`haiku`) with structured output at each point.** It works with no new provider. But it returns no
   calibrated probability, so there is no threshold that means anything, and it is 10–100× dearer per judgment. Rejected:
   thresholds are the point.
3. **A decision layer owned by Remit.** Questions are typed and declared in code. Any decider on OpenRouter can answer
   them. Every judgment is recorded with its outcome, a decider is scored per question, and the best one acts while others
   answer in shadow. Thresholds come from measured precision. The existing path (the LLM turn, or the rule) remains as
   System Two for what the decider is unsure of. **Chosen.**

## Decision

### The layer

- **`crates/judgment`** is the pure core. It does no I/O.
  - **One registry of questions** (`QUESTIONS`, `judgment/src/question.rs`). Each question is declared there once: a
    stable id with a version (`memory.same_claim@1`), its title in the owner's words ("Finding duplicate memories"), its
    instructions, its form, whether it acts alone or only informs, who decides when it does not (its System Two: "the
    librarian"), its target precision, its default decider, what a wrong answer said in words, and `MayLeave`. A
    declaration nobody could ask under (an empty title, a target precision outside (0, 1)) fails to compile. All eleven
    questions of the points below are declared from S1; each point's slice refines its own words in a new version.
  - **`Question`** is a declaration asked with a `Kind`: `noul(yes, no)`, `choice(options)` (at least two, each named once)
    or `score(levels)` (at least two), each side, option and level described in words. Its constructors check all of it,
    and a kind not of the declaration's form is refused, so a question that cannot be asked cannot be written.
  - **`pick`** reads a decider's answer once, against its question: what it picked, typed per kind
    (`model::JudgmentPick`: `Yes`, `No`, `Option`, `Level`), or none when the answer does not answer the question.
  - **`Standing`**, **`ranked`** and **`act_threshold`** (below).
- **Port and adapter.** The `Decider` port is declared in `crates/judgment` and takes a state plus questions, returning
  an answer or none for each question, or an error. Its one adapter is `openrouter::OpenRouter::decide` on
  `/api/alpha/decisions`, with the key, retries and timeout `crates/openrouter` already owns. There is no TypeSafe SDK and
  no second key.
- **A question with no answer is not answered; the others are.** The guide lets a provider refuse one question
  (`{"type": "refusal"}`) while it answers the rest, and says to read a refusal, a key left out of `answers` or a type it
  added later as "not answered", never as a verdict. The adapter reads each as none for that question
  (`DecisionAnswer::Refusal`, `DecisionAnswer::Other`), and `answers_of` (`chains/src/judge.rs`) is the one place that
  decides: an answer that is missing, refused or does not fit its question (`pick` gives none) leaves that question to the
  next decider and, at the end, to its System Two. The other questions of the call keep their answers, and their shadows
  are still asked.
- **`model::Judgment`, a table of its own (`judgments`).** It grows without bound. It holds: the question id and version,
  the subject (`card`, `memory pair`, `reading`, `inbox item`, `change`, with the chain it was asked in), a digest of the
  state as sent (never the state itself), the decider, the answer and its pick, `cost_usd` and latency, the `Use` the code
  made of it (`Acted`, `Escalated`, `Informed`, `Shadow`), and the outcome once known (`Right`, `Wrong` or `Unknown`, its
  source as `OutcomeSource`: `librarian`, `gate`, `owner`, `second_opinion`, `card`, `chain`; when).
- **Shadows.** Every judgment is also asked of up to two other deciders, recorded as `Shadow`. In a shadow the code acts
  on nothing; it is only measured.
- **Standing.** For each (question, decider) the standing is the Brier score and the AUC over judgments with a known
  outcome. The decider that acts for a question is the one with the best Brier score once it has 30 outcomes. Until then
  the default for every question is `typesafe/jev-1.14`, the owner's choice where the measurement above ties. The shadows
  start as `liquid/d1.2-ultra` and `perplexity/pplx-decider-v1.3:free`. A switch is said to the owner as "its answers
  proved more reliable": it is made on the Brier score, which rewards being right with the right confidence, not on how
  often a pick was right.
- **The order deciders are asked in** (`judgment::ranked`). Every decider with 30 outcomes, the best Brier score first,
  then the question's default. The first acts. When it fails, is late or gives the question no answer, the next is asked,
  for the questions still unanswered only, each held to its own act threshold. A decider that stops answering (a model
  retired) would otherwise hold its question in System Two for ever, since it makes no new outcome to lose its standing
  by. The shadows are then asked beside the decider that answered, but not those that just failed.
- **Thresholds.** Each question has a target precision set by what a wrong act costs. The act threshold is the lowest
  probability whose measured precision on outcomes reaches that target. With fewer than 30 outcomes it is 0.9.
- **The asymmetry rule.** A judgment *acts alone* only in two cases: it tightens (more review, more effort, a question
  escalated, a gate added), or it saves work whose result is checked later and can be reversed (a cheaper model whose card
  still passes its gates). Anything else only *informs* the agent that decides. **A judgment never loosens a rail**:
  authentication, pause and stop, a gate, the jail, credentials.
- **What may leave the machine** (`MayLeave` on each declaration). Today every question's state leaves as `Redacted`:
  `judge` drops the value of every credential field and redacts tokens and masks email addresses in every text, with the
  same functions a reading's body passes (`source::redact_value`, `source::clean_texts`), before the state is digested and
  sent. A question whose state may not leave is a new variant `judge` stops at (*Revisit when*).
- **No decider, no change.** Without the key, or when every decider in the order fails or is late (2 s each), the point
  runs its System Two path as today and records nothing for that question. A judgment never blocks a turn for long.

### The five points

1. **The model of a hire.** When neither the Chair nor the agent definition names a model, a cascade of yes/no
   questions chooses it: for each tier below the strongest, cheapest first, `hire.tier_can@1` ("Can a {tier} model do
   this work well?", its criteria from `provider::Tier::fits`), all in one `judge` call, about the agent, the brief, the
   deliverable, the criteria and the chain kind. **The cheapest tier whose yes acts runs the hire**; if none acts, the
   hire runs as today (System Two). A tier above it is never asked to act, so none is rewarded for being strong. Only
   the Chair's own `hire` asks: a hand-over, a workflow's hire and a duel's two sides run on the model and effort the
   Chair, the agent or Remit's own climb name, so a duel's sides share one model and no judgment is stored for them.
   **The effort is not chosen.** `hire.effort@1` (a Choice) is asked in the same call and only informs (`Acts::Informs`):
   it is recorded, shadowed and shown, never acted on, because a climb raises only the tier, so no outcome can score an
   effort wrong, and an answer that could only be right would pass for a perfect one. A model the Chair names wins, then
   the agent definition's, then the layer's, then the Chair's own (`HireInput::named_model`, the one place that
   precedence is written); the first two never climb. A gate signer stays on the judge model (a gate is a rail).
   **The recovery is built before the risk is taken:** a card whose model the layer chose is rehired one tier higher by
   code, recorded as a decision by Remit, when its turn fails or its gate sends its change back twice. Outcome of the
   tier question that acted: `Right` when the card is accepted, `Wrong` when its gate sent it back twice and it
   climbed, `Unknown` when a broken turn raised it (a broken turn says little of the model); the tiers it did not act on
   are not scored.

   *Exploration.* Only an answer at or above the act threshold (0.9 until a decider has outcomes) ever gets an outcome,
   so a decider would never learn whether its middling answers are right. When no tier acts, **`EXPLORE_PERCENT` = 10**
   % of hires (deterministic by card id, as `second_opinion` samples) act on the cheapest tier whose yes is at least
   **`EXPLORE_FLOOR` = 0.6**, recorded as `Acted` and shown as "trying sonnet; it moves up if it struggles". A tenth
   bounds the cost of finding out, and 0.6 is more likely yes than not by a margin, so the climb is rarely needed.

   *As built (S3).* `Orchestrator::hire` (`orch/src/chair_hire.rs`) calls `Core::pick_for_hire`
   (`chains/src/hire_choice.rs`) last, once nothing can refuse the hire, and only for a hire that signs nothing;
   `hiring_of`, which every other path calls, asks nothing. The card keeps `Card::chosen: Chosen { judgment }` (none for
   the climb's own choice), so the climb acts only on a model the layer chose. The hire's decision reads "…, model
   chosen by Jev 1.14, 97% sure; effort left as set; Jev 1.14 suggests medium, 82% sure". A hire that is refused marks
   its judgment `Unknown` (`hire_unused`).
   - **The climb** (`chains/src/climb.rs`) is a hand-over Remit makes (`hand_over_by`, `by_remit("climb")`): the work
     goes to the same employee on a new card one tier up, at its effort, in the same worktree; the new card is chosen
     by code too, so it climbs again until the top. The Chair is told in the hand-over's note that Remit is rehiring,
     and the new card's Hire decision is Remit's, with the climb's why. It never goes above the catalog's strongest.
     Two triggers. (a) A worker's turn fails with a cause a stronger model could have avoided (`Cause::Harness`,
     `Cause::Silent`; not a limit, a session that would not open, a process that ended or a restart), in
     `Core::respond_to_failed_turn`, which `record_turn_end` calls: the hand-over takes the core's note at once, and when
     Remit takes the card the Assistant is not handed a Retry item (one owner of a failure); when Remit cannot (a hiring
     rule refuses), the Assistant is, as it always was. (b) A gate asks for changes to a card's change the
     `CLIMB_AFTER_ROUNDS` = 2nd time or later (`shipping::gate`, beside `MAX_GATE_ROUNDS`, which asks the owner later):
     rounds are counted for each author card since it began, as distinct (reviewer card, change) requests by that role
     from the reviewers its handoff edges reach, so a verdict a retry records twice is one round and a raised card starts
     with none (it is given the old card's reviewer edges). The card raised is the one card whose model the layer chose
     among the employees in `version.by` of the landing of that change; with two such cards nothing is raised. A
     climb Remit cannot finish clears `chosen`, so it does not try again, and tells the Chair.
   - **A hand-over is never a descent.** A Chair's hand-over of a card the layer chose runs on that card's model
     (`hire_of_handover`) and keeps it chosen; only a climb changes the tier, and only upward.
   - **Outcomes.** The hand-over's own commit records the tier judgment (`Records.proofs`), in the same transaction:
     `Wrong` for a gate's climb, `Unknown` for a failed turn's climb, a Chair's hand-over and a refused climb. An
     `Accept` records `Right` on the card's judgment inside the commit that stores the acceptance (`commit.rs`), so the
     Chair's `accept_work` and a workflow's accept both score it.
2. **Whether a reading deserves a chain.** A subscription may carry a `screen`: a yes/no question in plain words that the
   Assistant writes, as it writes `expr` today. A reading screened out records `Skip(Screened { p, judgment })` in the
   Signals view. To learn from it, 5% of screened-out readings fire anyway as an audit. Outcome: the chain a reading
   started does work (`Right` for a pass), or finishes with nothing to do (`Wrong`).

   *As built (S6).* A `screen` is one statement in the owner's words, as `expr` is: when a reading deserves a chain, that a
   decider judges true or false of one reading. Only a `start` on what is heard carries one (`Subscription::check`
   refuses it on any other action or on a schedule, and past 600 characters); the Assistant writes it with `create_subscription`
   and `update_subscription` as `screen`, the Subscriptions tab shows it on the line ("Worth a chain when"), and a dry run asks
   no decider and says so. One registry entry serves every screen: `signal.screen@1`, asked with the subscription's screen as
   what makes the answer yes (`screen::question_of`), so the standing is measured over all screens together.
   - **Where it sits.** It screens *firings*, not readings, and not a batch: `Subscription::check` refuses a screen on a `batch`
     flow, whose one firing covers many readings, so a decider's answer about one is never applied to the rest. `signal::asks`
     names the reading each firing the flow makes is on, before overlap; the shell puts them to `Core::screen_firings`
     (`chains/src/screen.rs`) in one round; and `evaluate` applies the answers (`Input::screens`) between flow and overlap, pure: a
     firing whose reading a decider was sure needs no chain becomes `Skip(Screened)` (a near-miss whose detail says "94% sure it
     needs no chain", counted in `Tally::screened`) and frees its place in a `buffer_one` overlap, so the firing behind it starts
     rather than being skipped for room a dropped one took. Nothing takes the dropped firing's place, and a screen never
     changes how flow shapes the rest. The shell (`Pipeline::judge`) records a subscription with no screen as it reads it and
     holds the screened ones until one asking has been made for all of them: every ask of a pass runs together within
     `SCREEN_WITHIN` = 3 s, one deadline for the whole pass (a decider call is bounded by `DECIDE_FOR` = 2 s itself; the second
     is for the store), so the Tick, which holds `tick_mu`, waits at most that however many subscriptions screen. At most
     `SCREENED_PER_PASS` = 20 firings of one subscription are asked in a pass (the events of a kept signal take no place among
     them); the rest are deferred (`Verdict::Defer`): the cursor stops before the first of them and the next pass makes them
     again, so a burst is screened a pass at a time and never fires unscreened. A judgment whose answer came after the deadline
     is stored all the same, and the next pass that asks about the same reading under the same words reads it
     (`Reader::reading_judgment`) instead of asking again, so none is left without a use. The words of the screen are part of
     what a judgment is about (its subject carries their digest): a screen the Assistant corrected asks again about readings the
     old words judged, and a cursor rewound under unchanged words reads the first answers.
   - **What acts.** Only a *no* at or above the act threshold (`Acted`) drops a firing. The question declares that it acts on a no
     only (`ActsOn::No`): its threshold is measured over its nos alone, in the one place the threshold is worked out (`judge::threshold_of`, over `threshold_results`: all picks for a question that acts on either side, those of its side for `ActsOn::No` and `ActsOn::Yes`), because a yes that proved right says
     nothing of how often a no does, and a yes never acts. The judgment is stored as escalated, and the commit that records the
     drop sets it `Acted` in the same transaction (`Tx::set_judgment_used`), so none reads as an act that did nothing. The provisional bar holds until 30 results *of that side* are known, so the page and the steering say so in those words ("30 results of its no answers", `ActsOn::results_words`), and the owner's precision, clamped to the question's floor (90 for a screen: an event screened out wrongly is gone), applies to that side's bar. A yes, an
     unsure no, a refusal, a late answer, no decider, and the events of an event signal kept on this machine
     (`EventSignal::kept`, set with `update_event_signal`, which leaves it as it is when it is not given) all fire as the
     subscription always did. `AUDIT_PERCENT` = 5 of the firings a decider would drop (by a hash of the subscription and the
     reading's place) fire anyway, carrying the judgment: a no that acts is otherwise never checked, so its precision, and with it
     the threshold, would never be measured. `hire.tier_can`, which acts on a yes only, is declared `ActsOn::Yes` and measured over
     its yeses the same way; `memory.same_claim` acts on either side (a sure no overrides the cosine rule), so it stays measured over
     every pick.
   - **What proves it.** Every firing keeps the judgment of its reading (`Firing::screen`), the one place the link is kept. When the
     chain it started first finishes (the change to done, not a later save), the commit that stores the finish finds the firing by
     the chain (`Reader::screen_of_start`) and scores the judgment (`screen::finished_start`, `OutcomeSource::Chain`): a chain that
     hired a worker (`CardRole::Worker`; its Chair and its judges do not count) did work, so yes was right and no was wrong; one
     that hired none had nothing to do, so no was right and yes was wrong; one the owner ended, that stalled, or whose goal check
     could not run proves nothing (`Unknown`). Reopened and finished again, it replaces the outcome. A score that cannot be written
     is logged and never fails the finish.
   - **What leaves the machine.** For each firing asked about, to OpenRouter and the decider's provider: the signal's key, the
     reading's subject, its value (for an event, its headers and its body) and the subscription's screen. An event's body is
     read as JSON when it is, so that `judge` drops every credential-named field from it and redacts tokens and masks email
     addresses in every text (`source::redact_value`, `source::clean_texts`), as a source's reading is before an agent reads
     it. Names, ticket text and other personal data in a body leave as written. The seam is `EventSignal::kept`, which the
     owner sets by telling the Assistant: its events are never sent, and a subscription's screen of it asks nothing. The
     Assistant is told in its manual what leaves.
3. **Whether two memories say the same, or contradict.** Candidates move from cosine ≥ 0.84 to ≥ 0.65, then
   `memory.same_claim@1` and `memory.contradicts@1` judge them. Above the act threshold, the librarian must name the pair
   (supersedes, links or `distinct_from`), as `remember` already requires. It is *acted on* only when the decider's
   answer reached the act threshold (`Acted`); an answer too unsure to act, a late one and a refusal leave the cosine 0.84
   rule, the System Two, which still makes the librarian name the pair. Both questions are asked in one call per pair (the
   six nearest memories at most, together within one 2 s deadline, `crew/src/memory_judge.rs`), and a pair asked before is
   read back, not asked twice. A contradiction the decider acts on goes to the daily review as a candidate
   (`consolidate::judged_contradictions`, a finding of its own, `possible-contradiction`).
   Outcome: the librarian's word, each answer scored only by what the word says of its own question.
   - **Same lesson.** `mark_retaught` (the teaching told it again) says yes, for the pairs asked in the teaching's chain
     whose newer memory it did not write. `distinct_from` and a `related` or `because` link say no. A `supersedes` and a
     `refines` say nothing: a correction gives different advice, so it proves neither.
   - **Contradicts.** A `contradicts` link says yes. A `distinct_from` or a remember-time link says nothing, since a
     contradiction can be kept as another lesson, and so it still reaches the review. A link the librarian makes under
     a review, having read both memories to settle whether they contradict, says no unless it is `contradicts`.
   All of a call's outcomes are written in one transaction. *Built 2026-11-20:* the first draft counted a link as the
   same lesson and a supersedes as a duplicate; a link says the memory only bears on the other, and a supersedes is as
   often a correction.
4. **How deep a review goes.** `gate.risk@1` (a Score over the diff's summary, its paths and the chain's goal) sets the
   effort of the reviewer and QC: `high`, `xhigh` or `max`. `gate.touches_rail@1` adds the security gate when a diff
   changes what a rail does without matching its paths. This point only tightens: no gate is ever removed or lowered below
   today's. Outcome: a landing that holds, or one that is reverted or breaches.

   *As built (S4).* `Shipping::version_of_work` (`shipping/src/land.rs`) calls `Core::judge_change`
   (`chains/src/review_depth.rs`) once the checks passed and the version is about to be recorded, in one `judge` call.
   **What leaves the machine** (redacted as every state is): the commit's title and description, the paths it touches, the
   chain's goal, and whether the path rule matched a declared rail. Neither the diff nor the repository's rails file
   (a map of its security-critical code) leaves. `gate.touches_rail` is asked only where the path rule found no rail, since
   it only adds `security`. Each answer is stored on the landing's version (`LandingVersion::judged`, the judgment ids;
   every answered one, so one too unsure to act is scored too), and what the code did is `LandingVersion::effort` and the
   gate list. The levels of `gate.risk` are the efforts `high`, `xhigh`, `max`, read by index. **Only an answer at or above
   its act threshold acts** (0.9 until a decider has outcomes); below it the review stays as the kind and the rails make
   it. An effort is set and announced only when it is above the Chair's own (an empty effort ranks as `xhigh`, what Claude
   Code runs at, ADR-0070), since a judgment that raises no judge changes nothing. The effort is applied where the Chair
   hires any gate signer (`hiree_of`, `orch/src/chair_hire.rs`): the judge runs at `tightened(its usual, the highest effort
   a version still judging asks)`, which is the asked effort only when it is strictly above the usual, and never below it;
   a judge hired in a repository's worktree reads that repository's versions only. A judge hired earlier keeps its effort.
   What was done is told to the Chair in the land's result and recorded as Remit's decisions ("Review and QC run at max
   effort for this change", "A security review is added for this change") with the decider and its p, only when a judge
   will run (a version that needs no approval has none).
   - **Outcomes** are written in the commit that stores the evidence (`review_depth::ended`, from `commit.rs`): each
     verdict a gate records on a version, a release that went live (`ReleaseState::Live`), a landing stored as
     `Reverted`; source `Gate`. Evidence that proves nothing, and a landing that holds, settle only a judgment with no
     outcome yet, so they never replace what a gate or a revert proved; a write replayed with the same outcome changes
     nothing, so when it was first known stays. This refines the sentence above, whose "holds is Right" would score a
     decider that always says `max` perfect, and scores both ways:
     - `gate.risk`. A version a gate sent back (a request for changes, security's included) or that was taken back (a
       conflict apart) was risky: level 0 Wrong, levels 1 and 2 Right. One that went live was contained: level 0 Right,
       level 2 Wrong (it cost more than it gave), level 1 Unknown. An approval proves nothing until the release.
     - `gate.touches_rail` is scored by evidence about rails only, the security gate's verdict on that version (it runs
       only when a yes acted): it asked for changes, so a yes was Right; it approved, so the review the yes added found
       nothing, and the yes was Wrong. This is a proxy: a clean approval on a real rail change reads as Wrong. A no, and
       every other ending, is Unknown.
   - *Revisit when:* a bounded, redacted diff is worth sending to the decider; the judges already hired for an earlier
     version keep their effort, so a raise reaches them by a new hire only; the security verdict as the rail evidence
     needs a finding that says whether a rail was changed, if its approvals are mostly clean.
5. **An item for the owner.** Before the Assistant's turn, `inbox.option@1` (a Choice over the item's options) and
   `inbox.reversible@1` are asked. The Assistant is shown the answer as a prior and still decides. The second opinion
   becomes targeted: every answer where the Assistant disagrees with a decider at p ≥ 0.9, plus the random 8%. Outcome:
   the owner overturns, or the second opinion differs.
   *As built (S5).* `fire_assist` (`orch/src/assist.rs`) asks both through `Core::prior_for_item`
   (`chains/src/inbox_prior.rs`) before it sends the Assistant its message, and the message ends with what they said ("A
   fast decider (Jev 1.14) read this item before you: it would take allow (Send it), 95% sure; … It is a prior, not a
   decision"). The state is the item's kind, title and detail, who asked and the chain's goal, redacted as every
   judgment's is. The options are the item's, each named by its id, so a pick is what `answer_inbox` takes; an item with
   fewer than two options is asked `inbox.reversible` only, and a credential or a sign-in is asked nothing. The judgments
   are recorded on the subject `inbox_item`, and nothing else reads them as an instruction: no decider, a late answer or a
   refusal leaves the message and the Assistant's turn exactly as they were.
   - **Targeted second opinions.** `second_opinion_on` starts a second judge on every answer that `Prior::dissents`
     (`DISAGREES_AT` = 0.9) as well as on the 8% sample. Dissent is: the answer took an option (`Decision::option`, new,
     the id it took; `Decision::stance` stays beside it, being that option's meaning for kinds whose ids say nothing of
     it) and `inbox.option` picked another at p ≥ 0.9, an option that is the same answer in other words (`allow_once`
     against `allow_always`, compared by `stance_of`) not counting; or the answer approved (`Stance::Approve`) what
     `inbox.reversible` says at p ≥ 0.9 cannot be undone. `SecondOpinion::targeted` says it was started by a dissent
     and not by the sample.
   - **Selection.** A targeted opinion only scores picks that part from the Assistant, where the agreeing picks are
     scored only by the 8% sample, so a standing computed from both would read a decider low, the acting one lowest (its
     shadows are scored on its pick). A targeted opinion's outcome therefore has its own source,
     `OutcomeSource::SecondOpinionTargeted`: stored, counted and shown in the Judgments view as a second opinion's, and
     left out of `judgment_scores`, which every standing (Brier, AUC, act threshold, ranking) reads. A standing for
     `inbox.*` is the owner's outcomes and the sampled second opinions'.
   - **Outcomes** are one rule each, written in the transaction of the event that proves them.
     (a) *The owner overturns* an answer that took the option `inbox.option` picked: that pick is `Wrong`, source
     `Owner`, replacing what a second opinion said. An overturn proves nothing of a pick the Assistant did not take (the
     owner's word is free, and may choose none of the options) and nothing of `inbox.reversible`; the overturn decision
     carries the option of the answer it overturned (`Decision::option`).
     (b) *The second judge gives its answer* (`second_opinion`, before it reads the Assistant's): its option (new
     `SecondOpinion::option`, kept only when it is one of the item's) is the reference for `inbox.option`, `Right` when
     the decider picked it and `Wrong` when it did not; its `undoable` (new, asked only when a decider answered
     `inbox.reversible`) is the reference for that question. It scores at the judge's answer, so a judge that ends
     before comparing still scores, and only a pick with no outcome yet.
     The owner's word is final in either order. If the judge answers after the owner overturned the answer that took
     the judge's option, that reference is spent and the option's pick is not scored. If the owner overturns after, the
     outcome a judge that chose the overturned option gave to another pick is withdrawn (`clear_judgment_outcome`).
     A pick of an item the Assistant left to the owner, or answered in words, has no outcome; so has one whose answer
     was neither overturned nor re-decided. The owner's own answer to an item left to him would score it, and is not
     built.
   - **Cost.** One call of two questions before each turn on an item. Each decider is given `DECIDE_FOR` (2 s), so the
     worst case is 2 s for each decider ranked for the question and one more round for its default
     (2 s × (ranked deciders + 1)), under the 30 s `IN_PROCESS_DEADLINE` of a delivery.

### Further: what the layer makes possible

6. **A watchdog on a running card.** At each turn end, `card.progress@1` and `card.off_brief@1` are asked of the turn's
   recap against the brief. Below the threshold, a note goes to the card's Chair. It never pauses or stops anything: those
   stay rails. This catches the drift that the repeat and spin fingerprints (`chains/src/repeats.rs`, `spin.rs`) cannot
   see. Outcome: the card's result.
7. **A decider for every agent.** Capability `judgment` gives an MCP tool `ask_decider(state, questions)`. With it an agent
   screens a thousand log lines, ranks candidates or checks its citations for a fraction of a turn. Each answer is recorded
   as a judgment on the card. A fast tool an agent was missing is an agent tool first.
8. **The owner sees and steers it.** Outcomes gains a Judgments view, laid out by the server in the owner's words. Each
   question is titled by what it is for (its declaration's title). It leads with one sentence of what the figures do not say: which model answers it and why, and whether it has settled
   ("Not settled yet: until 30 results are known it decides alone only when at least 90% sure"). Three figures follow, as
   the Outcomes tab's figure cards (how often it proved right, "Right 96% of 288 checked", or "too few to tell" below 30;
   how often it decides alone and who decides the rest; what it cost), then its recent mistakes, each said as what happened ("Said two memories were the same lesson; the
   librarian kept them apart · was 93% sure") and linked to the chain it was asked in. The question's key, its threshold
   and each model's Brier score and AUC are folded under its details. A hire's line on the card's screen shows what was chosen, with its p. The owner steers through
   the Assistant (`set_decider`, `set_target_precision`), recorded with who and why, as every setting is. There is no form.

   *As built (S7).*
   - **The watchdog (point 6).** `Core::send_report` (`chains/src/turn_end.rs`) calls `Core::watch_report`
     (`chains/src/card_watch.rs`) just before a worker's report goes to its Chair, so the note reaches the Chair in the
     report's own message (a note sent after it would be read once the Chair has ruled). "The turn's recap" is the report.
     It asks for reports that end a turn or are written with `report` (stop `end_turn` or `report`; not a session that
     failed to open or a cut turn), by a card that signs no gate, and not one a duel judge reads first: those with no
     Chair verdict to follow, which is what scores the answers. What leaves the machine, redacted as every state is: the
     brief, deliverable, criteria, plan and the report (clipped to 8000 runes). With no key, no decider answering or a
     late one, the report goes as it always did. **The wait is one deadline, 2 s, around the whole asking**
     (`tokio::time::timeout(DECIDE_FOR, …)` around `judge` in `watch_report`): `judge` asks one round per ranked
     decider, each bounded by `DECIDE_FOR`, so without it a report would wait 2 s for each decider ranked (an
     earlier version of this text said a turn waits at most 2 s, which held only for one round). A test with two ranked
     deciders that are both late shows the report goes after 2 s, and fails at 4 s without the deadline. A report
     whose asking is cut off records nothing and goes as before.
     Only the alarming answer speaks, and a question says so in its declaration: `Definition::speaks` (`OnNo` for
     `card.progress`, `OnYes` for `card.off_brief`), and `judgment::use_of` gives an answer the use `Informed` (shown to
     the Chair) when its alarming pick is at or above its bar and `Held` (recorded, scored, shown to no one) otherwise.
     `judge` stores every such answer as `Held`, and the change that sends the report carrying the note upgrades the ones
     it says to `Informed` (`Records::shown`, in the commit), so an answer whose caller stopped waiting, or whose report
     never committed, is not recorded as shown.
     The bar is the act threshold measured at the question's precision (0.9 until a decider has 30 results; no
     probability reaching it says nothing); it is worked out once, in `judge` (`threshold_of`), returned on `Judged` and
     kept on the judgment, so the share of alarms that were shown reads back as it was and the page reads the bar by
     the same function. The note is one line at the foot of the report, "advice, not a verdict"; nothing is paused,
     stopped or refused.
     **Each answer is about one report.** `send_report` gives the report an id, which the card keeps (`Card::report`,
     set by every report the Chair is asked about, a failure's too: a verdict on a failed turn is not a verdict on the
     report before it, whose answers are left without an outcome), and the answers are recorded on that report (`SubjectKind::Report`,
     `JudgmentSubject::report(card, report)`).
     **Outcome, precisely** (`proved_by`, in the commit of the Chair's `accept` or `rework` decision, source `card`): an
     accepted report was moving toward its brief and on it, one sent back was not. `card.progress` yes is Right when the
     report is accepted and Wrong when sent back, no the reverse; `card.off_brief` yes is Right when sent back and
     Wrong when accepted, no the reverse. Each question is scored both ways, so a decider that never raises an alarm is
     wrong on every report sent back and cannot pass for perfect. Only the answers about the card's current report
     are proved; every open answer about an earlier report, which the card replaced before it was ruled on, is
     `Unknown`. A report that never gets a verdict (its chain ends) leaves its answers open.
   - **`ask_decider` (point 7).** Capability `judgment` (`CAP_JUDGMENT`, `writes: true`: every agent of the chain holds
     the tool once the capability is given, which the Assistant decides for the owner as it does any writing
     capability, ADR-0087; held by no kind at the start) gives `ask_decider(state, questions)`
     (`orch/src/judgment_tools.rs`, `Effect::Writes`: it records a judgment on the card and spends on the owner's
     account, so a Chair that could give itself the capability would make that spend its own). Each
     question is `{ask, form: yes_no | choice | score, yes, no | options | levels}`, checked by the same `Kind`
     constructors as Remit's own, so a question that cannot be answered is refused in words the agent can fix. All
     of them are one declaration, `agent.ask@1` (`Form::Free`, `Acts::Informs`, `scored: false`), and a `Question` built
     with `Question::worded` carries the agent's words before the declaration's sentence on treating the state as data.
     The state goes through `Core::judge`, so it is redacted as every state is, and each answer is a judgment on the
     asking card. `judge` redacts all that leaves the machine, centrally (`leaving`): the state's credential fields,
     the token shapes and emails of every text in it **and of its field names**, and every text of every question
     (the agent's words, criteria, option names, levels), so no point's question and no agent's can carry a token out.
     Two field names that clean to the same text (two masked addresses) are both kept, the later with a `#2` suffix;
     two options that clean to the same name refuse the question. The field names of every poll body are cleaned too
     (`source::clean_texts`). No point scores an agent's question, so it has no outcome and no standing, and it is asked of no
     shadow (`Definition::scored`), since the page would otherwise say another model answers beside it to be measured
     when nothing measures it. The agent reads each answer as its words (`yes`, an option, a level), its probability and
     the model; a question with no answer says so, and the agent decides it.
   - **Steering (point 8).** `Settings::steering`, by the question's id without its version, holds the model the owner
     pinned and the bar he set, each a `Setting { value, by, why, at }`, so a new wording of a question keeps them and
     each carries its own reason. `set_decider` and `set_target_precision` (`chains/src/steering.rs`), the Assistant's
     tools beside `read_judgments`, refuse a caller who is not the owner or the Assistant for him, a question not
     declared, a pin on a question that is not scored (nothing would use it), no reason, a model id that is not `vendor/model` or is a `~latest` alias, a bar outside 50 to 99
     percent (parsed once, into `model::TargetPrecision`), a bar below the question's floor, and a bar on a question
     that always informs. **A pinned model is first in the order (`ranked_by`) once it has `MIN_OUTCOMES` results of its
     own on the question**; before that it is on trial: asked beside the acting decider and shadow-scored, it only
     informs, because a model chosen on a name has no measurement of how it would decide alone. `set_decider`'s reply and
     the page say so with its count ("has 3 of the 30 results it needs … for now it only informs"), and the others
     follow a measured pin, so one that is retired never holds its question in System Two. **Each `Definition` carries a
     floor** (`Floor`: a percent and the reason): 90 where a wrong act loses something that is not taken back
     (`memory.same_claim`: a lesson merged away; `signal.screen`: an event screened out is gone), and the lowest
     precision (50) where a wrong act is taken back or only adds review (`hire.tier_can`: a hire sent back by its gate
     climbs a tier, which is why its bar may go down; `gate.risk` and `gate.touches_rail`: an act only deepens a
     review; `memory.contradicts`: the daily review reads its yes again; `card.*`: advice the Chair weighs).
     `set_target_precision` refuses below it, naming the reason, and a floor raised later holds over a setting stored
     below it. This is what keeps a lower precision from letting a decider lose something at less certainty than the
     declaration allows; the earlier claim that "a decider only acts where it tightens or saves reversible work" is
     what each declaration's `Acts` and floor now say. A bar on a question that speaks only when sure (`card.*`) is
     allowed, and the reply says it speaks to the Chair "only at" that level; below 30 results it adds that the
     provisional 90% still applies. The bar replaces the declared target precision wherever it is read: `judge` and the
     page both take it from `steering::target_precision` and the act threshold from `judge::threshold_of`. The
     Judgments page says it in the sentences it already has: "Answered by Liquid D1.2 Ultra, chosen by you:
     “…”." leads, and the sentence on the bar ends "The bar of 80% was set by you: “…”."; who and when are
     under the details (`Model chosen by`, `Bar set by`; `Speaks when` for the watchdog's questions). There is no
     form. A question agents write takes a model and no bar (nothing measures it). **Every value the owner sets is a
     `Setting<T>`**: `permission_wait_minutes` and `duel_agreement_percent` carry who set them and why as the steering
     does (migration 0127 wraps what was stored, as set by Remit before settings recorded it), and
     `set_permission_wait` and the coach's `set_duel_agreement` store their caller and reason. `Card::report` is the one
     wire addition (an id a client need not read).

   *Revisit when (S7):*
   - **The note makes a rework likelier.** A Chair that reads "off its brief" sends the report back more often, so an
     alarm's precision reads higher than the decider earned. Hold out a share of alarms (asked and recorded, not
     said) to measure it, once a question's alarms are numerous.
   - **A report sent back for a flaw that is not drift** scores a "fine" answer wrong. The Chair's feedback could say
     whether the report drifted; until it does the Chair's verdict is the proxy.
   - **Reports by the thousand:** `store::open_report_judgments` finds a card's reports by a GLOB on the subject id, as
     `pair_judgments_of` does by memory; a column and an index on the subject join them then.
   - **A self-bias hold-out is not built.** What it needs is kept: each watchdog judgment records the bar it was held to
     and whether it was shown (`Judgment::threshold`, `used` `Informed` or `Held`), so a later change can hold out a
     share of alarms (ask and record them, say nothing) and compare their results with the shown ones.
   - **A floor is declared, not measured.** The floors say what a wrong act costs; they are not learned from outcomes. A
     question whose floor is too low or high is changed in its declaration, which is a new version of it.
   - **`ask_decider`'s state and the number of questions are not bounded** (a decider the provider refuses answers
     none), and the pinned model id is not checked against OpenRouter's list (an unknown one fails and the next in
     order answers). A limit and the check join the hardening the owner asks for.
   - **A new version of a question keeps the owner's steering** (it is kept by id) though its standings start over: a
     bar set for the old wording governs the new until he sets another.

### Built in slices, each whole

S1 is the layer itself, with its view (`judgment`, `openrouter::decide`, `model::Judgment`, the migration, wire, the
Judgments view, shadows and standing). Then S2 memory (point 3, measured), S3 hire with the climb (1), S4 gate (4), S5
inbox with targeted second opinions (5), S6 signal screens (2), and S7 the watchdog, `ask_decider` and the steering tools
(6–8).

## Consequences

- Small judgments get cheaper by orders of magnitude, and better: a hire stops defaulting to the dearest model, duplicates
  are found at 7–8 of 8 instead of 2 of 8, and second opinions go where a disagreement is.
- Every judgment has a measured standing. A better decider shows up on OpenRouter as a shadow and takes over by its
  numbers, with no change to code.
- What a judgment reads leaves the machine: briefs, memories, readings' bodies and inbox items go to OpenRouter and to the
  decider's provider. Secrets are redacted as they are for sources. A reading from a customer system (Intercom) may carry
  personal data.
- A new table, a new view and one more external dependency. Losing the dependency changes nothing but cost and quality:
  System Two runs as today.

## Revisit when

- **Which state may leave the machine:** `MayLeave` on each question's declaration is the seam (one variant today,
  `Redacted`) and `EventSignal::kept` is the per-event-signal one S6 built. The control waits until the owner asks for
  hardening: deny by default for customer readings (Intercom's), a per-plugin declaration, a cap on a reading's size sent, and
  a decider run locally. Threat modelling (`aio-threat-models`) runs then.
- **A screen's outcome is the chain's hiring, not its words:** a Chair that does the work itself and hires nobody reads as
  "nothing to do". When the figures show screens proved wrong that way, the Chair's finish says it had nothing to do
  and that is the outcome's source. One standing is kept for all screens (`signal.screen@1`); a screen that is far harder
  than the rest gets a key of its own then.
- **A burst is screened a pass at a time:** `SCREENED_PER_PASS` firings a pass, so a flood waits for the next Ticks. A decider
  that answers fast and in bulk (a batch of readings in one call) would lift that, and would let a screen judge a `batch`'s many
  readings, which is refused today. Judgments are read by subject with no index (`Reader::reading_judgment`, `screen_of_start`
  scans one subscription's firings); one joins them when they are many.
- **The screen's edges:** `Pipeline::sleep_for` evaluates without the screen, so the Tick may wake for a firing a screen then
  drops; an overlap window read before the deciders answered may be stale when the record is made, and the record then redoes the
  pass; a `delay` flow counts a dropped firing as fired when it spaces the next. Each costs a wake or a pass, never a wrong firing.
- **A local decider becomes good enough** (a model on this machine through the same port): measure it as a shadow first.
- **The Decisions API leaves alpha or changes shape:** only `openrouter::decide` changes. Its contract is read in a test
  from the guide's own response example and a refusal (`remit/src/ports.rs`).
- **A replayed firing asks twice:** a firing delivered again after a crash between the asking and the message asks
  the deciders again; the first asking's judgments keep no outcome (`item_prior` reads the newest). Read the first back
  instead when it shows.
- **Item judgments by the thousand:** `store::item_judgments_of` reads an item's judgments by scanning the two questions'
  rows for its subject (as `pair_judgments_of` does); a column and an index on the subject join them then.
- **The owner's own answers score the item questions:** an item left to him, answered by him, proves each pick right or
  wrong at once, the strongest outcome there is, and the cheapest. It needs the answer paths of every kind of item
  (`orch/src/answer.rs`) to record it, so it waits until `inbox.option` has outcomes to show it is worth it.
- **A memory or an inbox item gets a screen of its own:** a mistake about it links there instead of to its chain.
- **A question's outcomes stay `Unknown` for most judgments:** its outcome source is wrong; redesign that question before
  trusting its thresholds.
- **Memory pairs judged by the thousand:** `store::pair_judgments_of` finds a memory's pairs by scanning the question's
  non-shadow judgments (the subject is inside the document); a column and an index on the subject join them then. A
  review that settles a contradiction by retracting a memory, or by superseding both, proves nothing yet: only a link
  does. `mark_retaught` scores the pairs asked in its teaching's chain since the teaching was recorded and not written by
  it, which is exact unless one teaching was refused beside two near memories and taught one of them again.
- **A decider's standing drifts down after a model update** (`typesafe/jev-1.14` to a later version): the standing is per
  pinned model id, and a `~latest` alias is never used.
- **A crash between a failed turn's commit and the climb's first commit:** the card then stays failed with its Retry not
  filed and no hand-over begun, so nothing resumes it. The hand-over itself replays (`Card.handover`, `finish_handovers`),
  and a gate's climb that a crash lost is made again by the next send-back (the trigger is `>=`, and `tier_above` is none
  once a card is handed over). The gap before a failed turn's hand-over is not closed: record the climb as the failure's
  own commit when it is seen.
- **Tier standings mix under one key:** `hire.tier_can` is one question key for every tier, so a standing blends the
  answers about `fast` with those about `workhorse`. Split the key per tier if the blend hides a tier a decider is bad at.
- **`hire.effort` informs only:** it is asked, recorded, shadowed and shown (the hire's decision says what a decider
  suggested), but no event says an effort was too low, so no outcome can score it wrong and an answer scored only right
  would read as a perfect precision. Let it act when an outcome source writes `Wrong` for it (a turn that ran out of
  effort, a gate finding that names thinking too little); until then the effort is the agent's, else the Chair's.
- **A shared change is raised for no one:** a gate raises the author of a change only when exactly one card whose model
  code chose made it (`version.by`); with two, or with an author the Chair named, the weak model cannot be told from the
  change. Attribute a change to its authors' hunks when an outcome source can, and raise each.
- **A broken turn is not proof of a weak model:** a failed turn raises the card (it is the recovery) but scores the choice
  `Unknown`; only a gate's rounds write `Wrong`. Score a failed turn `Wrong` when a cause can be told apart (a turn that
  hit a context limit, as against Claude Code ending its turn).
- **A raised card's reviewers:** the new card is given the old card's reviewer edges, so a reviewer restarted is handed
  the new author's report beside the old one's. A gate that reads who a verdict is for from the change would replace the
  edge.
