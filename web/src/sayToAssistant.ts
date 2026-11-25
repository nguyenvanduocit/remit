// What a page starts in the Assistant's box for the owner to finish and send (New client key, a page with nothing
// on it yet): the words are put in its box as its draft, and the Assistant opens over the page.
import { assistant } from './assistant';
import { prefill } from './drafts';
import { useAssistant } from './nav';

export function useSayToAssistant(): (words: string) => void {
  const { show } = useAssistant();
  return (words) => {
    const a = assistant.view?.assistant;
    if (a) prefill({ home: a.id, cardId: a.cardId }, words);
    show();
  };
}
