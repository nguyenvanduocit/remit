import { createCn } from 'cn/config';

// Merges class names the way Tailwind applies them: of two utilities that set the same property, the later wins. The names
// theme.css adds to Tailwind's scales are taught to it by scale, or it would not see that they set the same property as a
// default name: a spacing (the control ruler's control-*, icon, stack, card, gutter, pane), or a caller's `p-0 h-auto` would
// not replace a button size's `px-control-pad-xs h-control-xs` and both would stay; a font size (read, meta, control-*), or it
// would take `text-read` for a colour and keep `text-xs text-read`; a radius (control, control-sm), or a size's
// `rounded-control-sm` would stand beside the base's `rounded-control`. utils.test.ts holds this list to theme.css.
const THEME = {
  spacing: ['control-xs', 'control-sm', 'control-md', 'control-lg', 'control-pad-xs', 'control-pad-sm', 'control-pad-md', 'control-pad-lg', 'icon', 'stack', 'card', 'gutter', 'pane'],
  text: ['control-sm', 'control-md', 'read', 'meta'],
  radius: ['control-sm', 'control'],
};
export const cn = createCn({ extend: { theme: THEME } });
