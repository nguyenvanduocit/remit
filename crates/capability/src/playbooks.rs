//! The playbooks: reference content on how a kind of work goes well, a brief and the agents it suggests, which a Chair
//! reads when it fits and which binds no one (ADR-0113). The built-in ones are written at every start, unless the owner
//! edited one; the owner makes and changes the others by telling an `author` chain (playbook_tools.rs).

use model::{Actor, ChangeKind, Playbook, Seed};
use store::Tx;

use crate::Capabilities;
use crate::okr::{MEASURE_RULES, OKR_SEAT_CRAFT};
use crate::over::{Named, Over};
use chains::chain_core::optional;
use chains::changes::{Authorship, record_change, same, the_seed};
use chains::{Error, Result};

/// Starts the brief of every agent of the self-development playbook: that chain edits the system it runs on, and an
/// agent that does not know it reads its task as any other repository's.
const SELF_DEVELOPMENT_OPENING: &str = "You work on Remit itself, the system that runs you: your brief, your tools and the gates that judge you are this repository's code, and a change reaches only chains started after it lands and its release is installed, never this one. Before you design or edit, read and cite what applies of AGENTS.md (it wins over your general rules), docs/design.md, docs/adr/README.md and the ADRs for the area, crates/orch/README.md and crates/builtin/skills/app. Work in the chain's worktree and commit there; the Chair lands the change. Never push, restart, rebuild or redeploy the running instance: Remit's queue lands each change, and the repository's .remit/release/ releases it and installs it on every instance, by code. The database, secret.key and every credential are off limits. ";

/// The longest id a playbook may have.
const MAX_PLAYBOOK_ID_LEN: usize = 63;

/// A seed of a built-in playbook.
fn seed(agent: &str, name: &str, brief: &str) -> Seed {
    Seed {
        agent: agent.to_owned(),
        name: name.to_owned(),
        brief: brief.to_owned(),
        ..Seed::default()
    }
}

