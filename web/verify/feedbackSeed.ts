import type { Feedback } from '../src/core/generated/types.generated';
import { feedbackRecord } from './records';

// What agents told the organisation, for the Feedback tab (FeedbackView) on made-up data: unread and read, each kind, one from
// the Assistant (its home is no chain), one with a very long name and a long text, one in Markdown.
const MIN = 60_000;
const iso = (ms: number) => new Date(ms).toISOString();

export function feedbackSeed(now: number): Feedback[] {
  const fb = (id: string, kind: Feedback['kind'], name: string, text: string, agoMin: number, extra: Partial<Feedback> = {}): Feedback => feedbackRecord({
    id, kind, name, text, home: 'busy', cardId: 'eng', agent: 'engineer', model: 'claude-sonnet-5-5', createdAt: iso(now - agoMin * MIN), rev: 1, ...extra,
  });
  return [
    fb('fb_1', 'difficulty', 'Engineer', 'The brief names no repository, so I spent my first four turns finding which one holds the billing code.\n\n- `open_repo` lists nine repositories and none says what it holds\n- The goal mentions "the invoice job" only once\n\nA line in the brief naming the repository would have saved the turns.', 6),
    fb('fb_2', 'suggestion', 'Reviewer', 'Let the reviewer see the engineer\'s test run, not only the diff. I re-ran the suite twice to learn what the engineer already knew.', 41, { cardId: 'rev', agent: 'reviewer' }),
    fb('fb_3', 'complaint', 'Engineer with a very long name that goes on and on and on for the whole width of a list', 'Being sent back three times for the same missing test, each time with a different wording of the finding, is hard to act on. ' + 'It would help if the finding named the test file. '.repeat(6), 180, { readAt: iso(now - 150 * MIN) }),
    fb('fb_4', 'practice', 'Assistant', 'I answer from what the tools show, and I often cannot tell which chain the owner means by "this one". Naming it in his message would let me stop asking.', 26 * 60, { home: 'as_1', cardId: 'c_assistant', agent: 'assistant', readAt: iso(now - 20 * 60 * MIN) }),
  ];
}
