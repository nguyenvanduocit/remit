# ADR-0140 · A worker's session starts only above the work root's free-space floor

Status: Accepted

## Context

On 26/11 at 08:57 three chains' agents built Rust in their worktrees under `<data>/work` (`config.rs`: the work root is
fixed there) and filled the volume. The server's own writes then failed: the telemetry database logged "database or disk
is full" 71 times (remit.error issue b9f6cd0703fd715a), and every agent's Bash failed with it. Nothing in Remit looked at
free space before it started more work on that volume.

## Options

- A flag to move the work root to a larger volume. It moves the cliff, it does not remove it; any volume fills.
- Hold the chain at admission (`admit.rs`) under disk pressure. Admission gates a chain's entry to running, not the
  sessions a running chain's Chair hires, and the cards that fill the disk run inside chains already admitted.
- Refuse a session's start below a floor of free space, at the one place every session starts (`Core::launch`), for the
  seats that work in a worktree: a hire's and a judge's, new or resumed. The Chair and the Assistant build nothing, and
  they are who decides a refused card's retry and frees space: refusing them too would stop Remit at the floor with no
  one left to recover it.

## Decision

`Core::launch` (chains/src/session.rs) asks `Core::work_room` (chains/src/disk_floor.rs) before it chooses an account.
A card whose role is `Chair` or `Assistant` passes (`held_by_floor`, an exhaustive match on `model::CardRole::of`). For the rest
the host port reads the bytes free on the work root's volume (`Host::free_space`; `statvfs` available blocks in
`remit/src/host.rs`), off the async threads, and the pure `above_floor` compares it with `WORK_FLOOR`, 5 GiB: room for
two Rust builds at once (a build in a worktree writes about 2 GB; three took the last 6 GB in four minutes on 26/11) and
for the server's own writes. 10 GiB was first chosen and dropped: this host sits at 9 to 10 GiB free on most days, so it
would refuse every worker in its steady state. Below the floor the start fails as any start does (`fail_to_open`): the
card is failed with "start session: the volume of the work root has X free, below the floor of Y", in its stream, and
the retry goes to the inbox, which the Assistant decides. A volume that cannot be read refuses nothing and is logged at
warn once.

## Consequences

Remit stops adding workers to a volume that is filling; the server keeps 5 GiB for itself and the sessions at work. A
host with less than 5 GiB free on that volume starts no worker or judge until space is freed; its Chairs and the
Assistant go on. Sessions already open are not
stopped: a turn of one can still fill the volume.

## Revisit when

- A card refused for space should wait and reopen by itself, as `CardStatus::Waiting` does for a provider, instead of
  failing with a retry.
- The open sessions' turns should be bounded too (a check per turn, or a quota per worktree), if a single chain fills the
  last 5 GiB.
- The floor should scale with the volume or the measured size of a build, or be the owner's setting.
