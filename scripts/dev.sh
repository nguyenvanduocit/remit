#!/bin/sh
# Runs the app for development (`make dev`): the backend (crates/remit) on 4321, beside a real instance on 4320, built fresh and not reloaded, and the Vite dev server
# on 5173, which hot-reloads the web app and proxies the backend (web/vite.config.ts). A run first stops whatever still
# listens on those two ports, so running it again replaces the last run. Ctrl-C stops both. Data lives in
# ~/.remit-dev, apart from a real instance's ~/.remit; DEV_DATA names another directory. DEV_FLAGS adds flags to
# the backend, split on spaces; a later flag wins over the same one before it. The backend starts its AI
# runtime (`remit runtime`, its child, ADR-0058), which runs every agent and ends when the backend does. A run waits on no
# one: with no Claude Code login in <data>/claude the default account is signed out, and agents run on the accounts the
# Accounts page adds (a login, or a subscription's token, ADR-0129). Employees remember through the memory system in DEV_MEMORY (default
# ~/projects/memory) when its plugin binary is built, embedding with DEV_MEMORY_KEY_FILE, which defaults to the key file the
# owner's own memory uses (~/.memory/config.json's api_key_file).
set -eu
cd "$(dirname "$0")/.."

data=${DEV_DATA:-$HOME/.remit-dev}
backend_port=4321
web_port=5173

stop_listeners() {
	pids=$(lsof -ti "tcp:$1" -sTCP:LISTEN || true)
	[ -z "$pids" ] && return
	echo "stopping the process on :$1 ($pids)"
	kill $pids
	# A backend may take its whole stop to end (crates/remit/src/serve.rs: DRAIN_FOR + SHUTDOWN_FOR + HTTP_FOR, under 15 s):
	# wait as long.
	for _ in $(seq 30); do
		lsof -ti "tcp:$1" -sTCP:LISTEN >/dev/null || return 0
		sleep 0.5
	done
	echo "port $1 is still in use" >&2
	exit 1
}

stop_listeners "$backend_port"
stop_listeners "$web_port"

# The web app loads the client core. It is bound again when a source the last wasm build read (cargo's dep-info lists
# them, the core's dependencies included) is newer than the bound module, so the page never runs a stale core. It is
# bound with the wasm-dev profile (Cargo.toml), in seconds; the shipped one's LTO is `make web`'s.
core_bound=web/src/core/pkg/web/client_web_bg.wasm
core_sources=${CARGO_TARGET_DIR:-target}/wasm32-unknown-unknown/wasm-dev/client_web.d
if [ ! -f "$core_bound" ] || [ ! -f "$core_sources" ] ||
	[ -n "$(cut -d: -f2- "$core_sources" | tr ' ' '\n' | grep . | while read -r f; do [ "$f" -nt "$core_bound" ] && echo "$f"; done)" ]; then
	WASM_PROFILE=wasm-dev crates/client-web/bind.sh
fi
# A debug build, which reads the web app from web/dist on disk rather than embedding it.
cargo build --locked -p remit
remit=${CARGO_TARGET_DIR:-target}/debug/remit

# Memory searches by meaning with the owner's OpenRouter key when there is one (ADR-0117); without, by words and links.
openrouter_key=${DEV_OPENROUTER_KEY_FILE:-$HOME/.remit/openrouter.key}

# shellcheck disable=SC2086 # DEV_FLAGS is a list of flags, split on purpose
"$remit" -addr "127.0.0.1:$backend_port" -data "$data" -openrouter-key-file "$openrouter_key" ${DEV_FLAGS:-} &
backend=$!
trap 'kill $backend 2>/dev/null || true' EXIT INT TERM

echo "web:     http://localhost:$web_port/  (sign in with #key=<the key in $data/owner.key>)"
echo "backend: http://127.0.0.1:$backend_port/"
cd web && REMIT_BACKEND="http://127.0.0.1:$backend_port" npm run dev -- --port "$web_port" --strictPort
