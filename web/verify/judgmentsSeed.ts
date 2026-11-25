// Made-up Judgments pages (ADR-0118), as the server lays them out in words: a busy question whose answering model took
// over by its numbers, with mistakes (one escalated, one asked in no chain); a new one with too few results to tell; one
// that only advises, its one mistake's pick very long; one no certainty is safe for yet; and one asked once, with no
// result and no mistake. Quiet: none asked yet.
import type { DeciderStanding, JudgmentFigure, JudgmentMistake, Judgments } from '../src/core/generated/types.generated';

const MIN = 60_000;
const iso = (ms: number) => new Date(ms).toISOString();
const JEV = 'typesafe/jev-1.14';
const LIQUID = 'liquid/d1.2-ultra';
const PPLX = 'perplexity/pplx-decider-v1.3:free';
const NAME: Record<string, string> = { [JEV]: 'Jev 1.14', [LIQUID]: 'Liquid D1.2 Ultra', [PPLX]: 'Perplexity Decider (free)' };

const figure = (label: string, hint: string, value: string, over: string): JudgmentFigure => ({ label, hint, value, over });
const RIGHT = 'Of its answers whose result is known, the share that proved right. It counts once 30 results are known.';
const alone = (who: string) => `How often its answer was sure enough to act on without ${who}, who decides the rest.`;
const COST = "What every model's answers to it cost, those of the models measured beside it included.";
const model = (decider: string, answers: boolean, rest: Omit<DeciderStanding, 'name' | 'decider' | 'role'>): DeciderStanding => ({
  name: NAME[decider] ?? decider,
  decider,
  role: answers ? 'Answers it' : 'Measured beside it',
  ...rest,
});

