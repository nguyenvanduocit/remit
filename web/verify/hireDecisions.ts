// What a chain's decisions say of a hire whose model a decider chose and whose effort it only suggested, and of the climb that
// raised it a tier (ADR-0118): a tried model, a hire with only the effort suggested, the same hire of a kept employee, and
// Remit's raise of a card that failed. No server, no sign-in. Open /verify/hireDecisions.html.
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp, h } from 'vue';
import CardDecisions from '../src/components/CardDecisions.vue';
import { TooltipProvider } from '../src/components/ui/tooltip';
import type { Decision } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { actorOf, decisionRecord } from './records';

const MIN = 60_000;
const t0 = Date.now() - 40 * MIN;
const at = (min: number) => new Date(t0 + min * MIN).toISOString();
const chair = actorOf('agent', 'chair');
const decide = (id: string, min: number, fields: Partial<Decision> & Pick<Decision, 'kind' | 'text'>) =>
  decisionRecord({ chainId: 'ch', createdAt: at(min), id, ...fields });

const decisions: Decision[] = [
  decide('d1', 1, { kind: 'hire', by: chair, text: 'Hire Researcher · support tickets (researcher, sonnet · medium), trying sonnet; it moves up if it struggles (Jev 1.14, 71% sure); effort left as set; Jev 1.14 suggests medium, 82% sure.', why: 'The owner wants a weekly summary of what customers ask for.', cardId: 'c1' }),
  decide('d2', 5, { kind: 'hire', by: chair, text: 'Hire Engineer · orders migration (engineer, opus · high), effort left as set; Jev 1.14 suggests high, 91% sure.', why: 'It must add `status` to `orders` without taking checkout down.', cardId: 'c2' }),
  decide('d3', 9, { kind: 'hire', by: chair, text: 'Put Minh to work (researcher, worker, haiku · low), model chosen by Jev 1.14, 97% sure; effort left as set; Jev 1.14 suggests low, 95% sure.', why: 'A kept researcher is idle and the task is plain.', cardId: 'c3' }),
  decide('d4', 20, { kind: 'handover', by: actorOf('remit', 'climb'), text: 'Raise Researcher · support tickets from sonnet · medium to opus · medium.', why: 'it failed: Claude Code ended its turn with an error', cardId: 'c1' }),
  decide('d5', 21, { kind: 'hire', by: actorOf('remit', 'climb'), text: 'Put Researcher · support tickets (researcher, worker, opus · medium).', why: 'it failed: Claude Code ended its turn with an error', cardId: 'c4' }),
];

createApp({ render: () => h(TooltipProvider, null, () => h('div', { style: 'width: 720px; padding: 24px' }, [h(CardDecisions, { decisions })])) }).mount('#app');
