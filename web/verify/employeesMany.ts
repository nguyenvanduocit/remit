// The Crew page of employees (EmployeesView) on made-up data: a standing Chair with many reports (one of them a manager
// with reports of its own), a worker kept for one chain, a retired Chair whose active report keeps its place under it, a
// retired worker, a judge outside the Chair's tree, and the memory book of each (the Chair's holds every awkward case,
// see `memories` below); ?mem=none has none; ?mem=vi gives the Chair a book in Vietnamese at its longest (a 269-character
// title, a long Markdown body and evidence, 10 subjects, 8 links, a chain of 8 corrections, a 990-character teaching that wrote
// 9 memories); ?mem=big gives it 200 memories recalled now, 50 ended and 100 teachings. Each employee has its runs (the Chair
// many, one with a long title). ?rows=<n> adds n more workers under the Chair (0 by default; 500 is the size it is
// measured at); ?empty=1 has no employee and no mentor. The Mentors tab (?at=/mentors) holds four mentors with their standings:
// Lan in the app with two codes open, an open help and when she can help, Minh active 2h ago with three codes,
// Hoa (a long name) with no code used yet, and Tuan, removed. The Duels tab (?at=/duels) holds four kinds of work: engineering
// paused with its reason, a lineage of 12 duels beside one of 1; research going on; a long name with many lineages; writing with
// its only duel not judged yet; ?duels=none has none. No server, no sign-in: a fake client answers through the same sync as the real app.
// Open /verify/employeesMany.html?at=/employees/chair or ?at=/mentors/mt_lan. window.router is the router; window.paintMs the first frame's time.
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp } from 'vue';
import { createMemoryHistory } from 'vue-router';
import Shell from '../src/components/Shell.vue';
import { apiKey } from '../src/core/api';
import { api } from '../src/replica';
import { signedIn } from '../src/session';
import { appRouter } from '../src/router';
import { fakeServer, snapshotOf } from './fake';
import { ASSISTANT_CARD, assistantCard, assistantStanding, assistantView } from './assistantSeed';
import { capabilities } from './contextSeed';
import { agentsSeed } from './librarySeed';
import type { Ceiling, Employee, EmployeeRead, EmployeeRuns, LineageStanding, Memory, MemoryBook, Mentor, MentorStanding, Run, Teaching, WorkDuels } from '../src/core/generated/types.generated';
import { chainRead, chainRecord, employeeRecord, mentorRecord } from './records';
import '../src/theme.css';
import { markPaint } from './paint';
import { wordedRead } from './employeeSeed';

const params = new URLSearchParams(location.search);
const DAY = 86_400_000;
const iso = (ms: number) => new Date(ms).toISOString();
const now = Date.now();
const rows = Number(params.get('rows') ?? 0);
const empty = !!params.get('empty');

