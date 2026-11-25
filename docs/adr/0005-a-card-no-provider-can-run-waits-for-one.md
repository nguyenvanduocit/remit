# 0005. A card no provider can run waits for one

Status: accepted, 2026-10-28; partly superseded by ADR-0006 (moves between providers). Source: the provider failovers of 2026-10-26 and 2026-10-27 (events with status `moved`
and `error` in the live database, `~/.pm2/logs/remit-error.log`), and `docs/design.md` (When a provider fails).

## Context

Remit runs agents on two subscription accounts, Claude and Codex. Each has its own limits (a session limit over hours,
a weekly limit, a rate of tokens per minute). In the week before this record, Claude reached a limit at least five times,
and the owner signed in to another account twice to get more.

A card whose provider failed moved to the other provider (failover). With no provider left, the failure went to the owner's
inbox with a Retry button. On 2026-10-26 Claude reached its session limit at 13:48. Six cards moved to Codex in twenty
seconds, and Codex answered 429 (`exceeded retry limit, last status: 429 Too Many Requests`) thirty-four seconds later.
Until Claude's limit reset at 14:40, 40 failures on 20 cards reached the inbox. In each of them there was no provider left
to move to. The owner could not answer any of them: a Retry before the reset failed again.

Two more facts came out of the same data:

- Of 69 moves, none went back to the model the card came from. Which model a card came from was kept in memory, and the
  server restarted about ten times a day.
- After a restart, cards whose provider was out still opened a session there and failed one turn before they moved (nine
  such moves between 02:57 and 04:01 on 2026-10-27).

## Options

1. Move a card off its provider as soon as its own turns fail, whatever the provider's breaker says. Rejected: none of the
   40 failures had a provider to move to, so this would have saved none of them.
2. Read Codex's 429 as a limit, which opens its breaker at once. Rejected: in the same hour Codex answered far more than it
   failed, so the breaker would have kept every new agent off it for fifteen minutes. The cards that failed would still
   have had nowhere to go.
3. Treat a provider out of capacity as a shortage, not a fault. A card no provider can run waits for one, keeping its
   message, and the scheduler opens it again once a provider can. Store where a moved card came from. Check the provider
   before a turn.
4. Shrink the number of slots while a provider is out, and pace the moves. Not now: Codex ran all of 2026-10-27 alone
   without a 429, so a lasting shortage is not shown. How Codex limits its rate is also not known here.

## Decision

Option 3, with one rule: **a card runs a turn only on a provider in use. When none can run it, it waits for one, and the
owner hears of it once, from the Health bar, not per card from his inbox.**

- After a provider failure, the card moves to the nearest model of another provider that can run it. With none, it waits.
  A model of its own provider is no way out, since that provider has just failed it. This holds for every kind of failure,
  a signed-out account too. Signing in is done on the provider, which the Health bar shows, not by retrying a card.
- A waiting card has status `waiting` and an activity naming each provider that is out and why. Its message goes back in
  its queue and its failure records leave the stream, as they do on a move.
- The scheduler looks at a waiting card again after its failure's cooldown (two minutes for a service, fifteen for a limit
  or an account), spread a quarter either way. It opens the card if a provider can run it, and otherwise looks again a
  cooldown later. The spread is the jitter that keeps waiting cards from all starting at once when a provider answers,
  which is what sent the six cards into Codex's 429.
- A card whose provider's breaker is open moves, or waits, before a session or a turn is spent finding out.
- The model a card was moved from is stored on it (`movedFrom`), so it goes back there after a restart.

## Consequences

- The owner's inbox carries no failure he cannot act on. A provider's shortage shows once on the Health bar and on each
  waiting agent's own badge (`Waiting for a provider`).
- No new entity: `Card` gains one status value and one field (`movedFrom`), within ADR-0003.
- Waiting is bounded only by the provider. A card can wait until a weekly limit resets, days later. Its chain is not
  nudged as stalled meanwhile: its Chair cannot make capacity.
- A card can start up to a cooldown and a quarter after its provider answers again: up to about 19 minutes after a
  limit. This is the price of the jitter.
- When to look at a waiting card again is kept in memory. A restart opens every waiting card with the others whose
  messages are queued, and each waits again while no provider can run it.

Enforced by: TestACardWhoseProviderFailsWaitsForIt, TestAWaitingCardOutlastsARestart, TestACardDoesNotTakeATurnOnAProviderThatIsOut

## Revisit when

- A 429 or a limit reaches the cards while another provider is in use and could run them (option 1 would then help).
- Cards fail in a burst right after a move or a wake, on the provider that received them (option 4).
- An agent waits so long that the owner asks for its work by another way (a Chair able to hand the work elsewhere).
