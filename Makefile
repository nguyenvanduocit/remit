.PHONY: build core web test test-changed test-rust test-web audit run dev deps gen check-gen package hooks

deps:
	cd web && npm install --no-audit --no-fund

# The contracts are Rust (crates/wire, ADR-0072). tsgen writes their TypeScript for the web app: wire::app and the
# replica's shell, and what a tab says on its port to the core (client-web's port).
# $(1) is the directory the TypeScript goes under (the repository, or check-gen's scratch copy of it). Each run reads
# crates/model, whose records the contracts carry (ADR-0107).
tsgen = cargo run -q -p tsgen -- crates/wire/src app $(1)/web/src/core/generated crates/model/src && \
	cargo run -q -p tsgen -- crates/replica/src shell $(1)/web/src/core/generated/replica \
		crates/link/src crates/wire/src crates/model/src && \
	cargo run -q -p tsgen -- crates/client-web/src port $(1)/web/src/core/generated/client_web \
		crates/replica/src crates/link/src crates/wire/src crates/model/src
# README's flag table is written from the flags' definitions (crates/remit/src/flags.rs) by the test that checks it. The
# design-system package's tokens (docs/design-system/tokens.css) are written from the app's (web/src/style.css).
gen:
	$(call tsgen,.)
	scripts/design_tokens.sh docs/design-system/tokens.css
	REMIT_WRITE_README=1 cargo test -q -p remit --bin remit the_readme_flag_table

# Fails when the generated files differ from what `make gen` writes from the contract and the app's tokens: one of them
# was edited by hand, or the contract or a token changed without regenerating. It generates into a directory of its own
# and compares with the files in the working tree, so it changes nothing and holds on a tree where another session's
# contract change sits uncommitted (and regenerated) beside yours.
check-gen:
	@out=$$(mktemp -d) && trap 'rm -rf "$$out"' EXIT && \
	$(call tsgen,"$$out") && \
	diff -ru "$$out/web/src/core/generated" web/src/core/generated && \
	scripts/design_tokens.sh "$$out/tokens.css" && diff -u "$$out/tokens.css" docs/design-system/tokens.css

# The client core (crates/client-web) built for wasm32 and bound into web/src/core/pkg: the web app, its tests and its
# verify pages load it (crates/client-web/bind.sh).
core:
	crates/client-web/bind.sh

web: core
	cd web && npm run build

# The server and the client, release-built, with the web app embedded in the server (crates/remit/src/web.rs), copied
# into bin/. CARGO_TARGET_DIR, when set, is where cargo writes. Each binary is removed before the copy: macOS kills a
# signed binary rewritten in place (its cached signature no longer matches), so the copy must be a new file.
TARGET := $(or $(CARGO_TARGET_DIR),target)
build: web
	cargo build --release --locked -p remit -p remit-client
	mkdir -p bin
	rm -f bin/remit bin/remit-client
	cp $(TARGET)/release/remit $(TARGET)/release/remit-client bin/

# Two tiers. `test-changed` is what every change runs (below). `test` is the whole of it, under a minute: the contract
# check, the Rust workspace formatted, every workspace lint held and its tests, the web typecheck and tests; CI runs
# it. The suite holds only the tests that matter most (AGENTS.md, *Evidence and verification*): the compiler and the
# lints carry the rest. `--all-targets` leaves doctests out. A `harness = false` test (the jail's, whose
# main must own the process) runs as it is written. The client core is built with the wasm-dev profile here
# (WASM_PROFILE=wasm-dev, as make dev does): the tests need its behaviour, not its size; `make web` builds pkg/web small again. test's parts share
# nothing but the client core, so it builds that first and runs them side by side; -j is passed here, not set for the
# whole Makefile, as `make deps build test` must still run in order. --output-sync keeps each part's output in one piece.
test: check-gen
	WASM_PROFILE=wasm-dev crates/client-web/bind.sh
	$(MAKE) -j --output-sync=target test-rust test-web

# What a change can break and nothing else: `make test-changed FILES="crates/orch/src/step.rs web/src/act.ts"`, naming
# the files you edited. The mapping is in scripts/test_changed.sh. `test` stays the whole tier, for CI and for the owner.
test-changed:
	scripts/test_changed.sh $(FILES)

test-rust:
	cargo fmt --all --check
	cargo clippy --workspace --all-targets --locked -- -D warnings
	# nextest runs every test binary at once rather than one after another, so the slowest test sets the time.
	cargo nextest run --workspace --all-targets --locked

test-web:
	cd web && npm run typecheck && npm test

# The checks that read every dependency: their advisories, licences and sources (deny.toml), and no dependency left
# unused. They wait for the push (scripts/hooks/pre-push); CI runs them on every push and pull request.
audit:
	cargo deny check
	cargo machete

run: build
	./bin/remit

# The backend rebuilt and restarted on every run, the web app hot-reloaded by Vite; a new run replaces the last (scripts/dev.sh).
dev:
	scripts/dev.sh

# Packs the source tree as it is (uncommitted work included), its git history and, with `make package ARGS=--with-state`,
# the owner's data into bin/package, to rebuild on another machine (docs/deploy.md). The logic is a script because a recipe
# that calls $(MAKE) runs even under `make -n`; this line only prints.
package:
	scripts/package.sh $(ARGS)

# Runs scripts/hooks/pre-commit on every commit and scripts/hooks/pre-push on every push of this clone. A clone opts in
# once with `make hooks`; git does not version core.hooksPath.
hooks:
	git config core.hooksPath scripts/hooks