let numbered = 0;
const emp = (id: string, reportsTo: string, { ceiling, ...extra }: Partial<Omit<Employee, 'ceiling'>> & { ceiling?: Partial<Ceiling> } = {}): EmployeeRead => {
  numbered++;
  const e = employeeRecord({
    number: numbered,
    id,
    label: id,
    template: 'engineer',
    templateRev: 3,
    loadout: {
      instructions: 'Work in the area you answer for.',
      tools: ['Read', 'Grep'],
      skills: [],
    },
    title: 'worker',
    reportsTo,
    hiredBy: reportsTo,
    status: 'active',
    ceiling: ceiling ?? { effect: 'reversible', tools: ['Read', 'Grep', 'Edit'] },
    createdAt: iso(now - 12 * DAY),
    rev: 1,
    ...extra,
  });
  return wordedRead(e, { shape: 'circle', hue: (numbered * 47) % 360, accessory: 'halo' });
};
const employees: EmployeeRead[] = empty
  ? []
  : [
      emp('chair', 'owner', {
        label: 'Login Chair',
        template: 'chair',
        title: 'chair',
        scope: 'user login',
        strengths: 'tracing login failures from the session logs to the handler that threw them, and keeping a change to the token refresh small enough to review in one sitting',
        ceiling: {
          effect: 'irreversible',
          grants: [{ source: 'github', connection: 'default', effect: 'reversible' }],
          tools: ['Read', 'Grep', 'Edit', 'Bash'],
        },
        createdAt: iso(now - 30 * DAY),
      }),
      emp('judge', 'owner', {
        label: 'Login Judge',
        template: 'judge',
        title: 'judge',
        scope: 'checks the login objective',
        ceiling: { effect: 'read', tools: ['Read', 'Grep'] },
        createdAt: iso(now - 29 * DAY),
      }),
      emp('oldchair', 'owner', {
        label: 'Signup Chair (left in March, kept for its memory)',
        template: 'chair',
        title: 'chair',
        status: 'retired',
        scope: 'signup flow',
        createdAt: iso(now - 90 * DAY),
      }),
      emp('signup-eng', 'oldchair', {
        label: 'Signup engineer',
        scope: 'signup form',
      }),
      emp('lead', 'chair', {
        label: 'Session lead',
        scope: 'sessions and tokens',
      }),
      // An agent that has not named itself: it reads as its code and its template.
      emp('chainer', 'chair', {
        label: '',
        scope: 'the redirect bug',
      }),
      emp('retired-w', 'chair', {
        label: 'Retired reviewer',
        template: 'reviewer',
        status: 'retired',
        scope: 'reviews login MRs',
      }),
      ...Array.from({ length: 12 }, (_, i) =>
        emp(`tok${i}`, 'lead', {
          label: `Token worker ${i + 1}`,
          template: ['engineer', 'qc', 'reviewer', 'research'][i % 4],
          scope: `token ${['refresh', 'rotation', 'expiry', 'storage'][i % 4]}`,
        }),
      ),
      ...Array.from({ length: rows }, (_, i) =>
        emp(`m${i}`, i < 7 ? 'chair' : `m${i % 7}`, {
          label: `Worker ${i}: ${['reads the logs', 'fixes the form', 'checks the API', 'writes the tests'][i % 4]}`,
          status: i % 5 === 4 ? 'retired' : 'active',
          scope: `part ${i}`,
        }),
      ),
    ];

const HOUR = 3_600_000;
const code = (id: string, note: string, open: boolean, made: number, used?: number) => ({
  id,
  note,
  open,
  state: open ? 'open' : 'closed',
  createdAt: iso(now - made),
  lastUsedAt: used === undefined ? undefined : iso(now - used),
});
const mentor = (id: string, label: string, made: number, extra: Partial<Mentor> = {}): Mentor => mentorRecord({ id, label, status: 'active', rev: 1, createdAt: iso(now - made), ...extra });
const mentors: Mentor[] = empty
  ? []
  : [
      mentor('mt_lan', 'Lan', 7 * DAY, {
        strengths: 'visual design, accessible colour and type',
        availability: { note: 'away Friday, back Monday morning; reachable on Slack for anything urgent', at: iso(now - 3 * HOUR) },
      }),
      mentor('mt_minh', 'Minh', 21 * DAY, { strengths: 'Go concurrency, the payments API' }),
      mentor('mt_hoa', 'Hoa Tran, contractor for the password reset flow and its emails', 2 * DAY),
      mentor('mt_tuan', 'Tuan', 40 * DAY, { strengths: 'the old signup API', status: 'removed', rev: 3 }),
    ];
const mentorStandings: MentorStanding[] = empty
  ? []
  : [
      {
        mentor: 'mt_lan',
        inAppNow: true,
        lastActive: iso(now - 60_000),
        presence: 'in the app now',
        openHelps: 1,
        rev: 1,
        keys: [
          code('wkey_a', "Lan's phone", true, 6 * DAY, 60_000),
          code('wkey_b', "Lan's MacBook Pro at the office, the one on the standing desk by the window", true, 2 * DAY, 5 * 60_000),
        ],
      },
      {
        mentor: 'mt_minh',
        inAppNow: false,
        lastActive: iso(now - 2 * HOUR),
        presence: 'active 2h ago',
        openHelps: 0,
        rev: 1,
        keys: [code('wkey_c', "Minh's desktop", false, 9 * DAY, 2 * HOUR), code('wkey_d', '', false, 20 * DAY, 4 * DAY), code('wkey_e', "Minh's new phone", false, HOUR)],
      },
      {
        mentor: 'mt_hoa',
        inAppNow: false,
        presence: 'no code used yet',
        keys: [code('wkey_f', "Hoa's laptop", false, 30 * 60_000)],
        openHelps: 0,
        rev: 1,
      },
      {
        mentor: 'mt_tuan',
        inAppNow: false,
        lastActive: iso(now - 12 * DAY),
        presence: 'active 12d ago',
        keys: [],
        openHelps: 0,
        rev: 1,
      },
    ];

