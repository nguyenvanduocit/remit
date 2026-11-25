#!/bin/sh
# Warns, before a push, about each commit that changes more than BUDGET lines (added plus deleted, generated files and
# lock files left out): a change that large cannot be reviewed as one (docs/adr/0003-grow-from-the-working-core.md).
# A commit that is large on purpose says why in a line of its message starting with `Large-change:`, e.g.
# `Large-change: moves the routines out of crates/orch into a crate of their own, no behaviour changed`, and is not warned about.
# It never blocks the push: the budget is a prompt to split, not a rail.
#
# Reads what git gives a pre-push hook on stdin: one `<local ref> <local sha> <remote ref> <remote sha>` line per ref.
set -eu

BUDGET=800

while read -r _ local_sha _ remote_sha; do
	case $local_sha in *[!0]*) ;; *) continue ;; esac # a deleted ref pushes no commit
	case $remote_sha in
	*[!0]*) commits=$(git rev-list --no-merges "$remote_sha..$local_sha") ;;
	*) commits=$(git rev-list --no-merges "$local_sha" --not --remotes) ;; # a new branch: what no remote has yet
	esac
	for c in $commits; do
		lines=$(git show --numstat --format= "$c" -- . ':(exclude)*.generated.*' ':(exclude)*package-lock.json' ':(exclude)Cargo.lock' |
			awk '$1 != "-" { n += $1 + $2 } END { print n + 0 }')
		if [ "$lines" -le "$BUDGET" ] || git show -s --format=%B "$c" | grep -q '^Large-change:'; then
			continue
		fi
		echo "warning: $(git show -s --format='%h %s' "$c") changes $lines lines, past the budget of $BUDGET." >&2
		echo "  Split it, or say why it is one change in a 'Large-change: <why>' line of its message (scripts/commit_size.sh)." >&2
	done
done
exit 0
