#!/bin/sh
# Builds the client core for wasm32 and binds it with wasm-bindgen-cli into web/src/core/pkg/:
#   pkg/web   the browser's module (WebTransport), loaded by the Vue shell
#   pkg/node  the same core with the fake server, loaded by vitest in Node
#   pkg/fake  the browser's module with the fake server, loaded by the verify pages (web/verify)
# wasm-bindgen-cli must be the version the crate pins (Cargo.toml: wasm-bindgen = "=X"); it prints the gzipped size of
# each module it binds.
set -eu

root=$(cd "$(dirname "$0")/../.." && pwd)
target=${CARGO_TARGET_DIR:-$root/target}
out=$root/web/src/core/pkg
# pkg/web ships in the release, so by default it is built small (the wasm profile, Cargo.toml); WASM_PROFILE=wasm-dev
# (make dev, make test) builds it in seconds. pkg/node and pkg/fake never ship: they are always built with wasm-dev.
profile=${WASM_PROFILE:-wasm}

want=$(sed -n 's/^wasm-bindgen = "=\(.*\)"/\1/p' "$root/crates/client-web/Cargo.toml")
have=$(wasm-bindgen --version | cut -d' ' -f2)
if [ "$want" != "$have" ]; then
  echo "bind.sh: wasm-bindgen-cli is $have, the crate pins $want: cargo install wasm-bindgen-cli --version $want --locked" >&2
  exit 1
fi

build() { # <profile> <features> <wasm-bindgen target> <out dir>
  cargo build --manifest-path "$root/crates/client-web/Cargo.toml" --profile "$1" --target wasm32-unknown-unknown $2
  rm -rf "$4"
  wasm-bindgen --target "$3" --out-dir "$4" "$target/wasm32-unknown-unknown/$1/client_web.wasm"
  printf 'bind.sh: %s: %s bytes, %s gzipped\n' "$4" "$(wc -c < "$4/client_web_bg.wasm" | tr -d ' ')" \
    "$(gzip -9 -c "$4/client_web_bg.wasm" | wc -c | tr -d ' ')"
}

build "$profile" "" web "$out/web"
build wasm-dev "--features fake" experimental-nodejs-module "$out/node"
build wasm-dev "--features fake" web "$out/fake"