// Each agent's memory book (ADR-0117): what it recalls now, latest changed first, then what it stopped recalling, latest
// ended first, and the teachings its memory learned from, latest first, as the server orders them. The Chair's holds the
// awkward cases: a decision with a long Markdown body, a confirmed preference, every scope and kind, many subjects, links
// (one to a memory outside the book), a memory with no title, one with nothing to break on, a ticket's raw HTML, a chain of
// corrections (a superseded by b superseded by c), a retracted one with why, and 30 more; its teachings are waiting and
// settled, one of each role, one that wrote nothing.
let nextMemory = 1;
const remembered = (holder: string, title: string, ago: number, extra: Partial<Memory> = {}): Memory => ({
  id: `m_${nextMemory++}`,
  holder,
  template: 'chair',
  project: 'auth-service',
  kind: 'lesson',
  scope: 'own',
  trust: 'auto',
  status: 'active',
  title,
  body: '',
  evidence: '',
  work: 'Fix the login redirect',
  chain: 'ch_login',
  card: 'c_204',
  source: 'card:c_204 accepted',
  sourceLabel: 'an accepted card: Engineer · login redirect',
  teacher: '',
  about: [],
  links: [],
  supersededBy: '',
  retaught: [],
  recalled: 0,
  lastRecalledAt: null,
  helped: 0,
  misled: 0,
  createdAt: iso(now - ago),
  updatedAt: iso(now - ago),
  endedAt: null,
  endedWhy: '',
  ...extra,
});
let nextTeaching = 1;
const teaching = (holder: string, ago: number, extra: Partial<Teaching> = {}): Teaching => ({
  id: `t_${nextTeaching++}`,
  holder,
  template: 'chair',
  project: 'auth-service',
  chain: 'ch_login',
  card: 'c_204',
  role: 'accepted',
  fromEvent: 0,
  toEvent: 0,
  text: '',
  teacher: '',
  source: 'card:c_204 accepted',
  sourceLabel: 'an accepted card: Engineer · login redirect',
  state: 'settled',
  written: [],
  note: '',
  createdAt: iso(now - ago),
  settledAt: iso(now - ago + 4 * 60_000),
  ...extra,
});

