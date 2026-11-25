// A change an agent asked to write, as the owner reads it in his inbox (InboxCallout), in the server's own words: the item the Chair's
// propose_extension raises for a source supplied as a plugin binary (orch/extensions.go), with what the plugin says it is read before it exists.
// ?item=sensor shows the standing sensor a Chair asks for. ?item=amend shows the Chair amending an objective whose fields run to several lines, each a block as it is and as it would be (orch/change_item.go).
// Open /verify/inboxchange.html; ?width=<px> narrows the column the item sits in.
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp, h } from 'vue';
import { createMemoryHistory, createRouter } from 'vue-router';
import InboxCallout from '../src/components/InboxCallout.vue';
import { TooltipProvider } from '../src/components/ui/tooltip';
import { apiKey } from '../src/core/api';
import { api } from '../src/replica';
import { routes } from '../src/router';
import { fakeServer, snapshotOf } from './fake';
import type { InboxItem } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { inboxRecord, optionOf } from './records';

const extension = inboxRecord({
  id: 'i_ext', home: 'ch_1', cardId: 'c_chair', kind: 'change', title: "Chair asks to write extension vnpay", status: 'open', createdAt: new Date().toISOString(), rev: 1,
  detail: "Chair asks to add the source `vnpay` (VNPay) as a plugin. Agents then call its tools through the gate with the grants you give their chain; it runs as a process of this server with the credential of the connection it is called through. It is held to the program shown by its hash: a file that is another does not run.\n\nChair asks to write extension `vnpay`. These fields move a rail, so it is not written until you answer:\n\n- **capability**: `customers`\n- **program**: `sha256 56c0b3182827f88bec5a80532d399b9d045042508a7dbe13db1af77d2949a922, 9.1 MiB, file 56c0b3182827-vnpay`\n- **tools**: `list_failed_payments (read)`, `refund_payment (irreversible)`\n- **vars**: `VNPAY_KEY (secret): the merchant key`\n- **poll**: `true`\n- **ui**: `remoteEntry.js`, `page payments`\n\nApply writes it as shown. Not now writes nothing, and the agent is told.",
  options: [optionOf('apply', 'Apply'), optionOf('not_now', 'Not now')],
});
const amend = inboxRecord({
  id: 'i_amend', home: 'ch_1', cardId: 'c_chair', kind: 'change', changeKind: 'objective', title: 'Chair asks to amend the objective Lights Daily first player', status: 'open', createdAt: new Date().toISOString(), rev: 1,
  detail: "Chair asks to write objective `ob_b6745bdae1ed`. These fields move a rail, so it is not written until you answer:\n\n- **version**: `3`\n- **key result kr_0232df06f5d9**: \n\n  ```\n  A player who clicks a link can play today's Lights Daily puzzle in their browser, with no login.\n  owner: owner\n  judge: owner\n  check: {\"kind\":\"submit\",\"how\":\"The public URL returns 200 and a logged-out browser can play.\"}\n  ```\n\n  → \n\n  ```\n  A player who clicks the link can play and solve today's puzzle in the browser, and the solve is counted exactly once.\n  owner: owner\n  judge: owner\n  check: {\"kind\":\"command\",\"repo\":\"github.com/OWNER/lights-daily\",\"name\":\"npm ci && npm test\"}\n  ```\n- **guardrails**: \n\n  ```\n  - Disclosure: every post says Lights Daily is a new indie game made by an AI team.\n  - Community rules: each post follows that community's written self-promotion rules.\n  ```\n\n  → nothing\n\nApply writes it as shown. Not now writes nothing, and the agent is told.",
  options: [optionOf('apply', 'Apply'), optionOf('not_now', 'Not now')],
});
// The standing sensor a Chair asks for with create_standing_sensor (orch/sensor_tools.go, ADR-0065), as holdSensor raises it.
const sensor = inboxRecord({
  id: 'i_sensor', home: 'ch_1', cardId: 'c_chair', kind: 'change', changeKind: 'sensor', title: 'Chair asks to write sensor issues', status: 'open', createdAt: new Date().toISOString(), rev: 1,
  detail: "Chair asks to write sensor `issues`. These fields move a rail, so it is not written until you answer:\n\n- **reads**: `{\"signal\":\"event:issues\",\"kind\":\"poll\",\"source\":\"github\",\"connection\":\"default\",\"query\":\"repo:acme/app is:issue\"}`\n\nApply writes it as shown. Not now writes nothing, and the agent is told.",
  options: [optionOf('apply', 'Apply'), optionOf('not_now', 'Not now')],
});
const params = new URLSearchParams(location.search);
const items: Record<string, InboxItem> = { amend, sensor };
const item = items[params.get('item') ?? ''] ?? extension;
const width = params.get('width') ?? '440';
fakeServer({ appSnapshot: () => snapshotOf({}), inboxAnswer: () => ({}) });
createApp({ render: () => h(TooltipProvider, { delayDuration: 300 }, () => h('div', { style: { width: `${width}px`, padding: '16px' } }, [h(InboxCallout, { item })])) })
  .use(createRouter({ history: createMemoryHistory(), routes }))
  .provide(apiKey, api)
  .mount('#app');
