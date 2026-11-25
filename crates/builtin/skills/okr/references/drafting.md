# Drafting measures that are worth reading

Work in this order. Each step ends in something you can show.

1. **The customer and what they get.** Who is it for, and what is different in their day when this works? Write it as a person's behaviour or result ("opens the page and solves today's puzzle"), never as our output ("a post was made"). When the objective is about one of the owner's products, set `project` to its id (`list_projects`; your brief names your chain's): every chain of the objective starts from that project, and its customer is where yours starts.
2. **The outcome key result: one number per outcome.** Prefer what the customer did over what we did. Prefer distinct people or sessions over raw events (one player solving ten times is one player). Prefer a ratio of good to total (`solved / started`) where traffic varies, and a count where the volume is the point. Three to five key results are enough; each must be able to fail on its own.
3. **Find the signal.** List the signals your chain can read and read their real readings. The fields you see are the fields you may use. A field you did not see is a guess, and the dry run will refuse it.
4. **No signal: stop and build it.** A measure with no signal is not drafted. The first key result is the work of building the signal, as a `command` (its tests pass); a `work` chain whose goal names the playbook `okr-instrument` does it. A Chair makes the signal itself: `create_event_signal` opens it at once, and `create_standing_sensor` asks for the sensor that reads it, which waits for the owner's Apply in his inbox (both with the signals capability, which a Chair gives its chain with `grant_capability`). Never ask the owner to make a signal or a sensor, and never send him the steps. Once a real reading exists, draft the outcome measure on it.
5. **Set the threshold from what was measured.** Read the signal first: the baseline is the newest readings, not a hope. A target a little beyond the baseline is a target; a number from nowhere is decoration. Say the baseline in the key result's text, set the `window` to what the signal aggregates, and the `expiry` to when the answer is due.
6. **Guardrails: what must not get worse.** One per thing the owner would be angry to lose (speed, errors, cost, trust). Each is a signal and a condition that holds now; the dry run refuses one that already fails. A guardrail that cannot be read is a working rule: put it in the chains' briefs and out of the objective.
7. **Who can inflate it?** Ask of every measure what raises it without helping the customer: the team's own plays, an agent's own writes, a retry, a bot. Exclude or tell them apart at the source, or pair the measure with a guardrail that would catch the gaming.
8. **Unknowns.** What you do not know goes in the objective's unknown list with the question that would settle it. Do not invent a number to avoid an unknown.

## Outcome or activity

| Activity (weak) | Outcome (what the customer did) |
|---|---|
| posts published | distinct people who opened the game from a post |
| pull requests merged | the change reaching customers, and the number they use it |
| tickets closed | tickets reopened within a week, and time to a first answer |
| pages written | clicks and impressions of those pages |

Activity belongs in the plan of the chains. A key result that an agent can satisfy by doing more work, with no customer involved, measures the agent.

## Before you draft

For every measure, state: the signal, the field, the reading you saw (value and time), what the condition gives on it, and who could inflate it. If you cannot state all five, it is not ready.