const session = remembered('chair', 'Sessions are stored server side', 20 * 60_000, {
  kind: 'decision',
  scope: 'project',
  body: '# Sessions are stored server side\n\nThe cookie holds only an **id**; the session row holds the rest.\n\n**Why:** a signed token cannot be revoked before it expires, and the owner wants a stolen laptop cut off at once.\n\n**How to apply:**\n\n- never put the user id in the cookie\n- the refresh job is `internal/auth/refresh.go`, every 15 minutes\n\n```go\nfunc (s *Store) Revoke(ctx context.Context, id string) error {\n\treturn s.db.ExecContext(ctx, "DELETE FROM sessions WHERE id = ?", id)\n}\n```\n',
  evidence: 'the owner: "a stolen laptop has to be cut off at once"',
  source: 'card:c_1 steered by the owner (event 9)',
  teacher: 'the owner',
  about: ['file:internal/auth/session.go', 'file:internal/auth/refresh.go', 'topic:sessions', 'crate:store', 'repo:git.example.com/acme/auth-service', 'person:the owner'],
  recalled: 41,
  lastRecalledAt: iso(now - 6 * 60_000),
  helped: 9,
  retaught: ['t_3', 't_7'],
});
// A chain of corrections: the first said one thing, a judge corrected it, the owner corrected that.
const slashC = remembered('chair', 'Redirects keep the path exactly as the user typed it', 2 * HOUR, {
  kind: 'lesson',
  scope: 'lineage',
  trust: 'confirmed',
  body: 'Match the path as typed, trailing slash included; the router normalises nothing.',
  source: 'card:c_88 steered by the owner (event 412)',
  teacher: 'the owner',
  about: ['file:web/src/router.ts', 'topic:redirects'],
  recalled: 3,
  lastRecalledAt: iso(now - HOUR),
  helped: 2,
});
const slashB = remembered('chair', 'Strip the trailing slash before matching a redirect', 3 * DAY, {
  kind: 'lesson',
  scope: 'lineage',
  body: 'Normalise the path by stripping a trailing slash, then match.',
  source: 'card:c_71 sent back',
  teacher: 'judge E-4',
  about: ['file:web/src/router.ts', 'topic:redirects'],
  status: 'superseded',
  supersededBy: slashC.id,
  recalled: 12,
  lastRecalledAt: iso(now - 3 * HOUR),
  helped: 4,
  misled: 2,
  endedAt: iso(now - 2 * HOUR),
  endedWhy: 'the owner said the router must not rewrite a path the user typed; two agents said it misled them on the docs site',
});
const slashA = remembered('chair', 'A redirect loop hides behind a trailing slash', 9 * DAY, {
  kind: 'pitfall',
  scope: 'lineage',
  body: 'Check both forms of the path.',
  about: ['topic:redirects'],
  status: 'superseded',
  supersededBy: slashB.id,
  recalled: 7,
  lastRecalledAt: iso(now - 4 * DAY),
  helped: 1,
  endedAt: iso(now - 3 * DAY),
  endedWhy: 'a judge found the loop came from matching both forms, not from missing one',
});
slashC.links = [{ kind: 'supersedes', to: slashB.id }, { kind: 'because', to: session.id }];
slashB.links = [{ kind: 'supersedes', to: slashA.id }];
const retracted = remembered('chair', 'Ship pricing changes without asking', 5 * DAY, {
  kind: 'decision',
  scope: 'everyone',
  body: 'The Assistant approved a price change on its own; the memory said such changes need no one.',
  source: 'decision:d_9 overturned by decision:d_12',
  teacher: 'the owner',
  about: ['topic:pricing'],
  recalled: 2,
  lastRecalledAt: iso(now - 5 * DAY),
  misled: 1,
  status: 'retracted',
  endedAt: iso(now - 4 * DAY),
  endedWhy: 'the owner overturned the decision that taught it: a price is his to set',
});
const returnAddress = remembered('chair', 'Ask for the return address first', 3 * HOUR, {
  kind: 'preference',
  trust: 'confirmed',
  body: 'Before changing the redirect, ask which return address the user came from.',
  evidence: 'the reviewer, after the trailing-slash loop',
  teacher: 'Lan',
  source: 'card:c_88 steered by Lan (event 377)',
  about: ['topic:redirects', 'person:Lan'],
  links: [{ kind: 'related', to: slashC.id }, { kind: 'refines', to: 'm_elsewhere' }, { kind: 'contradicts', to: session.id }],
  recalled: 1,
  lastRecalledAt: iso(now - 2 * HOUR),
});
const memories: Record<string, Memory[]> = {
  chair: [
    session,
    slashC,
    returnAddress,
    remembered('chair', 'Reports lead with the customer', 2 * DAY, { kind: 'principle', scope: 'everyone', trust: 'confirmed', teacher: 'the owner', body: 'Short reports, the result first.', recalled: 120, lastRecalledAt: iso(now - 30_000), helped: 31, misled: 1 }),
    remembered('chair', 'Every login change ships behind the canary flag', 2 * DAY, { kind: 'intent', scope: 'project', body: 'Until the new login has held a week at 100%.', about: ['tool:flagd', 'topic:canary'] }),
    remembered('chair', '', 6 * DAY, { kind: 'fact', body: 'A memory with no title: just a line the librarian kept, `cargo fmt` runs before every commit.', about: ['tool:cargo fmt'] }),
    remembered('chair', 'A very long title with nothing to break on: ' + 'y'.repeat(120), 8 * DAY, {
      body: 'z'.repeat(400) + '\n\n' + Array.from({ length: 40 }, (_, i) => `- line ${i + 1} of a long memory`).join('\n'),
      evidence: 'x'.repeat(300),
      about: ['file:crates/store/src/' + 'very_long_module_name_'.repeat(5) + 'lib.rs', 'topic:golden files', 'tool:cargo nextest', 'crate:store', 'crate:wire', 'crate:replica', 'topic:migrations', 'topic:a subject with many words in it to wrap'],
    }),
    remembered('chair', 'A support ticket', 9 * DAY, { kind: 'fact', body: 'From a ticket: <script>alert(1)</script><img src=x onerror=alert(1)> <b>bold from outside</b>' }),
    ...Array.from({ length: 30 }, (_, i) =>
      remembered('chair', `Another thing the Chair remembers (${i})`, (10 + i) * DAY, {
        kind: (['lesson', 'pitfall', 'preference', 'fact'] as const)[i % 4],
        scope: (['own', 'project', 'lineage', 'everyone'] as const)[i % 4],
        body: ['why the token expires after 15 minutes', 'where the refresh job runs', 'what broke on the last release'][i % 3],
        recalled: i % 5 ? i * 3 : 0,
        lastRecalledAt: i % 5 ? iso(now - i * DAY) : null,
      }),
    ),
    slashB,
    retracted,
    slashA,
  ],
  judge: [remembered('judge', 'Logout did not clear the cookie', DAY, { kind: 'pitfall', template: 'judge', body: 'A defect the Chair had missed.' })],
  // what the Assistant's card remembers: the owner's words to it and his overturns
  [ASSISTANT_CARD]: [
    remembered('assistant', 'Merge on green, never wait for me', 2 * HOUR, { kind: 'preference', template: '', teacher: 'the owner', source: 'card:c_as steered by the owner (event 13091)', body: 'He wants approved changes landed as soon as their checks pass.' }),
    remembered('assistant', 'A price change is his call', 3 * DAY, { kind: 'principle', template: '', teacher: 'the owner', source: 'decision:d_9 overturned by decision:d_12', trust: 'confirmed', body: 'The Assistant applied a price change; he overturned it: prices are his.' }),
  ],
};
const teachings: Record<string, Teaching[]> = {
  chair: [
    teaching('chair', 3 * 60_000, { state: 'waiting', settledAt: null, card: 'c_301', source: 'card:c_301 accepted' }),
    teaching('chair', 20 * 60_000, { role: 'teaching', teacher: 'the owner', card: 'c_1', source: 'card:c_1 steered by the owner (event 9)', text: 'Keep sessions on the server: a stolen laptop has to be cut off at once, and a signed token cannot be revoked before it expires.', written: [session.id], note: 'Wrote one decision; the rest of his words repeated what the Chair already held.' }),
    teaching('chair', 2 * HOUR, { role: 'teaching', teacher: 'the owner', card: 'c_88', source: 'card:c_88 steered by the owner (event 412)', text: 'No. The router must not rewrite a path the user typed. Match it as typed.\n\nAnd tell the docs team.', written: [slashC.id], note: 'Superseded "Strip the trailing slash before matching a redirect" with what he said.' }),
    teaching('chair', 6 * HOUR, { role: 'review', card: '', chain: '', source: 'review:duplicates', note: 'Merged two near duplicates about the refresh job into one; nothing else to settle.', written: [session.id] }),
    teaching('chair', DAY, { card: 'c_250', source: 'card:c_250 accepted', note: 'Nothing new: the session confirmed what "Reports lead with the customer" already says.' }),
    teaching('chair', 3 * DAY, { role: 'teaching', teacher: 'judge E-4', card: 'c_71', source: 'card:c_71 sent back', text: 'x'.repeat(900), written: [slashB.id, 'm_elsewhere'], note: 'Wrote one lesson and retaught one held by the lineage.' }),
    teaching('chair', 4 * DAY, { role: 'review', card: '', chain: '', source: 'review:misled', note: 'Retracted nothing: the memory that misled was already superseded.' }),
  ],
};
// What each mentor taught, as their own memory keeps it.
const taught: Record<string, Memory[]> = {
  mt_lan: [
    remembered('mt_lan', 'Contrast before colour', 5 * HOUR, { kind: 'principle', template: '', scope: 'everyone', teacher: 'Lan', source: 'card:c_17 event:204', body: 'Check text contrast against the tokens before choosing a hue: 4.5:1 for text.', about: ['topic:contrast', 'topic:tokens'], recalled: 14, lastRecalledAt: iso(now - HOUR), helped: 5 }),
    remembered('mt_lan', 'One accent per screen', 2 * DAY, { kind: 'preference', template: '', scope: 'everyone', teacher: 'Lan', source: 'card:c_31 event:390', body: 'A screen carries one accent colour; the rest is neutral.' }),
  ],
  oldchair: [remembered('oldchair', 'Signup validates on blur', 80 * DAY, { kind: 'fact', body: 'The signup form validates on blur, not on submit.' })],
};
// What each mentor's own memory learned from: their marks on the replies of the agents they helped.
const taughtFrom: Record<string, Teaching[]> = {
  mt_lan: [
    teaching('mt_lan', 5 * HOUR, { holder: 'mt_lan', template: '', role: 'teaching', teacher: 'Lan', card: 'c_17', source: 'card:c_17 event:204', text: 'This grey on grey is 2.9:1. Check text against the tokens before you pick a hue: 4.5:1 for text, 3:1 for the parts of a control.', written: [taught.mt_lan![0]!.id], note: 'Wrote one principle.' }),
    teaching('mt_lan', 2 * DAY, { holder: 'mt_lan', template: '', role: 'teaching', teacher: 'Lan', card: 'c_31', source: 'card:c_31 event:390', text: 'Too many accents: keep one.', written: [taught.mt_lan![1]!.id], note: 'Wrote one preference.' }),
  ],
};

