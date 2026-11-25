// What a stream shows of the agents around it: the name of the agent that sent a message, where the agent a message went to
// stands (whether it is reading it). A stream draws the open chain's agents unless a component above it provides
// another (the Assistant's panel provides the Assistant's).
import { inject, provide, type InjectionKey } from 'vue';
import type { AgentStanding } from './core/generated/types.generated';
import { agentOf, NOWHERE, opened } from './openChain';
import { assistant } from './assistant';
import { agentName } from './ui';

export interface StreamContext {
  nameOf: (cardId: string) => string; // an agent's name, or its id when it is not one of these
  agentOf: (cardId: string) => AgentStanding;
  chainTab?: string; // the chain ids an agent names link to this tab of the chain ('': its default tab); none: no link
}
const streamKey: InjectionKey<StreamContext> = Symbol('stream');

export const chainStream: StreamContext = {
  nameOf: (id) => {
    const c = opened.view?.cards.find((x) => x.id === id);
    return c ? agentName(c) : id;
  },
  agentOf,
};

// The Assistant's stream: its own card, the agents of chains that reply to what it sent them, named by their id, and the
// chains it names, each a link that opens it.
export const assistantStream: StreamContext = {
  nameOf: (id) => (id === assistant.view?.card.id ? 'Assistant' : id),
  agentOf: (id) => (assistant.view && id === assistant.view.card.id ? assistant.view.agent : NOWHERE),
  chainTab: '',
};

export const provideStream = (ctx: StreamContext): void => provide(streamKey, ctx);
export const useStream = (): StreamContext => inject(streamKey, chainStream);
