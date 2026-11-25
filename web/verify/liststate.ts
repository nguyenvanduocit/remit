// The one grammar every list the screen asks the server for speaks (ListState.vue), on no data: each state as it shows, the
// same line in each list. Loading (the line waits 150 ms, so it is shot once it shows), failed with nothing held, failed again
// with what was read before still on screen (partial), held and empty, and held with rows; then the same in a narrow column,
// and the real MemoryBook through it. Try again logs to the console. Open /verify/liststate.html.
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp, h } from 'vue';
import { createMemoryHistory, createRouter } from 'vue-router';
import ListState from '../src/components/ListState.vue';
import MemoryBook from '../src/components/MemoryBook.vue';
import '../src/theme.css';

const retry = () => console.log('retry');
const rows = () => h('ul', { class: 'm-0 flex list-none flex-col gap-1 p-0 text-text' }, ['Backfill missing zone groups', 'Wrap store errors with the chain id'].map((t) => h('li', { class: 'rounded-md bg-panel2 px-3 py-2' }, t)));
const cases: [string, { held: boolean; error?: string; empty?: boolean }][] = [
  ['loading', { held: false }],
  ['error', { held: false, error: 'the forge answered 502 Bad Gateway' }],
  ['partial', { held: true, error: 'the link went down' }],
  ['empty', { held: true, empty: true }],
  ['rows', { held: true }],
];
const column = (width: number) =>
  h('div', { class: 'flex flex-col gap-4', style: { width: `${width}px` } }, cases.map(([name, p]) =>
    h('section', { class: 'rounded-lg bg-panel p-card', 'data-case': name }, [
      h('h2', { class: 'caps mb-2 text-mut' }, name),
      h(ListState, { what: 'the history', retry, ...p }, { empty: () => 'No changes recorded yet: each write to it shows here, with who wrote it.', default: rows }),
    ]),
  ));
const memories = h('section', { class: 'flex flex-col gap-4', style: { width: '420px' } }, [
  h('div', { class: 'rounded-lg bg-panel p-card', 'data-case': 'memory-error' }, [h(MemoryBook, { book: undefined, error: 'the server is restarting', loading: false, who: 'Engineer #2', viewKey: 'memory', surface: 'panel', retry })]),
  h('div', { class: 'rounded-lg bg-panel p-card', 'data-case': 'memory-empty' }, [h(MemoryBook, { book: { memories: [], teachings: [] }, error: '', loading: false, who: 'Engineer #2', viewKey: 'memory', surface: 'panel', retry })]),
]);
// The book keeps its open tab in the address, so it needs a router.
const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/', component: { render: () => null } }] });
await router.push('/');
createApp({ render: () => h('div', { class: 'flex items-start gap-6 p-4' }, [column(560), column(260), memories]) }).use(router).mount('#app');