// ?mem=vi: the Chair's book in Vietnamese, every part at its longest.
const viBook = (): MemoryBook => {
  const chain = Array.from({ length: 8 }, (_, i) =>
    remembered('chair', `Phiên bản ${8 - i} của quy tắc chuyển hướng đường dẫn khi người dùng gõ dấu gạch chéo ở cuối`, (i + 1) * DAY, {
      status: i === 0 ? 'active' : 'superseded',
      endedAt: i ? iso(now - i * DAY + HOUR) : null,
      endedWhy: i ? `Lần sửa thứ ${i}: giám khảo E-${i} thấy quy tắc cũ làm sai trang tài liệu và hai tác tử khác bị dẫn sai` : '',
      about: ['topic:chuyển hướng'],
      source: 'card:c_9123 được chủ sở hữu điều chỉnh (sự kiện 18234)',
    }),
  );
  for (let i = 1; i < 8; i++) chain[i]!.supersededBy = chain[i - 1]!.id;
  const vi = remembered('chair', 'Khi khách hàng doanh nghiệp yêu cầu xuất hóa đơn điện tử theo Nghị định 123/2020/NĐ-CP, luôn kiểm tra mã số thuế, địa chỉ đăng ký kinh doanh và thông tin người mua trước khi phát hành, vì hóa đơn đã ký số không thể sửa mà chỉ có thể lập hóa đơn điều chỉnh hoặc thay thế', 30 * 60_000, {
    kind: 'pitfall',
    scope: 'project',
    trust: 'confirmed',
    teacher: 'Nguyễn Thị Phương Thảo (kế toán trưởng)',
    source: 'card:c_9123 được chủ sở hữu điều chỉnh (sự kiện 18234)',
    body: '## Vì sao\n\nHóa đơn điện tử đã ký số **không thể sửa**. Nếu sai mã số thuế, phải lập hóa đơn điều chỉnh, gửi thông báo sai sót theo mẫu 04/SS-HĐĐT cho cơ quan thuế, và giải thích với khách hàng. Việc này mất ba đến năm ngày làm việc và khách hàng doanh nghiệp thường rất phiền lòng vì kỳ quyết toán thuế của họ bị ảnh hưởng.\n\n## Cách áp dụng\n\n- tra mã số thuế trên cổng tracuunnt.gdt.gov.vn trước khi phát hành\n- so địa chỉ với giấy phép đăng ký kinh doanh mới nhất\n- nếu khách hàng đổi tên công ty trong năm, hỏi lại tên pháp lý chính xác',
    evidence: 'Chị Thảo: "Tháng trước mình phải lập lại mười bảy hóa đơn cho Công ty Cổ phần Thương mại Dịch vụ Xuất nhập khẩu Hoàng Gia Phát chỉ vì sai một chữ số trong mã số thuế, khách hàng gọi điện phàn nàn suốt một tuần và kế toán bên họ phải làm lại toàn bộ tờ khai thuế giá trị gia tăng của quý"',
    about: ['topic:hóa đơn điện tử', 'topic:mã số thuế', 'topic:Nghị định 123/2020/NĐ-CP', 'person:Nguyễn Thị Phương Thảo', 'file:services/invoice/einvoice/issue_and_sign_with_provider.go', 'file:services/invoice/einvoice/adjustment.go', 'tool:viettel-sinvoice-cli', 'crate:invoice', 'repo:git.example.com/acme/billing-platform-vietnam', 'topic:khách hàng doanh nghiệp'],
    links: [...chain.slice(0, 3).map((c) => ({ kind: 'related' as const, to: c.id })), { kind: 'because', to: 'm_ngoai_1' }, { kind: 'refines', to: chain[0]!.id }, { kind: 'contradicts', to: chain[1]!.id }, { kind: 'supersedes', to: chain[2]!.id }, { kind: 'related', to: 'm_ngoai_2' }],
    recalled: 1234,
    lastRecalledAt: iso(now - 40_000),
    helped: 987,
    misled: 12,
  });
  const said = teaching('chair', 60_000, { role: 'teaching', chain: 'ch_invoice', card: 'c_9123', teacher: 'Nguyễn Thị Phương Thảo (kế toán trưởng)', source: vi.source, text: 'Hóa đơn đã ký số không sửa được. '.repeat(30), note: 'Đã viết một cạm bẫy và sửa lại bảy phiên bản của quy tắc chuyển hướng.', written: [vi.id, ...chain.map((c) => c.id)] });
  const again = Array.from({ length: 23 }, (_, i) => teaching('chair', (i + 2) * HOUR, { role: (['accepted', 'teaching', 'review'] as const)[i % 3], chain: 'ch_invoice', card: `c_${9200 + i}`, source: `card:c_${9200 + i} accepted`, written: [vi.id], note: `Dạy lại lần ${i + 1}.` }));
  vi.retaught = again.map((t) => t.id);
  return { memories: [vi, chain[0]!, ...chain.slice(1)], teachings: [said, ...again] };
};
// ?mem=big: the Chair's book at size.
const bigBook = (): MemoryBook => {
  const kinds = ['lesson', 'pitfall', 'preference', 'principle', 'decision', 'intent', 'fact'] as const;
  const scopes = ['own', 'project', 'lineage', 'everyone'] as const;
  const now_ = Array.from({ length: 200 }, (_, i) =>
    remembered('chair', `Memory number ${i} about the login flow and its tokens`, (i + 1) * HOUR, { kind: kinds[i % 7], scope: scopes[i % 4], body: `A body line ${i}`, source: `card:c_${i} accepted`, about: [`topic:t${i % 9}`, `file:src/f${i}.ts`], recalled: i, lastRecalledAt: iso(now - i * 60_000), helped: i % 6, misled: i % 11 === 0 ? 1 : 0 }),
  );
  const ended_ = Array.from({ length: 50 }, (_, i) => remembered('chair', `Ended memory ${i}`, (i + 300) * HOUR, { status: i % 2 ? 'retracted' : 'superseded', supersededBy: i % 2 ? '' : now_[i]!.id, endedAt: iso(now - i * HOUR), endedWhy: `why ${i}` }));
  const taught_ = Array.from({ length: 100 }, (_, i) => teaching('chair', i * HOUR, { role: (['accepted', 'teaching', 'review'] as const)[i % 3], teacher: i % 3 === 1 ? 'the owner' : '', text: i % 3 === 1 ? `Said thing ${i}` : '', source: `card:c_${i} accepted`, written: [now_[i]!.id], note: `note ${i}` }));
  return { memories: [...now_, ...ended_], teachings: taught_ };
};
const mem = params.get('mem');
const bookOf = (id: string, held: Memory[] | undefined, taughtIn: Teaching[] | undefined): MemoryBook =>
  structuredClone(
    mem === 'none' ? { memories: [], teachings: [] } : id === 'chair' && mem === 'vi' ? viBook() : id === 'chair' && mem === 'big' ? bigBook() : { memories: held ?? [], teachings: taughtIn ?? [] },
  );