/// The playbooks written at every start: reference a Chair may read or ignore.
pub(crate) fn default_playbooks() -> Vec<Playbook> {
    vec![
        Playbook {
         id: "discovery-fix".to_owned(),
         name: "Discovery → Fix".to_owned(),
         group: "engineering".to_owned(),
          description: "Find the root cause of a problem, fix it, review and verify.".to_owned(),
         brief: "Prove the root cause with evidence before anyone changes code. Choose the fix with the best value and least risk yourself and say why. Have an independent reviewer read the diff and verify the fix (record_verdict), then land it.".to_owned(),
         seeds: vec![
          seed("research", "Research", "Reproduce the problem and find its root cause with evidence (file:line, data, logs). Two competing hypotheses, each with a test that would kill it."),
          seed("architect", "Architect", "From the research, propose at least two solutions, compare them (second-order effects, pre-mortem), recommend one."),
          seed("engineer", "Engineer", "Implement the approved solution on a branch with a failing test first; run the build and tests."),
          seed("reviewer", "Reviewer", "Review the change against the goal and the solution; findings with file:line and severity; verdict. Reproduce the problem on the changed code and show it is gone, then run the tests and the neighbouring cases the fix could break; report what you ran and saw, not the engineer's claims; verdict."),
         ],
         ..Playbook::default()
        },
        Playbook {
         id: "chore".to_owned(),
         name: "Chore".to_owned(),
         group: "engineering".to_owned(),
          description: "A small, well-understood change (a typo, a config value, a rename, a dependency bump, a one-file fix): one engineer makes it and one reviewer checks it.".to_owned(),
         brief: "Keep it small: the goal already says what to change, so hire no research, architect or design. One engineer makes the smallest change that does it, with the check written first, and one reviewer reads the diff and runs the check. Plan in a few steps and brief tightly. If the engineer finds the task is bigger than a chore (a design choice, several repositories, a root cause still unknown), stop growing it: finish_chain saying so, and say it needs a work chain.".to_owned(),
         seeds: vec![
          seed("engineer", "Engineer", "Make the smallest change that does the goal, in the repository's conventions, with the check that fails before it and passes after; run the build and that check; report what changed (file:line) and what you ran. If it turns out bigger than a small change, stop and say why."),
          seed("reviewer", "Reviewer", "Read the diff. Run the check and the changed behaviour yourself, then the nearest behaviour it could break; report what you ran and saw, not the engineer's claims; verdict."),
         ],
         ..Playbook::default()
        },
        Playbook {
         id: "feature".to_owned(),
         name: "Feature development".to_owned(),
         group: "engineering".to_owned(),
          description: "Build a feature: acceptance criteria, design, implementation in slices, independent review.".to_owned(),
         brief: "Fix what done means before anything is built: acceptance criteria that can be checked by running the feature. Design before code, and follow the repository's existing patterns rather than inventing new ones. Split the work into slices that can each be reviewed and verified alone, and build nothing the goal does not ask for. A product choice the goal leaves open: take the most conservative reading and list it under assumptions in the landing's description, so the owner can overrule it when he reads what landed. Technical choices are yours: take the best value for the least risk and say why.".to_owned(),
         seeds: vec![
          seed("research", "Research", "Map where the feature belongs: the code paths it touches, the closest existing feature and the conventions it follows, the tests that cover the area. Everything with file:line."),
          seed("architect", "Architect", "From the research, write the acceptance criteria and the design: components, data and interfaces touched, at least two options compared (second-order effects, pre-mortem) with one recommended, and the slices to build in order."),
          seed("engineer", "Engineer", "Build one slice of the approved design on a branch with a failing test first, in the repository's conventions; run the build and tests; report what the slice does and how you verified it."),
          seed("reviewer", "Reviewer", "Review the change against the goal, the design and the repository's conventions: correctness, scope beyond the goal, missing tests; findings with file:line and severity; verdict. Run the feature against each acceptance criterion, then the edge cases and the neighbouring behaviour it could break; report what you ran and saw per criterion, not the engineer's claims; verdict."),
         ],
         ..Playbook::default()
        },
        Playbook {
         id: "canary-release".to_owned(),
         name: "Canary release".to_owned(),
         group: "engineering".to_owned(),
          description: "Build a change and put it live yourself: land it dark behind a flag, grow it stage by stage while guardrails are watched, and take it back the moment one breaches.".to_owned(),
         brief: "You own this change until it reaches everyone and has proved itself. It reaches its base only through land, and a version with a plan lands as one stage of a rollout: land refuses a stage unless the required gates approved the version, the previous stage on the repository has baked and every guardrail reads and holds, and a guardrail that breaches during the bake has the landing reverted by Remit, so safety lives in the stages, not in hesitation. Design the rollout before any code. (1) Blast radius: who the change can hurt and how fast it shows; a change nobody can be hurt by is one stage with a short bake. (2) Parallel running: the old path stays the default and keeps working beside the new one until the new one is proven; for a risky data or money path, run the new one in shadow (computed and compared, the old result returned). (3) The flag: the repository's own mechanism (a config value, an environment variable, a flag service); if it has none, the first change adds the smallest one, read at run time, defaulting to off. (4) The guardrails: signals that already read (list_signals, read_signal), each with the breach condition run on a real reading; a signal that does not exist is built first, as a sensor or an event the change emits, and land refuses a guardrail with no reading. (5) The stages and their bake: the first landing is dark (exposure 0%, behaviour identical to before), then a small share, then more, then everyone; a bake covers the repository's pipeline time plus enough traffic to show harm. (6) The way back for every stage, written in its plan before it lands. Run it in this order. The engineer builds the dark change and its signal; the reviewer reads the rollout and runs both paths and the rollback. Then land stage one. After each landing confirm the deploy by a signal, never by assuming the pipeline ran, and read the guardrails; you are woken on a breach. Ramp only by a flag change that is judged and landed like any change. At the end remove the old path and the flag in a last landing, so nothing is left half-migrated. A breach, or a reading you cannot trust, is a reason to take the stage back first and understand second: revert_landing it, then find the cause with evidence. A way back is never a failure; shipping a breach is. Tell the owner through ask_owner only for a decision that is theirs (a product choice, a credential, a deploy the pipeline does not do). The goal's check is a reading over the whole window after the last stage, and the chain finishes only when the judge has seen it.".to_owned(),
         seeds: vec![
          seed("research", "Research", "Map the change's blast radius: the code paths and data it touches, who and what depends on them, how the repository deploys (what runs on merge, what is manual), whether it already has a flag mechanism, and which signals already read the health of this area with their newest readings. Everything with file:line or a reading."),
          seed("architect", "Architect", "From the research, write the rollout plan: the flag and where it lives, how the old and new paths run in parallel, each stage (exposure, bake, guardrail key and breach condition proved on a real reading) and the way back for each, and the last change that removes the old path. At least two ways to stage it compared, one recommended."),
          seed("engineer", "Engineer", "Build the change behind the flag on a branch, failing test first: with the flag off the behaviour is identical to before (prove it by a test that runs both states), with it on the new path works. Build the signal or sensor the plan needs when none reads. Run the build and tests; report what each flag state does."),
          seed("reviewer", "Reviewer", "Review against the goal and the rollout plan: with the flag off nothing changes, the new path cannot reach users before its stage, the way back restores the old behaviour, the guardrail reads the harm the change could do. Findings with file:line and severity; verdict. Run both paths: the flag off must equal the behaviour before the change, the flag on must meet each acceptance criterion; then take the way back and show the old behaviour returns. Report what you ran and saw, not the engineer's claims; verdict."),
         ],
         ..Playbook::default()
        },
        Playbook {
         id: "bug-sweep".to_owned(),
         name: "Bug-class sweep".to_owned(),
         group: "engineering".to_owned(),
          description: "One kind of bug in every repository: scan, triage, fix, and a guard against its return.".to_owned(),
         brief: "Start from one confirmed instance and prove it with file:line. Write the defect's signature as a command anyone can re-run (rg or sg) and check that it matches the instance, matches nothing correct and does not match the fixed form. Scan every repository the goal covers: the goal names them, and when it does not, ask the owner which. Each repository is scanned or dropped with a reason, and the result carries the coverage ('scanned N/M, skipped K because ...'): a sweep that does not show its coverage is not done. Triage every hit as a defect, a false positive or unreachable (check serverless.yml, a Helm chart or a route) with file:line, and fix only what you triaged. An agent works in one repository: one engineer and one landing per repository, each with a failing test first. Add a guard so the class cannot come back (a test or lint rule in the repository's own tooling, or a shared helper), or say why none is possible. A variant you find along the way is listed as a follow-up with its own signature.".to_owned(),
         seeds: vec![
          seed("research", "Signature", "Prove the first instance with file:line, then write the defect's signature as a command anyone can re-run (rg or sg). Show that it matches the instance, matches nothing correct and does not match the fixed form."),
          seed("research", "Scan", "Run the signature over every repository the goal covers. Per hit: file:line, a verdict (defect, false positive, unreachable) and why. End with the coverage: repositories scanned of the total, and each one skipped with the reason."),
          seed("engineer", "Engineer", "Fix the defects of one repository on a branch with a failing test first; add the guard that stops the class returning; run the build and tests; report each hit fixed by file:line."),
          seed("reviewer", "Reviewer", "Review the change against the goal: every defect of the repository is fixed, nothing else is touched, the guard fails on the old code; findings with file:line and severity; verdict. Run the signature on the changed repository: no defect is left. Run the tests, and the guard against the old code to see it fail; report what you ran and saw."),
         ],
         ..Playbook::default()
        },
        Playbook {
         id: "wrong-number".to_owned(),
         name: "Wrong-number investigation".to_owned(),
         group: "data".to_owned(),
          description: "A number users see is wrong: the expected value, the trace, the fix at its source.".to_owned(),
         brief: "Define the right number before looking for the cause: what it must be for one concrete case, recomputed by hand from the source of truth (the formula in the code or the vault, and the data it starts from). If the formula itself is disputed, that is the finding: report it and do not pick one. Trace the wrong number from where it is produced to where it is shown, hop by hop with file:line, and find the first hop that diverges. Fix it where it is produced, never where it is shown. List every other consumer of the same value (reports, caches and their invalidation, exports, recalculations) and say for each whether it is right after the fix. A number already stored wrong needs a way to correct it: a recalculation, or a backfill prepared and never run, since writes to shared systems are refused. Done means the recomputed case matches, not that the tests pass.".to_owned(),
         seeds: vec![
          seed("research", "Expected", "Write the right number for one concrete case: inputs, formula, result recomputed by hand, with the source of the formula (file:line or the vault document). Say so when the formula is disputed."),
          seed("research", "Trace", "Follow the wrong number from where it is produced to where it is shown: numbered hops with file:line, the first hop that diverges from the expected number, any hop you did not open marked 'not read'."),
          seed("architect", "Impact", "From the trace, list every other consumer of the value and every place already holding it wrong; propose the fix at the producer and a way to correct stored data, recommending one."),
          seed("engineer", "Engineer", "Fix the producer on a branch with a failing test that encodes the expected case; run the build and tests; report the number before and after."),
          seed("reviewer", "Reviewer", "Review the change against the goal and the expected number: fixed at the producer, other consumers right, stored data covered; findings with file:line and severity; verdict. Recompute the expected case through the changed code and report both numbers side by side; check the neighbouring cases (zero, negative, empty, another currency) that the same code serves."),
         ],
         ..Playbook::default()
        },
        Playbook {
         id: "backfill".to_owned(),
         name: "Backfill / migration".to_owned(),
         group: "data".to_owned(),
          description: "Prepare a backfill or migration for the owner to run: dry-run, idempotent, with rollback.".to_owned(),
         brief: "Prepare only: never run a migration or a backfill against a shared store, and never deploy; the owner runs it. The deliverable is a landed change with the script and its runbook. The script touches an exact, stated set of records, and a query counts them. It does nothing by default: a dry-run prints what it would change, and writing takes an explicit flag. It is idempotent (a second run changes nothing), works in batches with a resume point, and has a rollback or says plainly why it has none. The queries that prove it worked come with it. Test it on fixtures or a local copy, never on live data. The cause of the bad data is fixed by its own change on the write path, not by the backfill, so the problem does not come back. Say what is irreversible and what the owner must check before running.".to_owned(),
         seeds: vec![
          seed("research", "Scope", "Find which records are wrong or missing and why: the query that counts them, a few concrete examples by id, and the cause with file:line. The cause is fixed by a separate change."),
          seed("engineer", "Script", "Write the script on a branch with tests on fixtures: dry-run by default, writing only behind an explicit flag, idempotent, batched with a resume point. Report how to run it and what it prints."),
          seed("reviewer", "Reviewer", "Review it as code that will run once on production data: wrong record set, writes that are not idempotent, unbounded batches, missing rollback; findings with file:line and severity; verdict. Run the dry-run, then the real run twice, on fixtures or a local copy; report the counts before and after, that the second run changed nothing, and that the proving queries catch a seeded bad record."),
         ],
         ..Playbook::default()
        },
        Playbook {
         id: "self-development".to_owned(),
         name: "Self-development".to_owned(),
         group: "engineering".to_owned(),
          description: "Improve Remit itself: a feature, a fix or a cleanup in this repository, designed against its own agreements, built, reviewed and verified.".to_owned(),
         brief: "This chain changes the system that runs it: you, your agents, this brief, the tools and the gates around you are this repository's code, so every change here changes yourself. It reaches chains started after it lands and its release is installed, never this one: do not lean on a change of yours to get this chain through. The work is Remit's own repository, at the remote the owner gives in the goal: open_repo it, and when the goal names none or it cannot be opened, finish_chain saying so. Never restart, rebuild or redeploy the running instance, never start a second copy, never touch ~/.remit/remit.db, and never push: each change lands through land once the reviewer and, when it touches a path .remit/rails names, security approved its version, and Remit releases it by the repository's .remit/release/: each instance installs the release that carries it, bakes it under fact:remit.health and takes it back by code if it hurts. Hire the Security reviewer whenever the change touches a rail. The worktree starts from the default branch on its forge, which can be behind the code that runs live: say in the landing what you could not see because of that. AGENTS.md binds the whole chain and wins over your general rules. Every agent you hire, from a seed or not, starts from the same opening the seeds carry: copy it ahead of the task into the brief of any agent no seed covers. Read how Remit has done before you design: list_chains and read_chain for earlier chains on the area. The server's own logs are the owner's: ask for what you need from them in your result. Fix what done means first, as acceptance criteria checked by running the thing, and take the smallest change that solves the root cause. The docs change with the behaviour (docs/design.md, the map in crates/orch/README.md, the app skill's references, an ADR for an architectural choice); a change to a brief, a playbook or an agent's instructions says in its description what behaviour it should change. Each slice that can be judged alone lands through land, with its priority and the line saying what waits on it, and finish_chain once every one has landed: Remit's release takes it from there, one release at a time. The description says what changed and why, the docs and ADRs followed or added, each acceptance criterion with what was run and seen, the threat model when a rail is touched, what in fact:remit.health (healthy, failures, toolErrors) would show it hurts, how it is taken back, whether it migrates the database (a one-way release, whose way back is a fix landed forward), and under assumptions every product choice the goal left open. Technical choices are yours: take the best value for the least risk and say why. A product choice the goal leaves open: take the most conservative reading and list it under assumptions.".to_owned(),
         seeds: vec![
          seed("research", "Research", &[SELF_DEVELOPMENT_OPENING, "Say which rules of those documents bind this goal. Then map where the change belongs: the packages and contract entries it touches, the nearest existing entity or tool and how it is stored, pushed and shown, the tests that cover the area, and what earlier chains (list_chains, read_chain) show about that area. Everything with file:line or the document section."].concat()),
          seed("architect", "Architect", &[SELF_DEVELOPMENT_OPENING, "From the research, answer which agent does this, with which tool, and how the owner sees and steers it. Say what the change makes the next Chair and its agents do differently. Write the acceptance criteria and the design (contract, migration, orchestration, view, docs), at least two options compared (second-order effects, pre-mortem) with one recommended, an ADR when the choice is architectural, and the slices to build in order."].concat()),
          seed("engineer", "Engineer", &[SELF_DEVELOPMENT_OPENING, "Build one slice of the approved design in the chain's worktree with a failing test first, in the repository's conventions; delete what it replaces; update docs/design.md, the map in crates/orch/README.md and the app skill where the behaviour they describe changed; run cargo clippy -p <crate> --all-targets -- -D warnings and cargo test -p <crate> for each crate you edited, and make test-changed FILES=\"<files>\" for web/ and scripts/; commit in the worktree and report what the slice does and how you verified it."].concat()),
          seed("reviewer", "Reviewer", &[SELF_DEVELOPMENT_OPENING, "Review the change against the goal, the design, AGENTS.md and the ADRs it touches: a concept with two owners, copied logic, a flag in disguise, a form where an agent should act, a rail an agent could now move, residue of the old code, docs and the map in crates/orch/README.md left behind, a landing description that would not let the owner understand it after; findings with file:line and severity; Run the change against each acceptance criterion, then the neighbouring behaviour it could break; run make test and cargo clippy --workspace --all-targets -- -D warnings yourself. For anything the owner sees, serve it through web/verify, use it as the owner will (click, drag, keyboard), measure with getBoundingClientRect and read the screenshot; report what you ran and saw, including what was wrong, not the engineer's claims; verdict."].concat()),
          seed("security", "Security", &[SELF_DEVELOPMENT_OPENING, "Hired when the change touches a path .remit/rails names (it is read from the default branch, and the file itself counts). Run a threat model of the change with the aio-threat-models skill: the flows it touches, their trust boundaries, each threat with the control that stops it (file:line) or the finding that none does, and what an agent of Remit could now reach or steer that it could not before; run the tests that guard the rail. Your verdict's notes carry the threat model, and the landed commit names them; sign the security verdict."].concat()),
         ],
         ..Playbook::default()
        },
        Playbook {
         id: "prd-stories".to_owned(),
         name: "PRD → Stories".to_owned(),
         group: "product".to_owned(),
          description: "Turn a product requirement into stories, acceptance criteria and a solution sketch.".to_owned(),
         brief: "Product decisions belong to the product's PM, a person, not to any agent here: draft, and list open questions instead of deciding them.".to_owned(),
         seeds: vec![
          seed("pm", "PM", "Write the missing user stories with testable acceptance criteria, linking the PRD."),
          seed("architect", "Architect", "Sketch the solution for each story: components touched, risks, open questions."),
         ],
         ..Playbook::default()
        },
        Playbook {
         id: "objective".to_owned(),
         name: "Objective".to_owned(),
         group: "objectives".to_owned(),
         description: "Shapes a big or half-formed request into an objective of key results, each with an owner and a check, for the Assistant to apply for the owner.".to_owned(),
         brief: [OKR_SEAT_CRAFT, " You do not do the work the request names: you turn it into an objective, and the chains that serve its key results start only once it is applied. Analyse first: who the customer is and what they get, the outcome, the north star, the guardrails, the horizon. Then draft with draft_objective, the first thing you write and without interviewing the owner: ", MEASURE_RULES, " Before you draft, list_signals and read_signal for the number each measure rests on. Until a signal exists, the key result is the work of building it, as a command; you make the signal a key result reads yourself (give your chain signals with grant_capability, then create_event_signal and create_standing_sensor), and never ask the owner to make one. Every unknown goes in the objective's unknown list with the question that would settle it, and you ask the owner (the question inbox kind) only when an unknown blocks a key result's check. A key result's owner and judge are the owner unless a Chair and a judge are worth hiring for it: propose them with the draft. A finding on the draft is answered once, in the draft; one still open waits for the owner as an exception. The owner corrects the draft by talking to you: each correction is a new draft over the one he read. Nothing is active until it is applied in his inbox, where the Assistant decides it for him and he may overturn it; you never apply it. Once it is applied, finish_chain: the objective is your product."].concat(),
         seeds: vec![
         ],
         ..Playbook::default()
        },
        Playbook {
         id: "okr-review".to_owned(),
         name: "OKR review".to_owned(),
         group: "objectives".to_owned(),
         description: "Audits the owner's active objectives against the measure rule and amends what a signal can already read.".to_owned(),
         brief: [OKR_SEAT_CRAFT, " You audit the objectives the goal names, or every active one when it names none: read_objective each, then have the OKR Auditor check its guardrails and key results against what the signals really read. ", MEASURE_RULES, " For each objective, report which measures pass the dry run and which fail and why. Amend with draft_objective only what the signals that already have a reading can settle: a guardrail or a key result rewritten as a reading on a real field, or moved out of the objective into the brief of the chains when it is a working rule and no number. A measure with no signal is not drafted: report it to the owner as the signal that has to be built (the playbook okr-instrument builds it). One amendment per objective, keeping the ids of the key results you keep; it waits in his inbox, where the Assistant decides it for him, and you never apply it. Finish with finish_chain: one line per objective, what passes, what you amended, what waits on a signal."].concat(),
         seeds: vec![
          seed("okr-auditor", "Auditor", "Audit one objective: for every guardrail and every reading key result read the signal and run the condition on its real reading; report pass or fail per measure with the reading you used, its age and what the condition gave, and name the measures a person could inflate. Edit nothing."),
         ],
         ..Playbook::default()
        },
        Playbook {
         id: "okr-checkin".to_owned(),
         name: "OKR check-in".to_owned(),
         group: "objectives".to_owned(),
          description: "Reads where each key result stands on its signal, what moved since last time, and which measure has gone stale.".to_owned(),
         brief: [OKR_SEAT_CRAFT, " You report; you amend nothing unless the goal asks for it. For every active objective the goal names (all of them when it names none) read_objective, then have the OKR Analyst read each measure's signal. ", MEASURE_RULES, " Per key result: its standing (a reading key result is met or not met by its signal alone), the value the newest reading gave and when it was made, and the move against the readings before it. Say which measures are stale (no reading for longer than the signal's own rhythm), which cannot be read at all, and which guardrail is close to its bound. Do not call a key result met because a chain said so. Finish with finish_chain: a short table per objective, then the one thing the owner should decide, if there is one."].concat(),
         seeds: vec![
          seed("okr-analyst", "Analyst", "Read every measure of the objective on its signal: the newest reading with its time, the readings before it for the move, and the condition's result. Report numbers as read; say 'no reading' where there is none, and invent no figure."),
         ],
         ..Playbook::default()
        },
        Playbook {
         id: "okr-instrument".to_owned(),
         name: "Build the signal a measure needs".to_owned(),
         group: "objectives".to_owned(),
          description: "A measure needs a number no signal carries: build the signal, with a test, until it reads for real.".to_owned(),
         brief: [OKR_SEAT_CRAFT, " A measure the owner wants has no signal, so nothing can be measured yet. ", MEASURE_RULES, " The deliverable is a signal that reads: list_signals shows its key and read_signal shows a real reading made by the instrumentation, and the chain's result names the fields of that reading's value so the objective's author can write the condition. Choose the route with the best value for the least risk and say why: an event signal the system posts to or emits to, a sensor of a source Remit has, a plugin or a script a sensor runs, or a counter in the product that a source reads. You make the signal yourself with create_event_signal, and ask for the sensor that reads it with create_standing_sensor, which waits for the owner's Apply in his inbox; never ask him, or the Assistant, to make either, and never tell him the steps. Team and agent traffic is excluded or told apart at the source, since a measure the team can inflate measures the team. A credential, a sign-up or an account only the owner can give goes to him as a question in his inbox, naming exactly what you need; you work around nothing silently. Build the instrumentation as a change, landed, with a test that fails first and shows an event produces a reading; do not draft or amend an objective, the owner's shaping chain does that once the signal reads."].concat(),
         seeds: vec![
          seed("okr-analyst", "Analyst", "Name the number the measure rests on, the event that makes it, who writes that event, and the routes that would put it in a signal, each with its cost and who could inflate it; recommend one. Say which fields the reading's value must carry."),
          seed("engineer", "Engineer", "Build the chosen instrumentation on a branch with a failing test first: an event produces a reading of the signal; run the build and tests, then show the signal's real reading with read_signal and report its fields."),
          seed("reviewer", "Reviewer", "Review the instrumentation against the goal: it reports what the measure counts and nothing else, the team's own traffic is excluded or told apart, no personal data leaves, and the test fails on the old code; findings with file:line and severity; verdict. Produce the event the way a real user or system would and show the signal's reading change with read_signal; report the reading before and after and what you ran, not the engineer's claims; verdict."),
         ],
         ..Playbook::default()
        },
    ]
}

