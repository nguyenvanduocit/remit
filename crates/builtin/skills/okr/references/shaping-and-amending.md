# Shaping, amending, refining and closing an objective

## A new objective

A big or half-formed wish becomes an objective through a chain of kind `objective`: a Chair analyses the customer and the outcome, reads the signals that exist, and drafts it without interviewing him. The draft waits in his inbox as a change item, which you decide for him (Apply or Not now, `answer_inbox`); he corrects it by talking to the Chair, and overturns your answer when he disagrees. Start it with `create_chain`, kind `objective` (see the app skill, `references/chains.md`), the goal written from his wish as that reference says and naming the `objective` playbook its Chair reads, and tell him which chain runs.

The draft is checked as the Chair writes it, and again when it is applied. A measure with no signal is not drafted: the objective's first key result is then the work of building the signal, as a command.

## Changing one

An amendment is a new draft of the whole objective over the version he read, by the Chair of a chain that has the objectives capability; ask for it through `okr-review` (which audits and amends) or `objective` (which reshapes). A key result the amendment leaves out is kept as retired: its chains finish and no new chain opens for it.

An active objective whose old guardrails were written in words has them as conditions with no signal. It keeps running, and its next amendment cannot be applied until each guardrail names a signal that reads, or is moved into the chains' briefs.

## Refining a key result

A key result can be refined into key results of its own, each with its own check, owner and judge: one with no check is met when every key result refining it is. The ones the owner committed to stay the commitment; what refines them changes how they are reached, so it is applied at once and recorded as a change he reads, never put in his inbox. Whoever answers for a key result refines it: the employee's chains, or you for one the owner answers for, with `refine_key_result` (the parent, the text, the check, who answers and judges; `id` and `retire` to change or retire one). Each key result runs as many chains at once as its share (1 to begin, one more for each of its landings that holds), so refining is how work on it goes wide; a key result whose check is a command may instead be raced by competing attempts (`propose_chain` with `attempt`, one call each, up to its share), whose approved versions wait until a judge runs the check on each and picks the one that lands. Read the reply before you add another: it lists what already refines that parent and the chains serving each. What refines a key result retires with it, and once it is met.

## A stalled key result

A key result whose last 3 chains finished without progress (no work landed, no goal met) is stalled: it takes no new chain, and `read_objective`'s tree and the Objectives page say which chains those were. A chain the owner asks for is never held by it. Lift it with `lift_key_result_stall` only when something is different now (a new approach, a refinement, a blocker gone), saying what in `why`: he reads it. When the same work would only fail again, refine the key result or change how its chains are briefed instead.

## Closing one

`draft_objective_close` with the objective's id, on his word. It raises a change item, which you apply for him. Chains serving its key results are not stopped.

## Who answers

Every key result has an owner (who answers for it) and a judge. Both are the owner until a Chair or a judge is hired for it with the draft; a reading key result's standing is read from its signal, so its judge has nothing to rule on.
