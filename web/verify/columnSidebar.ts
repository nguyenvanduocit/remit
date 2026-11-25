// ColumnSidebar (the board's columns as marks, beside a chain) alone, in a 320px column, on made-up counts: hover a mark to see
// it swell with its neighbours. Open /verify/columnSidebar.html; ?col=<key> picks the column shown (default: running).
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp, h, ref } from 'vue';
import ColumnSidebar from '../src/components/ColumnSidebar.vue';
import { TooltipProvider } from '../src/components/ui/tooltip';
import '../src/theme.css';

const columns = [
  { key: 'running', label: 'Running', count: 1 },
  { key: 'waiting', label: 'Waiting for you', count: 83 },
  { key: 'watching', label: 'Watching', count: 6 },
  { key: 'done', label: 'Done', count: 12 },
  { key: 'away', label: 'Put away', count: 45 },
  { key: 'a', label: 'Queued', count: 0 },
  { key: 'b', label: 'Paused', count: 2 },
];
const shown = ref(new URLSearchParams(location.search).get('col') ?? 'running');

createApp({
  render: () =>
    h(TooltipProvider, null, () =>
      h('div', { style: 'width:320px;height:100vh', class: 'flex' }, [
        h(ColumnSidebar, { columns, shownKey: shown.value, slide: 'next', label: 'Board columns', onGo: (k: string) => (shown.value = k) }, {
          head: () => h('div', { class: 'h-9 rounded-md border border-line px-3 text-[12.5px] leading-9 text-mut' }, 'Search'),
          default: () => h('div', { key: shown.value, class: 'h-20 rounded-md border border-line p-3 text-mut' }, `${shown.value} column`),
        }),
      ]),
    ),
}).mount('#app');
