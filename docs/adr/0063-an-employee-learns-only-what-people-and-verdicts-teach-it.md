# 0063. An employee learns only what people and verdicts teach it, through the owner's memory system

Status: accepted, 2026-11-05; extended by ADR-0088 (Chairs, the Assistant and mentors learn); its backend superseded by ADR-0117; built by `docs/superpowers/plans/2026-11-05-agent-learning.md`. Source: the owner's research
request and interviews of 2026-11-05 (`docs/research-agent-learning/design.md` §1: "core records, plugin remembers, a coach
decides; duels and ratings stay in the core"). Supersedes ADR-0061's memory (Claude Code's auto memory, one directory per
employee); builds on ADR-0034 (a source the core lacks is a plugin), ADR-0058 (sessions run in the AI runtime) and ADR-0062
(the Assistant decides for the owner, he overturns).

## Context

ADR-0061 gave every employee Claude Code's auto memory in a directory of its own. Any session with Write saved at any moment,
whatever its card's verdict, and any session could read any employee's directory (`protectedFromAgents` did not list it).
The research (`docs/research-agent-learning/report.md`) finds that memory written without a gate makes agents worse
(SWE-Bench-CL; MINJA and AgentPoison poison it at under 0.1% of entries), that learning which works today is text a frozen
model reads, extracted from human teaching and gated by a verdict, and that taste is learned from people's edits (CIPHER). The
owner already runs a memory system that extracts, embeds, scopes and consolidates memories from Claude Code transcripts
(`~/projects/memory`).

## Options

1. Keep auto memory, copy each card's memory aside and merge it on accept. Files plus a provenance table are two owners of one
   memory, the gate sits after the agent has written, and it breaks once the runtime leaves the core's disk.
2. Memory as Remit tables and tools. Rebuilds a backend the owner already runs and trusts.
3. The core records every teaching and holds the gate; the owner's memory system is the backend, run by the core as a child;
   a coach employee holds the learning policy; duels with an AI judge stay in the core. **Chosen.**

## Decision

- **The core records every teaching as a fact.** `teach.verdict` (accept or rework, the judge, the reason), `teach.steer` (a
  mentor's message), `teach.edit` (the diff between the agent's last push and what was merged), `teach.overturn` (the decision,
  its maker, the owner's reason) and `teach.duel` (both cards, the judge's pick) name the employee taught and the card. Any later
  learner, model or backend re-reads them.
- **Memory is the owner's memory system, run by the core.** With `-memory-bin` and `-memory-plugin`, the core runs
  `memory daemon` with `MEMORY_HOME=<data>/memory` and a config it writes: the runtime's transcripts as the only scan root
  (`discover: false`), nothing extracted until admitted (`gated: true`), no spend cap, the extractor on the agents' Claude login
  (`extract_env`), embeddings with `-memory-key-file`. An employee is a project (visibility `private`), its template a stack,
  the owner's principles the global scope. Without the flags, employees run with no memory.
- **The gate.** Only the core admits: an accepted card's session as `accepted` (lessons, pitfalls, preferences and facts from
  the whole session), a sent-back card's or a mentor's teaching as `teaching` (only what the teacher said), a teaching outside a
  transcript (an edit, an overturn's reason) through `learn`. An overturn retracts what the overturned decision taught.
- **Memory is private by rail.** Every session runs with Claude Code's auto memory off and the memory plugin loaded, its
  project in `MEMORY_PROJECT`. Its hooks recall the employee's own project, its stack and global, never another employee's.
  Sessions run with `--strict-mcp-config`, so the plugin's save tools are absent: the employee searches with Remit's
  `search_memory` and never writes its own memory. The Assistant's sessions run on a project of their own, `assistant`, which
  the owner's overturns of its answers teach. The backend's database, logs and key file are hidden from every session.
- **The owner sees and steers memory through agents.** The Crew page, `read_memory` and the Chair's gist read the daemon's
  `list`; a forget the Assistant drafts applies through `forget`. A coach (the built-in playbook `coach`, capability `coaching`)
  shares a lesson with its lineage when two siblings' accepted cards back it, drafts a lesson for every employee or a forget
  for the Assistant to apply, asks Chairs for contests and reads the repeat-correction rate, all by the words of its brief.
- **Duels.** A Chair contests a card with a sibling for an employee of another template; an AI judge picks, its verdict a
  decision the owner may overturn. A Bradley–Terry rating per template per kind of work, shown with its count, guides hiring;
  duels of a kind of work pause when the judge's agreement with the owner's overturns drops below the coach's share.
- **Clean slate.** At start, `<data>/employees/*/memory` is archived to `<data>/backups/memory-<time>.tar.gz` and removed.

## Consequences

- An employee knows only what an accepted card or a person taught it; a rejected card's conclusions never reach its memory.
- Remit depends on a memory binary the owner builds; without it, employees work without memory rather than fail.
- Extraction spends the agents' Claude quota.
- The owner's own daemon and Remit's are separate homes; nothing crosses between them.

## Revisit when

- The runtime leaves the core's machine: the daemon follows the transcripts, or the runtime ships them to it.
- A teaching is a channel an outsider reaches (a review thread from a stranger, a poisoned transcript): the threat model runs
  on the gate, and admitted teachers are checked against the people the owner made.
- The plugin's hooks inject too much: a recall budget per session.
- The judge's calibration needs more than overturns: owner-labelled duels as an eval set.
- Claude Code changes how plugins, auto memory or `--strict-mcp-config` work: `agent.Spec` and the launcher move with it.

Enforced by: TestAnEmployeesSessionReachesItsMemoryThroughThePlugin, TestTheCoreRunsTheDaemonInItsOwnHome,
TestTheClientSpeaksTheDaemonsProtocol, TestTheMemoryBackendsFilesAreHiddenFromAgents, TestTheEmployeesFileMemoryIsArchivedThenRemoved,
TestAnAcceptedCardIsAdmittedForItsEmployee, TestASentBackCardsConclusionsAreNotLearned, TestAMentorsSteerIsATeaching,
TestAnOverturnRetractsWhatItsDecisionTaught, TestTheOwnersEditsAfterMergeBecomeATeaching, TestSearchMemoryReadsOnlyTheEmployeesOwnScopes,
TestTheClientTeachesAndSearchesInTheDaemonsProtocol, TestTheAssistantsSessionsRecallWhatOverturnsTaughtIt,
TestContestingACardOpensASiblingForAnotherLineage, TestTheJudgesPickUpdatesBothRatings, TestTheStaffListShowsTheLineageRating,
TestALineageRatingFollowsTheDuelsOfItsKindOfWork, TestDuelsPauseWhenTooFewOfTheJudgesPicksStand, TestAnOverturnedDuelReversesTheRating,
TestDuelsPauseWhenTheJudgeDisagreesWithTheOwner, TestTheCoachsPolicyIsWordsInItsBrief, TestCoachingReadsForEveryAgentAndWritesForTheChair,
TestRepeatCorrectionsAreCorrectionsThatTaughtALessonHeld, TestTheCoachSharesALessonWithItsLineageOnItsSiblingsMemories,
TestTheCoachDraftsALessonForEveryEmployeeForTheOwner, TestTheCoachDraftsAForgetForTheOwner, TestTheCoachAsksAChairToContestACard,
TestTheCoachSetsTheDuelsShareTheOwnerNames, TestOnlyACoachingChainWatchesTheTeachings, TestDuelStandingsAreAnsweredPerKindOfWork;
web: DuelsView.test.ts, CardDecisions.test.ts, decisionRows.test.ts, verify/duels.mjs
