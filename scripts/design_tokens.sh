#!/bin/sh
# Writes the design-system package's tokens (docs/design-system/tokens.css, Open Design's token contract) from the app's
# own: `scripts/design_tokens.sh <out.css>`. The app's `:root` in web/src/style.css is the one place a colour, a font, a
# size or a duration is written; the package takes it verbatim, then names it again by the contract's names, each a var()
# of the app's, so no value is ever written twice. `make gen` writes the file; `make check-gen` fails when it differs.
set -eu
out=${1:?usage: design_tokens.sh <out.css>}
root=$(cd "$(dirname "$0")/.." && pwd)
tokens=$(awk '/^:root *\{/ { on = 1 } on { print } on && /^\}/ { exit }' "$root/web/src/style.css")
[ -n "$tokens" ] || { echo "design_tokens.sh: no :root block in web/src/style.css" >&2; exit 1; }
{
	cat <<'HEAD'
/* Written by scripts/design_tokens.sh from web/src/style.css (`make gen`); do not edit: change the app's token and
   regenerate. Remit's design tokens for Open Design: the app's own names first, as the app writes them, then the
   contract's names, each pointing at the app's. Read DESIGN.md for what each is for, ADR-0090 for the colour roles. */

HEAD
	printf '%s\n' "$tokens"
	cat <<'CONTRACT'

/* Open Design's token contract, by the app's names. */
:root {
  --surface: var(--panel);
  --surface-warm: var(--panel);
  --fg: var(--text);
  --fg-2: var(--text2);
  --muted: var(--mut);
  --meta: var(--faint);
  --border: var(--line);
  --border-soft: var(--rule);
  --accent: var(--blue);
  --accent-on: var(--on-primary);
  --accent-hover: var(--blue-hi);
  --accent-active: var(--blue-mark);
  --success: var(--green);
  --warn: var(--amber);
  --danger: var(--red);
  --info: var(--cyan);
  --font-display: var(--sans);
  --font-body: var(--sans);
  --font-mono: var(--mono);
  --text-sm: var(--size-meta);
  --text-base: var(--size-body);
  --text-lg: var(--size-read);
  --leading-body: var(--leading-ui);
  --radius-sm: var(--control-radius-sm);
  --radius-md: var(--control-radius);
  --radius-lg: var(--control-radius);
  --radius-pill: 9999px;
  --elev-flat: none;
  --elev-ring: var(--shadow-card);
  --elev-raised: var(--shadow-raised);
  --focus-ring: 0 0 0 2px var(--bg), 0 0 0 4px var(--blue);
  --motion-fast: var(--dur-fast);
  --motion-base: var(--dur-base);
  --ease-standard: var(--ease-enter);
}
CONTRACT
} >"$out"