export function judgmentsSeed(quiet: boolean, now = Date.now()): Judgments {
  if (quiet) return { questions: [] };
  const mistake = (i: number, over: Partial<JudgmentMistake>): JudgmentMistake => ({
    id: `ju_${i.toString(16).padStart(6, '0')}`,
    text: 'Said two memories were the same lesson; the librarian kept them apart',
    sure: 'was 93% sure',
    at: iso(now - (i * 97 + 12) * MIN),
    home: 'ch_librarian',
    card: '',
    ...over,
  });
  return {
    questions: [
      {
        key: 'memory.same_claim@1',
        title: 'Finding duplicate memories',
        summary: 'Settled: it decides alone when at least 86% sure, a level that has proved right at least 90% of the time.',
        answeredBy:
          'Answered by Perplexity Decider (free) — switched from Jev 1.14 because its answers proved more reliable. Jev 1.14 and Liquid D1.2 Ultra answer beside it, to show whether one would do better.',
        figures: [
          figure('Right', RIGHT, '96%', '277 of 288 checked'),
          figure('Decides alone', alone('the librarian'), '73%', '301 of 412'),
          figure('Cost', COST, '$0.02', '412 checks'),
        ],
        mistakes: [
          mistake(1, {}),
          mistake(2, { sure: 'was 88% sure' }),
          mistake(3, { text: 'Said two memories taught different lessons; the librarian merged them', sure: 'was only 61% sure, so the librarian decided' }),
          mistake(4, { sure: 'was 90% sure', home: '' }),
          mistake(5, { sure: 'was 87% sure', at: iso(now - 9 * 24 * 60 * MIN) }),
        ],
        details: {
          facts: [
            { label: 'Question', value: 'memory.same_claim@1' },
            { label: 'Decides alone when', value: 'at least 86% sure' },
            { label: 'Must be right', value: '90% of the times it decides alone' },
            { label: 'Answers in all', value: "1236, the measured models' included" },
          ],
          deciders: [
            model(PPLX, true, { judgments: 412, outcomes: 288, unknown: 14, brier: 0.0412, auc: 0.981, costUsd: 0 }),
            model(LIQUID, false, { judgments: 412, outcomes: 288, unknown: 14, brier: 0.0538, auc: 0.996, costUsd: 0.00412 }),
            model(JEV, false, { judgments: 412, outcomes: 288, unknown: 14, brier: 0.1871, auc: 0.903, costUsd: 0.0143 }),
          ],
        },
      },
      {
        key: 'hire.tier_can@1',
        title: 'Choosing the model for a hire',
        summary: 'Not settled yet: until 30 results are known it decides alone only when at least 90% sure.',
        answeredBy: 'Answered by Jev 1.14. Liquid D1.2 Ultra and Perplexity Decider (free) answer beside it, to show whether one would do better.',
        figures: [
          figure('Right', RIGHT, '—', '6 checked, too few to tell'),
          figure('Decides alone', alone('the usual model'), '53%', '9 of 17'),
          figure('Cost', COST, '<$0.01', '17 checks'),
        ],
        mistakes: [mistake(6, { text: 'Chose the standard tier for a hire; the card needed a stronger one', sure: 'was 92% sure', home: 'ch_7e21', card: 'ca_7e21b04c' })],
        details: {
          facts: [
            { label: 'Question', value: 'hire.tier_can@1' },
            { label: 'Decides alone when', value: 'at least 90% sure, until 30 results are known' },
            { label: 'Must be right', value: '80% of the times it decides alone' },
            { label: 'Answers in all', value: "43, the measured models' included" },
          ],
          deciders: [
            model(JEV, true, { judgments: 17, outcomes: 6, unknown: 0, brier: 0.112, auc: 0.875, costUsd: 0.00018 }),
            model(LIQUID, false, { judgments: 16, outcomes: 6, unknown: 0, brier: 0.164, auc: 0.75, costUsd: 0.00033 }),
            model(PPLX, false, { judgments: 10, outcomes: 0, unknown: 0, costUsd: 0 }),
          ],
        },
      },
      {
        key: 'inbox.option@1',
        title: 'Pre-answering inbox items',
        summary: 'It only advises; it has been right 87% of the 31 times we could check.',
        answeredBy: 'Answered by Jev 1.14. Liquid D1.2 Ultra and Perplexity Decider (free) answer beside it, to show whether one would do better.',
        figures: [
          figure('Right', RIGHT, '87%', '27 of 31 checked'),
          figure('Decides alone', alone('the Assistant'), 'Never', 'it advises the Assistant'),
          figure('Cost', COST, '<$0.01', '58 checks'),
        ],
        mistakes: [
          mistake(8, {
            text: 'Suggested “Grant the staging Bytebase connection read and reversible-write access for the migration window, then revoke it once the backfill of zone groups and every dependent report has been verified by the analyst” for an inbox item; you decided otherwise',
            sure: 'was 81% sure; it only advised',
            home: 'ch_93a1',
          }),
        ],
        details: {
          facts: [
            { label: 'Question', value: 'inbox.option@1' },
            { label: 'Decides alone when', value: 'never: it only advises' },
            { label: 'Must be right', value: '90% of the times it decides alone' },
            { label: 'Answers in all', value: "174, the measured models' included" },
          ],
          deciders: [
            model(JEV, true, { judgments: 58, outcomes: 31, unknown: 3, brier: 0.071, auc: 0.94, costUsd: 0.00031 }),
            model(LIQUID, false, { judgments: 58, outcomes: 31, unknown: 3, brier: 0.093, auc: 0.91, costUsd: 0.00061 }),
            model(PPLX, false, { judgments: 58, outcomes: 31, unknown: 3, brier: 0.102, auc: 0.89, costUsd: 0 }),
          ],
        },
      },
      {
        key: 'gate.touches_rail@1',
        title: 'Spotting changes to a rail',
        summary: 'Not settled: no level of certainty has proved right 95% of the time, so the path rule decides every time.',
        answeredBy: 'Answered by Jev 1.14. Liquid D1.2 Ultra answers beside it, to show whether it would do better.',
        figures: [
          figure('Right', RIGHT, '55%', '22 of 40 checked'),
          figure('Decides alone', alone('the path rule'), '0%', 'no certainty is safe yet'),
          figure('Cost', COST, '<$0.01', '44 checks'),
        ],
        mistakes: [mistake(7, { text: 'Said a change left the rails alone; the landing showed it touched one', sure: 'was only 55% sure, so the path rule decided', home: 'ch_93a1' })],
        details: {
          facts: [
            { label: 'Question', value: 'gate.touches_rail@1' },
            { label: 'Decides alone when', value: 'never: no level of certainty has proved right 95% of the time' },
            { label: 'Must be right', value: '95% of the times it decides alone' },
            { label: 'Answers in all', value: "88, the measured models' included" },
          ],
          deciders: [
            model(JEV, true, { judgments: 44, outcomes: 40, unknown: 0, brier: 0.24, auc: 0.55, costUsd: 0.00003 }),
            model(LIQUID, false, { judgments: 44, outcomes: 40, unknown: 0, brier: 0.26, auc: 0.5, costUsd: 0.00002 }),
          ],
        },
      },
      {
        key: 'card.off_brief@1',
        title: 'Keeping a running card on its brief',
        summary:
          "It only advises, and speaks to the card's Chair only when at least 96% sure; it has been right 84% of the 38 times we could check. The bar of 95% was set by the Assistant for you: “a false alarm costs the Chair a turn, and a missed one is caught by its review”.",
        answeredBy:
          'Answered by Liquid D1.2 Ultra, chosen by you: “it read our briefs best”. Jev 1.14 answers beside it, to show whether it would do better.',
        figures: [
          figure('Right', RIGHT, '84%', '32 of 38 checked'),
          figure('Decides alone', alone("the card's Chair"), 'Never', "it advises the card's Chair"),
          figure('Cost', COST, '<$0.01', '61 checks'),
        ],
        mistakes: [
          mistake(9, {
            text: 'Said a card had left its brief; the card showed it had not',
            sure: 'was 96% sure; it only advised',
            home: 'ch_93a1',
            card: 'ca_93a1c2d0',
          }),
        ],
        details: {
          facts: [
            { label: 'Question', value: 'card.off_brief@1' },
            { label: 'Speaks when', value: "it warns the card's Chair and is at least 96% sure" },
            { label: 'Must be right', value: '95% of the times it decides alone or speaks' },
            { label: 'Answers in all', value: "122, the measured models' included" },
            { label: 'Model chosen by', value: 'you, 2026-11-20: “it read our briefs best”' },
            {
              label: 'Bar set by',
              value: 'the Assistant for you, 2026-11-20: “a false alarm costs the Chair a turn, and a missed one is caught by its review”',
            },
          ],
          deciders: [
            model(LIQUID, true, { judgments: 61, outcomes: 38, unknown: 5, brier: 0.083, auc: 0.93, costUsd: 0.00041 }),
            model(JEV, false, { judgments: 61, outcomes: 38, unknown: 5, brier: 0.121, auc: 0.88, costUsd: 0.00061 }),
          ],
        },
      },
      {
        key: 'signal.screen@1',
        title: 'Screening incoming events',
        summary: 'Not settled yet: until 30 results are known it decides alone only when at least 90% sure.',
        answeredBy: 'Answered by Jev 1.14.',
        figures: [
          figure('Right', RIGHT, '—', '0 checked, too few to tell'),
          figure('Decides alone', alone("the subscription's rule"), '0%', '0 of 1'),
          figure('Cost', COST, '<$0.01', '1 check'),
        ],
        mistakes: [],
        details: {
          facts: [
            { label: 'Question', value: 'signal.screen@1' },
            { label: 'Decides alone when', value: 'at least 90% sure, until 30 results are known' },
            { label: 'Must be right', value: '90% of the times it decides alone' },
            { label: 'Answers in all', value: "1, the measured models' included" },
          ],
          deciders: [model(JEV, true, { judgments: 1, outcomes: 0, unknown: 0, costUsd: 0.00001 })],
        },
      },
    ],
  };
}