/// The shipped playbook id, as stored when it ships: built in.
pub(crate) fn default_playbook(id: &str) -> Option<Playbook> {
    default_playbooks()
        .into_iter()
        .find(|p| p.id == id)
        .map(|p| Playbook { builtin: true, ..p })
}

/// Whether id is a playbook's: lowercase letters, digits and dashes, starting with a letter or a digit.
fn valid_playbook_id(id: &str) -> bool {
    let mut chars = id.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && id.len() <= MAX_PLAYBOOK_ID_LEN
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// p as it would be written, or why it cannot be.
pub fn prepared_playbook(mut p: Playbook) -> Result<Playbook> {
    if !valid_playbook_id(&p.id) {
        return Err(Error::refused("id must be lowercase letters, digits and dashes"));
    }
    p.name = p.name.trim().to_owned();
    p.description = p.description.trim().to_owned();
    p.brief = p.brief.trim().to_owned();
    if p.name.is_empty() {
        return Err(Error::refused("name is required"));
    }
    let mut seeds = Vec::with_capacity(p.seeds.len());
    for (i, mut s) in std::mem::take(&mut p.seeds).into_iter().enumerate() {
        s.agent = s.agent.trim().to_lowercase();
        s.name = s.name.trim().to_owned();
        s.brief = s.brief.trim().to_owned();
        if s.agent.is_empty() || s.brief.is_empty() {
            return Err(Error::refused(format!(
                "agent {} needs an agent and a brief",
                i.saturating_add(1)
            )));
        }
        if s.name.is_empty() {
            s.name.clone_from(&s.agent);
        }
        s.effort = chains::failover::resolve_effort(s.effort.trim())
            .map_err(|e| Error::refused(format!("agent {}: {e}", i.saturating_add(1))))?;
        seeds.push(s);
    }
    p.seeds = seeds;
    let def = default_playbook(&p.id);
    if let Some(def) = &def {
        p.group.clone_from(&def.group); // which column a built-in stands in is code
    } else {
        p.group = model::parse_group(&p.group).map_err(|e| Error::refused(e.to_string()))?;
    }
    p.builtin = def.is_some();
    p.edited = true;
    Ok(p)
}

/// Stores playbook p, as `prepared_playbook` made it, in the transaction tx with its change, at at.
pub fn put_playbook(tx: &Tx<'_>, w: &Authorship, at: model::Time, p: Playbook) -> Result<Playbook> {
    let cur = optional(tx.playbook(&p.id))?;
    let p = tx.put_playbook(p)?;
    record_change(tx, w, at, ChangeKind::Playbook, &p.id, cur.as_ref(), Some(&p))?;
    Ok(p)
}

/// Playbook id as a refused save names it.
fn named_playbook(id: &str) -> Named<'_> {
    Named {
        what: "playbook",
        id,
        create: "create_playbook",
        update: "update_playbook",
        reread: "read_playbook",
    }
}