// Each employee's runs across chains, newest first: the Chair's are many, of every outcome, one waiting, one running, one
// with a long chain title and a long brief.
const run = (chainId: string, chainTitle: string, ago: number, extra: Partial<Run> = {}): Run => ({ chainId, chainTitle, cardId: `c_${chainId}`, request: 'Fix it and say what changed.', outcome: 'accepted', source: 'chair', waiting: false, live: false, started: iso(now - ago), ended: iso(now - ago + 40 * 60_000), ...extra });
const runsOf = (id: string): EmployeeRuns => {
  const runs: Run[] =
    id === 'chair'
      ? [
          run('ch_login', 'Fix the login redirect loop for users with a trailing slash', 20 * 60_000, { outcome: 'open', live: true, ended: undefined, source: 'owner' }),
          run('ch_invoice', 'Kiểm tra mã số thuế trước khi phát hành hóa đơn điện tử theo Nghị định 123/2020/NĐ-CP cho mọi khách hàng doanh nghiệp', 3 * HOUR, { outcome: 'sent back', waiting: true, request: 'Một yêu cầu rất dài: '.repeat(12) }),
          run('ch_tokens', 'Rotate the refresh tokens', DAY, { outcome: 'failed' }),
          run('ch_docs', 'Docs for the session store', 3 * DAY),
          run('ch_old', 'Stop the old signup API', 9 * DAY, { outcome: 'stopped' }),
        ]
      : [];
  return { employee: id, runs, more: false, failures: runs.filter((r) => r.outcome === 'failed').length, waiting: runs.filter((r) => r.waiting).length };
};

