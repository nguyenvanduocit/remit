#!/bin/bash
# Formats a Rust file right after an agent writes it (PostToolUse on Edit/Write/MultiEdit), so `make check-rust`'s
# `cargo fmt --check` never fails on agent-written code. Only the file written is formatted: other sessions' files in the
# working copy are left as they are. rustfmt reads rustfmt.toml from the file's directory upwards; the edition is the
# workspace's (Cargo.toml [workspace.package]). A file that does not parse is left for the compiler to report.
set -u

file=$(jq -r '.tool_input.file_path // ""')
case "$file" in
*.rs) [ -f "$file" ] && rustfmt --edition 2024 "$file" 2>/dev/null ;;
esac
exit 0