/// Who writes as by, with nobody letting it through.
fn written_by(by: &Actor) -> Authorship {
    Authorship {
        by: by.clone(),
        approved_by: None,
    }
}

impl Capabilities {
    /// Writes the default playbooks, replacing earlier versions of them unless the owner edited one; other playbooks are
    /// kept. An edited one still gets the group this version gives it: it is code.
    pub async fn seed_playbooks(&self) -> Result<()> {
        self.retire_playbooks().await?;
        for p in default_playbooks() {
            let mut p = Playbook { builtin: true, ..p };
            let key = p.id.clone();
            let cur = optional(self.store.read(move |r| r.playbook(&key)).await)?;
            if let Some(cur) = cur.as_ref().filter(|c| c.edited) {
                if cur.group == p.group {
                    continue;
                }
                p = Playbook {
                    group: p.group,
                    ..cur.clone()
                };
            }
            if let Some(cur) = &cur
                && same(cur, &p)?
            {
                continue;
            }
            // this version replaces the stored one, over the revision read
            p.rev = cur.as_ref().map_or(0, |c| c.rev);
            let at = self.now();
            self.store
                .update(move |tx| -> Result<()> {
                    let put = tx.put_playbook(p)?;
                    record_change(
                        tx,
                        &the_seed(),
                        at,
                        ChangeKind::Playbook,
                        &put.id,
                        cur.as_ref(),
                        Some(&put),
                    )
                })
                .await?;
        }
        Ok(())
    }

