// Remit's icons (Icon.vue) and the callout's three tones (Callout.vue, ADR-0090), on no data: every name in ICONS at the
// sizes the app draws them (12, 14, 16) and large (32) to judge the grid and the stroke, an unknown name (its plain ring),
// and a callout of each tone with and without a heading. Open /verify/icons.html.
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp, h } from 'vue';
import Icon, { ICONS } from '../src/components/Icon.vue';
import Callout from '../src/components/Callout.vue';
import '../src/theme.css';

const names = [...Object.keys(ICONS), 'no-such-icon'];
const cell = (name: string) =>
  h('figure', { class: 'm-0 flex flex-col items-center gap-2 rounded-lg bg-panel p-card text-text', 'data-icon': name }, [
    h('div', { class: 'flex items-end gap-3' }, [12, 14, 16, 32].map((size) => h(Icon, { name, size }))),
    h('figcaption', { class: 'mono text-mut' }, name),
  ]);
const tones = (['ask', 'note', 'error'] as const).flatMap((tone) => [
  h(Callout, { tone }, { title: () => `A ${tone} callout with a heading`, default: () => h('p', { class: 'm-0 text-text2' }, 'Its body sits under the heading, in the card’s full width.') }),
  h(Callout, { tone }, { default: () => `A ${tone} callout with no heading: the chip stands beside the body.` }),
]);

createApp({
  render: () =>
    h('main', { class: 'flex h-full flex-col gap-6 overflow-auto p-6' }, [
      h('section', { 'aria-label': 'Icons', class: 'grid gap-stack', style: 'grid-template-columns: repeat(auto-fill, minmax(150px, 1fr))' }, names.map(cell)),
      h('section', { 'aria-label': 'Callouts', class: 'grid max-w-[880px] grid-cols-2 gap-stack' }, tones),
    ]),
}).mount('#app');
