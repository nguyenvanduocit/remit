# Remit

> Category: Developer Tools
> The owner's console for a company of agents. Dark-only, dense, one job per hue (ADR-0090), states in chips.

The code is the source of truth: `web/src/style.css` (`:root` palette, base, focus, reduced motion), `web/src/theme.css`
(shadcn names, radius, the `mono` / `caps` / `code-chip` / `owner-bubble` recipes), `web/components.json` (shadcn-vue
`reka-nova`, Lucide, Tailwind 4) and `web/src/components/ui/` (19 primitives). `tokens.css` beside this file is written
from `style.css` by `make gen` (`scripts/design_tokens.sh`) and checked by `make check-gen`: never edit it by hand.

## 1. Visual Theme & Atmosphere

Remit is a working console, not a marketing site. An owner watches a board of cards, a tree of agents and their chains,
and steps in when an agent needs them. The screen is dense and quiet: a near-black, blue-cool canvas (`#0b0d11`) with
panels stepped up by luminance (`--bg` → `--panel` → `--panel2` → `--panel3`), hairline borders (`--line #262d3a`) and
faint dashed rules between rows (`--rule`). Depth comes from surface steps and half-pixel white rings, not from big
shadows.

Colour carries meaning, one job per hue (ADR-0090). Blue (`#6aa3ff`) is what the owner can act on: the main button, a
link, the focus ring. Cyan is work going on that needs no one. The owner himself is ink, not a hue. Every other hue is a
state, shown in a chip or a badge (an icon or a word on the hue's tint), never as a tinted card or a coloured border.
Nothing is coloured for decoration.

Type is Inter Variable with `"cv11", "ss01"` at a 13px base and 1.45 line height. JetBrains Mono Variable at 11.5px sets
ids, keys, values and code. Small uppercase labels (`caps`: 11px, 500, +0.04em, `--mut`) head sections such as GOAL or
RUNNING 2.

**Key characteristics**
- Dark-only. There is no light theme; `theme.css` notes where one would go.
- Dense: 13px body, the control ruler (24/28/32/36), 20px badge, named rhythms. What the owner reads at length (an
  agent's words, what waits on him, the Assistant) is 14px/1.6.
- One job per hue: blue act, cyan at work, orange waits on the owner, amber warning, green done, red failed, sand a gate or
  note, violet the Assistant; the owner is ink; an agent's badge colour is its identity.
- Agents are letters on their own colour (`AgentBadge`), one fixed pair per role.
- Every interactive element shows a 2px `--ring` (blue) focus outline, offset 2px.
- `prefers-reduced-motion` zeroes every animation and transition.

## 2. Color Palette & Roles

### Surfaces (darkest to lightest)
| Token | Value | Role |
|---|---|---|
| `--bg` | `#0b0d11` | page canvas, code block fill |
| `--bg2` | `#0f1218` | sunken areas, sidebars |
| `--panel` | `#13171e` | cards, columns, drawers (`--card`) |
| `--panel2` | `#181d26` | popovers, secondary buttons (`--popover`, `--secondary`) |
| `--panel3` | `#1e2430` | hover / muted fills, `code-chip` (`--muted`, `--accent`) |
| `--line` | `#262d3a` | borders (`--border`) |
| `--line2` | `#323b4b` | strong hairlines, the owner's bubble border |
| `--line3` | `#6c7890` | the edge of a field the owner types into (`--input`), 3:1 on every surface |
| `--well` | `#08090c` | the pressed surface: the open card, the chosen row (with `--shadow-pressed`) |
| `--rule` | `#1f2531` | faint dashed rules between rows |

### Text
| Token | Value | Role |
|---|---|---|
| `--text` | `#e7eaf0` | primary text (`--foreground`) |
| `--text2` | `#b7bfcc` | body under a title, code text |
| `--mut` | `#8993a4` | labels, metadata, `caps` (`--muted-foreground`) |
| `--faint` | `#838d9e` | timestamps, scrollbar thumb |
| `--on-primary` | `#0b1020` | text on a blue fill |

### Hues and their state roles
| Hue | Base | Role | Quiet forms |
|---|---|---|---|
| Blue | `#6aa3ff` | what the owner can act on: main button, link, focus ring | `-tint` `rgba(0,143,245,.13)`, `-bg #162544`, `-line #2b4776`, `-text #6fb4ff`, `-hi #8fbaff`, `-mark #4d8ff0` |
| Cyan | `#45c8d8` | at work, needs no one: running, in progress, new (`--info`) | `-tint`, `-bg #0f2226`, `-line #22525a`, `-text #7fdde8` |
| Green | `#43c38a` | success, done | `-tint`, `-bg #10211a`, `-line #285a43`, `-text #7fdcae` |
| Amber | `#f0b44c` | warning, waiting | `-bg #1d160b`, `-bg2 #3a2a0e`, `-line #6b5022` |
| Red | `#f06a6a` | danger, failed (`--destructive`) | `-tint`, `-bg #1d1012`, `-bg2 #2a1212`, `-line #6b2a2a`, `-text #ff9a9a`, `-hi #ffd0d0` |
| Orange | `#f08c3c` | an agent that needs the owner | `-tint`, `-bg #21160d`, `-line #6e4422`, `-text #f3a35c` |
| Sand | `#e9cf94` | a gate, a Chair's note, decided | `-tint`, `-bg #191710`, `-line #62553a` |
| Violet | `#a98bff` | the Assistant, QA | `-bg #241d3d`, `-line #4a3a7a`, `-text #c7b6ff` |
| Pink | `#ff8fb0` | release agent | — |

How to use a quiet form: `-tint` sits behind a chip (`IconChip`), `-bg` behind a tag, `-line` is a badge's border,
`-text` is text on a tint. A state badge is `border-<hue>/30 bg-<hue>/10 text-<hue>`. A card or a callout is never
tinted and never takes a coloured border. The owner is ink: badge `owner` is `border-line2 bg-panel3 text-text`, his
bubble `owner-bubble` (`--panel3`, `--line2`).

### Overlays and lines
- `--veil` / `--veil2` / `--veil3`: white at 6 / 9 / 18 % for pills, hovers and hairline rings.
- `--tree-line` `rgba(90,150,255,.42)`: the lines of an outline (agent tree, an event's steps).
- `--meter-on` `#6c7fe0`: a used cell of the context meter.

### Agent badges
Each known agent is a letter on its own background: research cyan on `#12343a`, architect blue on `#17284a`, engineer
green on `#14331f`, reviewer amber on `#3a2d12`, QA violet on `#2c1f3c`, PM `#c6b0ff` on `#2d2141`, chair `#d6c8ff` on
a `#2a2150 → #173055` gradient, critic `#ffb1c9` on `#3a2330`, security `#ffb37a` on `#3a2516`, release pink on
`#3a1b25`, human and judge sand, scout `--blue-hi` on `#1d2a3a`. Unknown agents get a colour from their name.

## 3. Typography Rules

### Families
- **Sans**: `"Inter Variable", Inter, ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif`, with
  `font-feature-settings: "cv11", "ss01"`. Fonts ship with the app; no Google Fonts import.
- **Mono**: `"JetBrains Mono Variable", "JetBrains Mono", ui-monospace, "SF Mono", Menlo, monospace`.

### Tokens
`--size-read` 14px with `--leading-read` 1.6 (`text-read`): what the owner reads at length, an agent's words (`Markdown
type="body"`), the conversation's bubbles, the detail of what waits on him. `--size-body` 13px with `--leading-ui` 1.45: the
UI's body, the board, the tree. `--size-meta` 12px (`text-meta`): meta beside them. Controls take `--control-text-sm` 12px
or `--control-text-md` 13px by their step.

### Scale (measured from the components)
| Size | Use |
|---|---|
| 10.5px | tiny meta inside dense rows |
| 11px | `caps` labels, badges, mono in callouts |
| 11.5px | `mono` recipe, `code-chip`: ids, keys, values, inline code |
| 12px | secondary rows, small buttons |
| 12.5px | default button, callout heading, list rows |
| 13px | body (`body { font: 13px/1.45 }`) |
| 14–15px | card and drawer titles |
| 18px | section heading |
| 26px | page title |

Weights: 400 body, 500 labels and buttons, 600 titles, 700 callout headings and agent letters.

### Recipes (`theme.css`)
- `mono`: mono family, 11.5px.
- `caps`: 11px, 500, `letter-spacing: .04em`, uppercase, `--mut`. A label in a tone's colour sets its colour after.
- `code-chip`: `1px 5px` padding, 4px radius, `--panel3` fill, mono 11.5px.
- `owner-bubble`: `--line2` border, `--panel3` fill (ink, ADR-0090), radius `12px 12px 3px 12px` (tail on the owner's side).

## 4. Component Stylings

shadcn-vue (style `reka-nova`) on Reka UI, Tailwind 4, Lucide icons (one outlined family, `size-4` default, `size-3`
in badges). Primitives: badge, button, card, collapsible, dialog, dropdown-menu, input, input-group, label, popover,
resizable, select, separator, sonner, table, tabs, textarea, toggle, toggle-group, tooltip.

### The control ruler (`--control-*`)
Every button, field, select and toggle stands on one of four steps, each with its padding, text and corner:

| Step | Height | Padding | Text | Radius | Button size | Field size |
|---|---|---|---|---|---|---|
| xs | `--control-xs` 24 | `--control-pad-xs` 8 | `--control-text-sm` 12 | `--control-radius-sm` 6 | `xs`, `icon-xs` | — |
| sm | `--control-sm` 28 | `--control-pad-sm` 10 | `--control-text-sm` 12 | `--control-radius-sm` 6 | `sm`, `icon-sm` | `sm` |
| md | `--control-md` 32 | `--control-pad-md` 12 | `--control-text-md` 13 | `--control-radius` 8 | `default`, `icon` | `default` |
| lg | `--control-lg` 36 | `--control-pad-lg` 14 | `--control-text-md` 13 | `--control-radius` 8 | `lg`, `icon-lg` | — |

Utilities: `h-control-md`, `size-control-sm`, `px-control-pad-md`, `text-control-md`, `rounded-control(-sm)`. A caller
picks a size (`<Input size="sm">`, `<SearchBox size="sm">`, `<Button size="sm">`), never a height class. Below `md` a
field's text is 16px, which keeps a phone from zooming in.

### Buttons (`ui/button`)
- Base: the ruler's radius and text, 1px transparent border, 500 weight, colour transitions at `--dur-fast`, presses down 1px.
- Variants: `default` blue fill with `--on-primary` text · `outline` border + `--background`, hover `--muted` ·
  `secondary` `--panel2` · `ghost` transparent, hover `--muted` · `destructive` red at 10–20 % with red text ·
  `success` green outline tint · `link` blue, underline on hover.
- One solid blue button per view for the main action; the rest are outline, secondary or ghost.

### Badges (`ui/badge`)
Pill (`rounded-4xl`), 20px high, 8px x-padding, 11px, 400 weight. `count` size: 18px min square for "Board 3".
Status variants (picked by `statusTone` in `ui.ts`, `Tone = quiet | info | success | warning | gate | needs | danger`):
- `quiet` border + `--secondary` + `--muted-foreground`
- `info` / `success` / `warning` / `danger`: `border-<c>/30 bg-<c>/10 text-<c>`
- `gate` sand, `needs` orange (`bg-orange-tint text-orange-text`)
- `owner` blue-line / blue-bg / blue-text; `assistant` violet-line / violet-bg / violet-text

### Cards and panels
- Board card (`ItemCard.vue`): `--panel` fill, 8px radius, `9px 10px` padding, `--shadow-card` (half-pixel `--veil2`
  ring + 1px shadow), hover `--shadow-card-hover`, the open card pressed into the page (`--well`, `--shadow-pressed`).
  A chosen row in a list or tree (`NavRow`, `TreeRow`) is pressed the same way. Never a ring or glow.
- Generic card (`ui/card`): `--card` fill, 12px radius, 16px padding, a 1px ring of text at 10 %, 16px / 500 title.
- Popovers and dialogs use `--panel2` and `--shadow-pop` / `--shadow-raised`.

### Callouts (`Callout.vue`)
A neutral card: `--panel`, `--shadow-card`, 8px radius, `--space-card` padding, no coloured border. Its tone is a chip
(`IconChip`, 20px, an icon on the hue's `-tint`, the glyph in the hue) beside the heading, or beside the body without one:
`ask` orange (`waits`, the default), `note` sand (`comment`), `error` red (`alert`); `icon` names another. Heading 13px /
600 in `--text`. Embedded `pre`: `--bg`, `--line` border, 7px radius, mono 11px `--text2`.

### Inputs
`--input` (`--line3`) border, `--background` fill, on the ruler (`default` 32, `sm` 28), blue ring on focus, red border
and ring when `aria-invalid`.

### Icons (`Icon.vue`)
One 24-unit grid, a 2-unit stroke with round caps and joins, `currentColor`. `name` from `ICONS` (`IconName`); an unknown
name draws a plain ring. `label` makes it an image a screen reader names; without it, it is decoration. Interface marks:
check, plus, x, chevrons, arrows, search, filter, copy, link, external, pencil, trash, archive, more, play, pause, stop,
interrupt, clock, eye, key, inbox, chart, person, help, alert, spark, gear, list, file, book, terminal, download, comment,
scale, repeat, dot, circle. Domain glyphs: `chain`, `chair`, `assistant`, `agent`, `employee`, `mentor`, `crew`, `gate`,
`judge`, `release`, `merge`, `signal`, `subscription`, `event`, `schedule`, `objective`, `waits`, `playbook`, `skill`,
`memory`, `source`, `extension`. The gallery is `web/verify/icons.html`.

### Scrollbars
10px, no track, a `--faint` thumb inset as a pill (3px transparent border, 8px radius) that lightens to `--mut` on
hover and `--text2` while dragging.

## 5. Layout Principles

- Full-viewport application: `html, body, #app` at 100% height, body does not scroll; panes scroll on their own and
  resize (`ui/resizable`). No centred page container.
- Main surfaces: board (columns of cards), chain page, agent drawer, card drawer, sidebars.
- Spacing on Tailwind's 4px grid, with the repeated rhythms named once (`style.css`, utilities by the same name):
  `--space-icon` 6 (`gap-icon`: an icon and its words), `--space-stack` 8 (`gap-stack`: items of a list),
  `--space-card` 10 (`p-card`: inside a card or callout), `--space-gutter` 10 (`gap-gutter`: board columns),
  `--space-pane` 16 (`px-pane`: a pane's edge). 2px half-steps appear only inside dense rows.
- The board shows as many whole 300px-or-wider columns as it holds, snaps to a column, and names what lies out of view
  at the edge (a fade and a button). Too narrow for two, it shows one, with a row of the columns' names to switch.
- Radius scale: 4px chips · 6px small controls · 8px buttons, cards, callouts (`--radius: .5rem`) · 12px large
  surfaces · pill for badges.
- Rows are separated by dashed `--rule` lines rather than boxes.

## 6. Depth & Elevation

| Level | Token | Use |
|---|---|---|
| Flat | none | rows, inline items |
| Card | `--shadow-card` | cards on a panel |
| Card hover | `--shadow-card-hover` | hovered card |
| Pop | `--shadow-pop` | menus, popovers |
| Raised | `--shadow-raised` | dialogs, drawers |

Higher surfaces are also lighter (`--panel` → `--panel2` → `--panel3`). Shadows stay near-black at 45–50 %.

## 7. Do's and Don'ts

### Do
- Use tokens by name: `var(--x)` in scoped CSS, or the Tailwind utility (`bg-panel2`, `text-mut`, `border-line`).
- Show state with a hue's quiet form (tint + line + text) plus a word or icon, never colour alone.
- Keep blue for what the owner can act on; cyan for work going on; the owner in ink (ADR-0090).
- Set ids, keys, branches and values in `mono` or `code-chip`.
- Use `caps` for section labels.

### Don't
- Don't write a raw colour, font or shadow in a component; add a token to `style.css` first.
- Don't add a light theme ad hoc, gradients (except the chair badge) or decorative colour.
- Don't use a second solid button for the same action, or turn hover text grey.
- Don't add a Google Fonts import (shadcn's `add` appends one; delete it).
- Don't animate for decoration. Motion is `--dur-fast` 120ms (hover, press), `--dur-base` 160ms (a popover, a menu),
  `--dur-panel` 220ms (a dialog) on `--ease-enter`; only what arrives moves, what closes goes at once, and all of it
  stops under reduced motion.
- Don't tint a card or a callout, or draw its state as a coloured border, ring or glow.

## 8. Responsive Behavior

Remit is a desktop console that holds up on a phone. Panes resize by drag. The board fits whole columns to its width
and, below two, shows one with a switcher. Below 641px the top bar folds its five areas into a menu named by the current
area. Touch targets are 24–36px (the control ruler), not 44px.

## 9. Agent Prompt Guide

### Quick reference
- Canvas `#0b0d11`, panel `#13171e`, popover `#181d26`, hover `#1e2430`, border `#262d3a`, rule `#1f2531`.
- Text `#e7eaf0`, body `#b7bfcc`, label `#8993a4`, faint `#838d9e`.
- Action blue `#6aa3ff` with text `#0b1020` on it; at-work cyan `#45c8d8`; the owner in ink.
- States: green `#43c38a`, amber `#f0b44c`, red `#f06a6a`, orange `#f08c3c` (needs owner), sand `#e9cf94` (gate),
  violet `#a98bff` (Assistant).
- Inter Variable 13px/1.45, JetBrains Mono 11.5px, caps labels 11px / 500 / .04em.
- 8px radius, the control ruler 24/28/32/36, 20px pill badges, 4px grid.

### Example prompts
- "A board column: `--panel` cards with `--shadow-card`, 8px radius, `9px 10px` padding (`ItemCard.vue`), title
  13px/600, a `caps` label, a status badge in its tone, a mono id in `--mut`."
- "An agent drawer header: the agent's letter badge, name 15px/600, a `needs` badge if it waits on the owner, actions
  as one blue button and ghost icon buttons."
- "A callout asking the owner to decide: a neutral `--panel` card, an orange chip with the `waits` icon beside a 13px/600
  heading in `--text`, body `--text2`."

### From a design to Vue
Open Design produces HTML; the app is Vue 3 + shadcn-vue. `components.html` names each component by its Vue component
and variant, and `USAGE.md` has the full table. Port a design by replacing each block with the Vue component it names;
the app's `style.css` and `theme.css` already hold every token, so no CSS moves across.

### Departures from the Open Design baseline
- Body text is 13px, not 16px: the console is dense by design.
- Touch targets are 24–36px: desktop only.
- Display sizes above 26px do not exist in the app; `--text-3xl` / `--text-4xl` in `tokens.css` are extrapolated for
  decks and marketing pages.