const pausedWhy =
  "duels of engineering are paused: the owner overturned 4 of the judge's last 10 picks there, so it agrees with him on 60%, under the 70% duels need. They go on when the coach sets a lower share";
// Each lineage as the server stands it; the Duels tab reads its duel rating.
const rated = (template: string, work: string, rating: number, duels: number, won: number): LineageStanding => ({ template, work, score: 0, held: 0, corrected: 0, failed: 0, settled: false, duel: { template, work, rating, duels, won }, evidence: [] });
const duelStandings: WorkDuels[] = [
  { work: 'engineering', standings: [rated('builder', 'engineering', 1688, 12, 9), rated('engineer', 'engineering', 1561, 1, 1), rated('reviewer', 'engineering', 1402, 11, 2)], judged: 10, overturned: 4, paused: pausedWhy },
  { work: 'research', standings: [rated('research', 'research', 1530, 3, 2), rated('analyst-with-a-very-long-template-name-that-has-nothing-to-break-on', 'research', 1470, 3, 1)], judged: 3, overturned: 0 },
  {
    work: 'customer-support-and-onboarding-for-the-enterprise-tier',
    standings: Array.from({ length: 14 }, (_, i) => rated(['support', 'writer', 'qc', 'engineer', 'builder', 'reviewer', 'research'][i % 7] + (i > 6 ? '-2' : ''), 'x', 1700 - i * 25, 2 + i, 1 + (i % 3))),
    judged: 9,
    overturned: 1,
  },
  { work: 'writing', standings: [], judged: 0, overturned: 0 },
];

