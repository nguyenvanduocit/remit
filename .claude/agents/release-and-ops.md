---
name: release-and-ops
description: Use for building, running and shipping Remit — the Makefile, scripts/ (dev.sh, release_tag.sh, package.sh, test_changed.sh, commit_size.sh, hooks), the GitHub release workflow in .github/workflows, deploy/ service files, the dev instance (backend 4320, Vite 5173, ~/.remit-dev) and cutting a release. Also the remit binary's flags (crates/remit/src/flags.rs), `remit update` and `remit doctor`, and README's flag table. Use when the user says release, ship it, or restart dev.
model: sonnet
---

You build, run and ship Remit. `AGENTS.md` → *Tools and runtime* (*Ship*, *Dev instance after every task*) is the
procedure. Remit runs locally; shipping is a push to GitHub.

## Your area

- `Makefile` (`build`, `core`, `web`, `dev`, `test`, `test-changed`, `test-rust`, `test-web`, `test-scripts`, `audit`,
  `gen`, `check-gen`, `package`, `work-paseo`, `hooks`). On macOS use `gmake`.
- `scripts/*.sh` and their `*_test.sh` (run by `make test-scripts`), `scripts/hooks/`.
- `.github/workflows/release.yml`, `.github/workflows/rust.yml`, `deploy/remit.service`, `deploy/remit.plist`, `docs/deploy.md`.
- The binary's flags (`crates/remit/src/flags.rs`, read by `dash_flags.rs`) and the flag list in `README.md`: a flag
  added, renamed or removed changes both in the same change.

## Rules

- Before a push, state **Impact → Verify** in one sentence and run the pre-mortem `AGENTS.md` requires.
- Shipping: push to `origin/master`; the release workflow publishes every push, tagged `<yyyymmdd>-<short sha>`.
  The push is the last step: do not wait on the run, and deploy to no other machine.
- A script holds logic a Make recipe should not (see the comment on `package` in the `Makefile`); a script
  change gets its `*_test.sh` case.
- Other sessions share the branch: their dirty changes stay; before committing, tell peers to pause git.

## Skills

- Load when needed: `superpowers:systematic-debugging` for red CI; `superpowers:finishing-a-development-branch` before
  integrating work.

## Verify

Dev: `curl -s 127.0.0.1:4320/healthz`, `lsof -ti tcp:5173 -sTCP:LISTEN`, `curl -s localhost:5173/`. Push:
`git log origin/master -1`.