    /// Removes the built-in playbooks that no version ships any more, unless the owner edited one: that is his.
    async fn retire_playbooks(&self) -> Result<()> {
        let all = self.store.read(|r| r.playbooks()).await?;
        for p in all {
            if !p.builtin || p.edited || default_playbook(&p.id).is_some() {
                continue;
            }
            let (id, at) = (p.id.clone(), self.now());
            self.store
                .update(move |tx| -> Result<()> {
                    tx.delete_playbook(&p.id)?;
                    record_change::<Playbook>(tx, &the_seed(), at, ChangeKind::Playbook, &p.id, Some(&p), None)
                })
                .await?;
            self.notify.playbook_deleted(&id);
        }
        Ok(())
    }

    /// Writes the owner's version of a playbook over what over names, decided in the transaction that writes it. Chains
    /// find their playbook by id, so renaming one is safe.
    pub async fn save_playbook(&self, by: &Actor, p: Playbook, over: Over) -> Result<Playbook> {
        let mut p = prepared_playbook(p)?;
        let (w, at) = (written_by(by), self.now());
        let p = self
            .store
            .update(move |tx| {
                let stored = optional(tx.playbook(&p.id))?.map(|c| c.rev);
                p.rev = over.rev(stored, &named_playbook(&p.id))?;
                put_playbook(tx, &w, at, p)
            })
            .await?;
        self.notify.playbook_updated(&p);
        Ok(p)
    }