const fake = fakeServer({
  appSnapshot: async () =>
    structuredClone(snapshotOf({
      objectives: [],
      employees,
      mentors,
      mentorStandings,
      chains: [chainRead(chainRecord({ id: 'ch_login', title: 'Fix the login redirect loop for users with a trailing slash', status: 'running', createdAt: iso(now - 3 * DAY), updatedAt: iso(now - HOUR) }))],
      inbox: [],
      playbooks: [],
      capabilities,
      standings: [],
      workload: {
        running: 0,
        capacity: 4, pressure: '',
        queued: 0,
        waiting: 0,
        rev: 1,
      },
      insights: [],
      agents: agentsSeed(),
      skills: [],
      sources: [],
      events: [],
      eventSignals: [],
      sensors: [],
      clientKeys: [],
      feedback: [],
      assistant: {
        ...assistantView(now, assistantCard(now, { status: 'idle', queued: 0, activity: '' }), assistantStanding('ready')),
        drafts: [],
      },
    })),
  duelStandings: async () => structuredClone(params.get('duels') === 'none' ? [] : duelStandings),
  employeeMemory: async (id) => bookOf(id, memories[id], teachings[id]),
  cardMemory: async (id) => bookOf(id, memories[id], teachings[id]),
  mentorMemory: async (id) => bookOf(id, taught[id], taughtFrom[id]),
  employeeRuns: async (id) => structuredClone(runsOf(id)),
  cardEvents: async () => ({ events: [], truncated: false }),
  draftSave: async (d) => ({ ...d, updatedAt: iso(Date.now()), rev: 1 }) as never,
  workerKey: async (id) => ({
    key: `wk_${id}_Zx81mQ4tP0vB7nLs2Rk9yW3cA6eH5uJd`,
  }),
});
signedIn();

const router = appRouter(createMemoryHistory());
await router.push(params.get('at') ?? '/employees');
createApp(Shell).use(router).provide(apiKey, api).mount('#app');
markPaint();
Object.assign(window, { router, push: fake.push, duelStandings });
