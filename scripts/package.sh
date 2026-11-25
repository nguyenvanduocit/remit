#!/bin/sh
# Packages Remit to rebuild on another machine (`make package`). It writes, into $OUT (default bin/package):
#
#   remit-<sha>[-dirty].tar.gz        the source tree as it is now: tracked files plus untracked ones git does not
#                                          ignore, so uncommitted work travels too (what a supervisor runs is often exactly
#                                          that), with the docs, the deploy templates and a PACKAGE manifest naming the
#                                          commit it is based on and whether the tree differed from it
#   remit-<sha>.bundle                the git history (HEAD, every branch and tag): the new machine keeps the log that the docs
#                                          point to, and builds a binary that reports its commit on /healthz
#   SHA256SUMS                             checksums of what was written
#   remit-state-<time>.tar.gz         only with --with-state: the owner's data, which is secret. A consistent copy of
#                                          the database (VACUUM INTO, safe while the server runs) and the key its MCP
#                                          credentials and event signals' door keys are sealed with. Mode 0600;
#                                          docs/deploy.md says how to restore it. Without it the new machine starts empty.
#
# The source archive holds no database, key or environment file: it stops with an error if the list of files to pack
# names one. No binary is packed: the target builds its own (`make deps build`), so the binary is always of the source
# beside it. DATA (default ~/.remit) is where --with-state reads from.
set -eu
cd "$(dirname "$0")/.."

out=${OUT:-bin/package}
data=${DATA:-$HOME/.remit}
with_state=0
case "${1:-}" in
"") ;;
--with-state) with_state=1 ;;
*)
	echo "usage: scripts/package.sh [--with-state]" >&2
	exit 2
	;;
esac

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
trap 'exit 1' HUP INT TERM
mkdir -p "$out"

sha=$(git rev-parse HEAD)
changed=$(git status --porcelain | wc -l | tr -d ' ')
suffix=
[ "$changed" != 0 ] && suffix=-dirty
name=remit-$(git rev-parse --short HEAD)$suffix

# Files git lists that are no longer on disk (deleted, not yet committed) are left out, and so are a machine's local
# settings and any node_modules (a cache here, not the source). Sorted, so equal trees give equal archives.
# shellcheck disable=SC2016 # the script is for the inner sh, which expands it
git ls-files -z -co --exclude-standard |
	xargs -0 sh -c 'for f; do if [ -e "$f" ]; then printf "%s\0" "$f"; fi; done' sh |
	grep -zvE '^\.claude/settings\.local\.json$|(^|/)node_modules/' |
	LC_ALL=C sort -z >"$tmp/files"

if tr '\0' '\n' <"$tmp/files" | grep -E '(^|/)(secret\.key|\.env[^/]*|[^/]*\.(db|pem)(-wal|-shm)?)$' >&2; then
	echo "refusing to pack the files above: the source archive holds no database, key or environment file" >&2
	exit 1
fi

mkdir "$tmp/manifest"
cat >"$tmp/manifest/PACKAGE" <<EOF
Remit source package
commit:      $sha
uncommitted: $changed paths differ from that commit (0: the tree is the commit)
packaged:    $(date -u +%Y-%m-%dT%H:%M:%SZ)
rust:        $(sed -n 's/^channel *= *"\(.*\)"/\1/p' rust-toolchain.toml | head -n 1) (rust-toolchain.toml)
build:       see docs/deploy.md
EOF

tar --null -T "$tmp/files" --owner=0 --group=0 --numeric-owner --transform "s,^,$name/," -cf "$tmp/src.tar"
tar -C "$tmp/manifest" --owner=0 --group=0 --numeric-owner --transform "s,^,$name/," -rf "$tmp/src.tar" PACKAGE
gzip -9n <"$tmp/src.tar" >"$out/$name.tar.gz"
bundle=remit-$(git rev-parse --short HEAD).bundle
# HEAD too: a clone takes its checkout from the bundle's HEAD, and a tree checked out at a tag has no branch to name it.
git bundle create "$out/$bundle" HEAD --branches --tags >/dev/null 2>&1
written="$name.tar.gz $bundle"

if [ "$with_state" = 1 ]; then
	for f in "$data/remit.db" "$data/secret.key"; do
		[ -f "$f" ] || {
			echo "--with-state needs $f (DATA=$data)" >&2
			exit 1
		}
	done
	state=remit-state-$(date -u +%Y%m%dT%H%M%SZ)
	umask 077
	mkdir -p "$tmp/state/$state/backups"
	sqlite3 "$data/remit.db" "VACUUM INTO '$tmp/state/$state/remit.db'"
	cp "$data/secret.key" "$tmp/state/$state/secret.key"
	if [ -d "$data/backups/secret" ]; then cp -R "$data/backups/secret" "$tmp/state/$state/backups/secret"; fi
	tar -C "$tmp/state" -czf "$out/$state.tar.gz" "$state"
	chmod 600 "$out/$state.tar.gz"
	written="$written $state.tar.gz"
fi

# shellcheck disable=SC2086 # $written is file names without spaces
(cd "$out" && sha256sum $written >SHA256SUMS)
echo "packaged into $out: $written ($changed uncommitted paths included)"