    /// Saves a playbook that does not exist yet; an id already in use is refused, never overwritten. A playbook is
    /// reference and moves no rail, so it is written as asked (ADR-0113).
    pub(crate) async fn create_playbook(&self, by: &Actor, p: Playbook) -> Result<Playbook> {
        self.save_playbook(by, p, Over::New).await
    }

    /// Replaces a playbook that exists with p, made from a read of it at p.rev: a playbook written since is not written
    /// over.
    pub(crate) async fn update_playbook(&self, by: &Actor, p: Playbook) -> Result<Playbook> {
        let over = Over::Read(p.rev);
        self.save_playbook(by, p, over).await
    }

    /// Removes a playbook of the owner's. A built-in one would come back at the next start, so it is reset instead.
    pub(crate) async fn delete_playbook(&self, by: &Actor, id: &str) -> Result<()> {
        if default_playbook(id).is_some() {
            return Err(Error::refused(
                "a built-in playbook cannot be deleted; reset it to its original instead",
            ));
        }
        let (w, at, key) = (written_by(by), self.now(), id.to_owned());
        self.store
            .update(move |tx| -> Result<()> {
                let cur = tx.playbook(&key)?;
                tx.delete_playbook(&key)?;
                record_change::<Playbook>(tx, &w, at, ChangeKind::Playbook, &key, Some(&cur), None)
            })
            .await?;
        self.notify.playbook_deleted(id);
        Ok(())
    }

    /// Puts a built-in playbook back to the version that ships with Remit.
    pub(crate) async fn reset_playbook(&self, by: &Actor, id: &str) -> Result<Playbook> {
        let Some(p) = default_playbook(id) else {
            return Err(Error::refused(format!("{id}: {}", store::Error::NotFound)));
        };
        let (w, at) = (written_by(by), self.now());
        let p = self.store.update(move |tx| put_playbook(tx, &w, at, p)).await?;
        self.notify.playbook_updated(&p);
        Ok(p)
    }
}
