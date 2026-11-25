#!/bin/sh
# Runs the checks the named files can break, and no others: `make test-changed FILES="<file>..."`. The caller names the
# files it edited rather than this script asking git, as a working copy shared with other sessions holds changes that
# are theirs. `--plan` prints what would run and runs nothing.
# `--lint` runs only clippy, over the same crates: scripts/hooks/pre-commit runs it on the files a commit stages.
#
# A file reaches:
#   rust    the tests of the crate that holds it; clippy over that crate and every workspace crate that depends on it, in code or
#           in tests, which compiles their tests without running them (`make test` in the release workflow runs them).
#           Cargo.toml, Cargo.lock, rust-toolchain.toml, clippy.toml and deny.toml reach every crate.
#   gen     `make check-gen`, for the crates tsgen reads the contracts from and the TypeScript it writes, and for the app's
#           tokens and the design-system copy written from them
#   web     the typecheck (it spans the project) and the vitest files related to a source file; a config file reaches all
# A file none of these cover is named in the output, so a gap shows. `make test` runs everything.
set -eu

plan_only=0 lint_only=0
while :; do
	case ${1:-} in
	--plan) plan_only=1 ;;
	--lint) lint_only=1 ;;
	*) break ;;
	esac
	shift
done
if [ $# -eq 0 ]; then
	echo "usage: test_changed.sh [--plan] [--lint] <file>..." >&2
	exit 2
fi

here=$(pwd)
root=$(cargo locate-project --workspace --message-format plain)
root=$(dirname "$root")
cd "$root"
physroot=$(pwd -P)

relative() { # relative <path>: the path from the workspace root, whether it was given absolute or not
	p=$1
	for prefix in "$here/" "$root/" "$physroot/"; do
		case $p in "$prefix"*) p=${p#"$prefix"} ;; esac
	done
	printf '%s\n' "$p"
}

crate_of() { # crate_of <relative file>: the package name of the crate under crates/ that holds it, or nothing
	case $1 in crates/*/*) ;; *) return ;; esac
	dir=${1#crates/}
	dir=crates/${dir%%/*}
	[ -f "$dir/Cargo.toml" ] || return
	sed -n 's/^name *= *"\(.*\)"/\1/p' "$dir/Cargo.toml" | head -n 1
}

gen=0 all_rust=0 web=0 web_all=0
changed='' related='' skips=''
for arg in "$@"; do
	f=$(relative "$arg")
	case $f in
	crates/wire/src/* | crates/link/src/* | crates/replica/src/* | crates/tsgen/* | web/src/core/generated/* | \
		web/src/style.css | docs/design-system/tokens.css | scripts/design_tokens.sh)
		gen=1
		;;
	esac
	case $f in
	web/src/core/generated/*) ;;
	web/src/*.ts | web/src/*.vue)
		web=1
		related="$related $f"
		;;
	web/src/* | web/verify/*) web=1 ;;
	web/package.json | web/package-lock.json | web/vite.config.ts | web/vueOptions.ts | web/tsconfig.json | web/index.html | web/public/*)
		web=1 web_all=1
		;;
	scripts/* | Makefile | .github/* | docs/* | *.md) ;;
	Cargo.toml | Cargo.lock | rust-toolchain.toml | clippy.toml | deny.toml) all_rust=1 ;;
	*)
		c=$(crate_of "$f")
		if [ -n "$c" ]; then
			changed="$changed $c"
		else
			skips="${skips}skip $f: no test maps to it
"
		fi
		;;
	esac
done

as_line() { # the names on stdin, sorted and unique, on one line
	sort -u | tr '\n' ' ' | sed 's/ $//'
}

tested() { # the changed crates
	for c in $changed; do echo "$c"; done | as_line
}

linted() { # the changed crates and every workspace crate that depends on one of them, in code, in tests or to build
	for c in $changed; do
		cargo tree --offline -i "$c" -e normal,dev,build --prefix none --format '{p}' | awk '{ print $1 }'
	done | as_line
}

plan=''
add() { plan="$plan$*
"; }
[ "$gen" = 1 ] && add gen check
if [ "$all_rust" = 1 ]; then
	add test all
	add clippy all
else
	if [ -n "$changed" ]; then
		add test "$(tested)"
		add clippy "$(linted)"
	fi
fi
if [ "$web" = 1 ]; then
	add web typecheck
	if [ "$web_all" = 1 ]; then
		add web all
	elif [ -n "$related" ]; then
		add web related"$related"
	fi
fi
plan="$plan$skips"
if [ "$lint_only" = 1 ]; then
	plan=$(printf '%s' "$plan" | grep '^clippy ' || true)
	[ -n "$plan" ] && plan="$plan
"
fi

if [ "$plan_only" = 1 ]; then
	printf '%s' "$plan"
	exit 0
fi

failed=0
step() { # step <description> <command...>: a failure is counted, not fatal, so every part reports
	echo "==> $1"
	shift
	"$@" || failed=1
}
packages() { # packages <name>...: the names as cargo's -p arguments
	for c in "$@"; do printf -- '-p %s ' "$c"; done
}
# shellcheck disable=SC2329 # invoked through step
in_web() { (cd web && "$@"); }
while IFS= read -r line; do
	case $line in
	"gen check") step "make check-gen" make check-gen ;;
	"test all") step "cargo nextest run --workspace" cargo nextest run --workspace --all-targets --locked ;;
	"clippy all") step "cargo clippy --workspace" cargo clippy --workspace --all-targets --locked -- -D warnings ;;
	"test "*)
		# shellcheck disable=SC2086 # the crates are separate words
		args=$(packages ${line#test })
		# shellcheck disable=SC2086 # the arguments are separate words
		step "cargo nextest run $args" cargo nextest run --locked --no-tests=pass $args --all-targets
		;;
	"clippy "*)
		# shellcheck disable=SC2086 # the crates are separate words
		args=$(packages ${line#clippy })
		# shellcheck disable=SC2086 # the arguments are separate words
		step "cargo clippy $args" cargo clippy --locked $args --all-targets -- -D warnings
		;;
	"web typecheck") step "web: npm run typecheck" in_web npm run typecheck ;;
	"web all") step "web: npm test" in_web npm test ;;
	"web related "*)
		files=$(printf '%s\n' "${line#web related }" | sed -E 's#(^| )web/#\1#g')
		# shellcheck disable=SC2086 # the files are separate words
		step "web: vitest related $files" in_web npm exec -- vitest related --run --passWithNoTests $files
		;;
	"skip "*) echo "$line" ;;
	esac
done <<EOF
$plan
EOF
exit "$failed"
