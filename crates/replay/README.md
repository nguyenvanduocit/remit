# replay

The pure core of a replay (ADR-0069): which recorded goals a change's candidate is run against (`select`), how one goal
fared under the goal judge (`outcome_of`, `outcome_of_replay`), which goal to queue next (`next`) and what a replay's goals
come to (`score`). It stores, starts and reads nothing; the orchestrator runs the chains and keeps the replay on its change
item.
