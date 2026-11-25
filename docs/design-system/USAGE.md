# Remit usage

Package guide for Open Design agents, and for the person who turns a design into Vue.

## Read order

1. This file.
2. `DESIGN.md` for intent, palette roles, type, components and do's and don'ts.
3. Paste `tokens.css` into the artifact's first `<style>` block before writing component CSS. It is written from the app
   (`make gen`); never edit it.
4. Open `components.html` when exact sizes, variants or states matter. Copy its component CSS rather than inventing new
   controls.

## Rules

- Use token names, never raw hex outside the pasted `:root` block.
- Name things the way the Vue app does: the class names in `components.html` (`btn-outline`, `b-needs`, `item`,
  `callout note`, `A-engineer`) mirror the Vue component and its variant, so a design ports by swapping.
- One job per hue (ADR-0090): blue is what the owner can act on, cyan work going on, the owner in ink. Other hues are
  states, shown in a chip or badge, never on a card's surface or border.
- Dark only. Dense: 13px body, 14px for long reading, controls on the ruler (24/28/32/36).

## From an Open Design artifact to Vue

Open Design produces HTML. The Vue app (`web/`, Vue 3 + shadcn-vue `reka-nova` + Tailwind 4) already holds every token
and component, so porting a design means replacing each HTML block with its Vue component. Do not copy the artifact's CSS
into the app: the app's `style.css` and `theme.css` already define it.

| HTML in the artifact | Vue in `web/src` |
|---|---|
| `button.btn.btn-{variant}.{size}` | `<Button variant="{variant}" size="{size}">` from `@/components/ui/button` |
| `span.badge.b-{tone}` · `.count` | `<Badge variant="{tone}">` · `size="count"` from `@/components/ui/badge`; pick the tone with `statusTone()` in `ui.ts` |
| `span.agent.A-{role}` | `<AgentBadge agent="{role}" :size="28" />` |
| `article.item` | `<ItemCard>` |
| `div.ui-card > header > h4 + p`, `.content` | `<Card><CardHeader><CardTitle/><CardDescription/></CardHeader><CardContent/></Card>` |
| `div.callout`, `.note`, `.error` | `<Callout>` (tone `ask`), `<Callout tone="note">`, `<Callout tone="error">` |
| `label + input.input` | `<Label>` + `<Input>`; invalid → `aria-invalid="true"` |
| `textarea.input` | `<Textarea>` |
| `div.tabs-list[.line] > button.tab` | `<Tabs><TabsList variant="default\|line"><TabsTrigger value>` + `<TabsContent>` |
| `span.tip > [role=tooltip]` | `<Tooltip><TooltipTrigger as-child/><TooltipContent/></Tooltip>` |
| `hr.sep` | `<Separator>` |
| `.caps`, `.mono`, `.code-chip`, `.owner-bubble` | the same names as Tailwind utilities (`theme.css` `@utility`) |
| any `var(--x)` colour | the Tailwind utility of the same name (`bg-panel2`, `text-mut`, `border-line`) or `var(--x)` in `<style scoped>` |
| inline Lucide `<svg>` | the `@lucide/vue` icon of the same name |

A block with no row here is new. Build it in Vue from the primitives above, and add it to this table and to
`components.html` once it exists in the app.
