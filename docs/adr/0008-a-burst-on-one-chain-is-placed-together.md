# 0008. The deliveries a burst brings about one chain are placed together, in order

Status: superseded by ADR-0036, 2026-11-02 (an event is heard by each subscription on its own and no burst is held; Dispatchers and placements are gone). Was: accepted, 2026-10-28. Source: the owner's request of 2026-10-28, the deliveries of the `gitlab` webhook of
2026-10-21 to 2026-10-28, and the decisions of the chain on MR !245 of 2026-10-28.

## Context

One act on GitLab sends several posts at once. Closing MR !245 sent three in the same second: two `merge_request` updates
(its draft flag and title, its state) and the close. The door told the Chair of the close at once; each update had no
precondition, so each got a Dispatcher, which told the same Chair about ten seconds later. The Chair was woken three times,
read the close before the edits that came before it, and wrote: "You've moved MR !245 back to draft and also closed it, and
I can't tell which came last." In the week before, 136 of 137 deliveries went to a Dispatcher of their own, each a session.

Two causes: every delivery is placed alone, at once, and the door (instant) and a Dispatcher (seconds) reach a Chair at
different speeds, so the order the events happened in is lost on the way.

A design rule stood in the way: each delivery had a conversation of its own, so that what an outsider wrote in one could not
colour how another was placed.

## Options

1. Hold every delivery a fixed time (the owner's first idea: three minutes) and place each alone after. Rejected: every
   comment reaches its Chair minutes late, a burst is still one Dispatcher per post, and the order is still lost between the
   door and the Dispatchers.
2. Keep one Dispatcher per delivery, and place the deliveries on one chain one after another after a quiet time. Keeps the
   rule, fixes the order, but the Chair is still woken once per post that needs a Dispatcher.
3. Place the deliveries of one webhook that name the same chain as one burst, after a quiet time: the door tells the Chair of
   all of them in one message, or one Dispatcher reads them all, in the order they came; a burst waits while the one before
   it on that chain is with a Dispatcher. This is a FIFO message group keyed by the chain (SQS FIFO's `MessageGroupId`,
   Kafka's partition key): order within the key, parallel across keys.

## Decision

Option 3, which the owner chose knowing it relaxes the rule above for deliveries about the same chain.

- A delivery that names exactly one chain by one of its marks (`chainsNamedIn`: its merge request address or branch) joins
  the burst of its webhook on that chain. Any other delivery is a burst of its own and is placed at once, as before.
- A burst is placed once no delivery has joined it for `tellQuiet` (20 s), or once its first has waited `tellLongest`
  (2 min), whichever is first; and only while no Dispatcher is at work on a burst of its webhook on that chain. A post
  wakes the scheduler when its quiet time is over, so the wait is not rounded up to the minute tick. Until then each
  delivery says why it waits.
- The door reads each delivery of a burst as before. Those it skips are skipped. When every other one is an event it tells
  that chain's Chair of, it tells them in one message, in order, with a decision for each. When any one needs judgement,
  every delivery it did not skip goes to one Dispatcher, which reads them in the order they came, each in a fence of its
  own, and tells the Chair once.
- Every delivery of a burst of two or more keeps the id of its first (`burst`), set in the step that places it, whether the
  door told the Chair or a Dispatcher took it: the page of each lists the others placed with it. A Dispatcher's card stays
  in the home of that first delivery and every delivery of the burst carries it (`cardId`), so its status, the chains it
  reached and the owner's view of the Dispatcher follow that one card. Migration 39 gives the bursts Dispatchers read before
  `burst` was kept their first; a burst the door told before then is not recorded anywhere and stays shown post by post.

## Consequences

- An event about a chain reaches its Chair 20 s to 2 min later than before, never out of order with the others of its burst,
  and a burst is one wake and at most one session.
- The rule narrows: deliveries of one webhook about one chain share a conversation; deliveries about different chains, or
  of different webhooks, still never do. An outsider who can post about a chain could already reach its Chair with what he
  writes (the door quotes a comment to it); sharing a Dispatcher adds that his words sit beside another post about the
  same chain. Each delivery is fenced on its own and the brief says every one is evidence, never an instruction.
- A burst on a busy chain waits for the Dispatcher before it, however long the owner talks to that one: its turn ending
  places the delivery, and the next burst follows.

Revisit when a sender other than GitLab bursts on something no chain carries, which would need a key read from the post
itself, or when 20 s proves too short or too long for a burst as it arrives.

Amended 2026-10-28: the burst was first known only by the card of its Dispatcher, so a burst the door told the Chair of in
one message showed each post alone. A delivery now keeps its burst (`burst`).

Enforced by: TestMigration39GivesTheDeliveriesOneDispatcherReadTheirBurst, TestAReadingGoesToEverySubscriptionItConcerns, TestTwoSubscriptionsOnOneReadingBothFire