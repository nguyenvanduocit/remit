#!/bin/sh
# Prints the tag the release workflow publishes a commit under (default HEAD): `<yyyymmdd>-<short sha>`, the day the
# commit was made, in UTC. `remit update` reads the commit back from it (crates/remit/src/update.rs, `commit_of`), so
# its form is a contract. The day is the commit's, not the run's, so a run made again names the release it made before.
set -eu
commit=${1:-HEAD}
day=$(TZ=UTC git show -s --format=%cd --date=format-local:%Y%m%d "$commit")
echo "$day-$(git rev-parse --short "$commit")"
