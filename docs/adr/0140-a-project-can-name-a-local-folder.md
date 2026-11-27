# 0140. A project can name a local folder

Status: accepted. Extends ADR-0108 with the folder names of ADR-0137.

## Context

A chain can open a folder in place. A project only remembered forge remotes, so the owner had to repeat the folder path on each chain.

## Options

1. Put the folder path in the owner's project notes. The repository list would omit a location the Chair uses for work.
2. Store an absolute folder path in the existing repository list. `open_repo` already understands both kinds of name.

## Decision

Option 2. The Assistant's project tools accept absolute paths and expand `~/` on the Remit server. The Chair passes the full path to `open_repo`, which checks the directory and its eligibility when opening it. Forge remotes retain their existing representation. The existing list can hold either name without rewriting stored records.

## Consequences

The Projects page shows the saved path and every Chair receives it in the project brief. The directory is on the Remit server.

## Revisit when

If installations need to restrict which directories an agent may open, enforce that policy at the existing folder eligibility check in `open_folder`.
