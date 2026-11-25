# Reference: DeepSeek's Super-agent architecture, compared with Remit

Source: the DeepSeek Harness team's article on the **Super-agent** (Chinese original, read in machine translation; shared by
the owner on 2026-11-02). Its prototype: `github.com/deepseek-ai/dsh-superagent-prototype`. Compared against `docs/design.md`
and ADR-0001, 0013, 0014, 0016, 0018, 0021, 0022, 0024, 0027, 0031, 0032, 0033.

Coverage of our side: `docs/design.md` (Intent, Constraints, Model, How a chain runs, How results are checked, Event signals,
Channels, The Assistant, The provider, Running it), the `internal/orch` package map and the head of `watchdog.go`. Statements
about our behaviour come from those documents, not from reading code or tests (MEDIUM-HIGH). ADR-0031 to 0033 are proposed.

## The paper in one paragraph

A Super-agent is one continuous agent that stays online, works many tasks at once and reacts to events. Its five modules:
(2) a two-level architecture, a long-lived **Router** that only talks to humans and dispatches, and isolated **Workers** that
execute; (3) **event-driven Triggers** filtered by code first and an LLM second, routed by **Thread subscription**, with
context inlined or spilled and recalled hop by hop (**Chain of Recall**) and background **speculative execution**; (4)
**full-duplex** async tool calls and per-channel output norms; (5) **separate compute, state and environments**, with a
stateless cloud loop and a daemon per endpoint (BYOM); (6) an external **Watchdog** session that judges progress.

## Module by module

| Paper | Remit | Verdict |
|---|---|---|
| §2 Router and Workers | No single router. The Assistant is a long-lived agent in no chain but receives no events and dispatches nothing. A Dispatcher handles each event burst, a Chair each chain. | Different shape: scales by chain with no single point, but no agent holds cross-chain context. |
| §3.1 Two-stage filtering | CEL preconditions settle events in the door; the rest goes to a Dispatcher (ADR-0024). | Same idea; ours is sandboxed and checked at write time. |
| §3.2 Thread subscription, pass-through | A chain's tell subscription routes an event to its Chair by code, matched on MR address or branch. A burst waits for 20 s of quiet. | Partial: no Thread entity for IM or email, no many-to-many. |
| §3.3 Inline & Spill, Chain of Recall | The Dispatcher's first message inlines the chains an event matched; the Chair has `list_agents`, `read_output`. | Not formalized as a rule. |
| §3.4 Speculative execution | None. `propose_chain` is capped at 3 per run; ADR-0001 holds unrequested work while 5 MRs wait. | Missing, and in tension with review capacity. |
| §4.1 Full-duplex async loop | Messages steer into a running turn (priority `now`); background tasks and Monitor news start turns; a waiting chain frees its slot. | Equivalent, provided by Claude Code. |
| §4.2 Multi-channel | Channel adapters for web, door and CLI. The forge is read, never commented on. | Deliberate divergence: the owner reviews in our Changes tab, not in GitHub or IM. |
| §5 Separate compute, state, environment | One binary, SQLite, local Claude Code processes; crash-safe (`step`/`apply`, `Recover`, session resume, `VACUUM INTO`). | Opposite choice: one host, single user. |
| §6 Watchdog | Deterministic: idle alert, `unstick`, `cutSilentTurns`, `/healthz`, provider breaker. | Partial: drift is judged only by the goal judge at the end and reviewer/QC per change. |
| §8.4 Security (their open problem) | Sealed credentials, write-only inbox field, grants by effect (ADR-0018, 0027), jails (ADR-0021), owner key (ADR-0022). | We are ahead. |

## Where Remit is ahead

1. Rails in code: merge, pause and stop, sign-in are never agent behaviour.
2. Verification: reviewer and QC per patch id, goal judge, ADR-0013 watches, the customer section (ADR-0016). The paper's
   experiments measure scheduling only.
3. Attention as admission control (ADR-0001). Both designs name human attention the bottleneck; only we enforce it.
4. Exactly-once and crash safety: request ids, one-transaction transitions, queued prompts that survive a crash.

## Weaknesses in the paper

- The single-session baseline never delegated under the neutral prompt (0 of 5 runs): it tests prompt-following, not architecture.
- Pass-through did not speed tasks up; it cut Router tokens by about 37% (§7.1).
- Synthetic load (about 300 messages, 20 tasks, a tool that only consumes time) and mock Workers in the Router test. The claim
  that a lighter model suffices for the Router is untested.
- One Router session is a single point of failure, and the authors name long-term memory as the bottleneck after months (§8.1).

## Ideas worth taking, ranked

1. **A Thread entity for non-forge streams** (HIGH value). An IM topic or email chain is a stream a chain subscribes to,
   many-to-many. Extends ADR-0014 and ADR-0024, replaces marks-only matching, and is the prerequisite for noticing two chains
   working the same area mid-run (today only deduplicated at chain start).
2. **Speculative work that holds no review slot until it has a verified result** (MEDIUM). Needs an ADR first: a draft MR
   spends the owner's attention (ADR-0001).
3. **Memory for the Dispatcher** (MEDIUM). Each burst starts a fresh conversation. ADR-0033 is where it lands.
4. **A semantic watchdog** (LOW-MEDIUM). An agent that checks running chains for drift from their goal; never a rail.
5. **Measure our own scheduling** (MEDIUM). The paper's §7.2 method (arrival rate against time per decision) applies to
   Dispatchers; we have outcomes (ADR-0002) but no event-to-placement latency figure.

Not taken: stateless cloud compute and BYOM (§5). They would give up the jail and the credential model, and multi-host is a
non-goal (ADR-0009 names a holder on another machine as a decision to take deliberately). Revisit if chains must run on a
second machine.
