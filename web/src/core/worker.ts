// The SharedWorker that runs the browser's one client core (ADR-0095 decision 4): every tab of the app connects to it
// (main.ts, core/client.ts `overPort`) and the core's `Tabs` (crates/client-web `worker.rs`) serves each on its port.
// Ports that connect while the core's module loads wait for it; the listener is set first, as a connection that finds
// none is lost. Each dial asks the server where its link is and the hash of the certificate it presents (/link.json):
// the server makes a new certificate at each start, so a dial after a restart trusts the new one.
import init, { Tabs } from './pkg/web/client_web.js';

const waiting: MessagePort[] = [];
let tabs: Tabs | undefined;

globalThis.addEventListener('connect', (e) => {
  const port = e instanceof MessageEvent ? e.ports[0] : undefined;
  if (!port) return;
  if (tabs) tabs.connect(port);
  else waiting.push(port);
});

await init();
tabs = Tabs.browser(async () => {
  const r = await fetch('/link.json');
  if (!r.ok) throw new Error(`/link.json answered ${r.status}`);
  return r.json() as Promise<unknown>;
});
for (const port of waiting.splice(0)) tabs.connect(port);
