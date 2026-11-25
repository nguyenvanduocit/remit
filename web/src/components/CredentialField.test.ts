// @vitest-environment happy-dom
import { afterEach, describe, expect, test, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { toast } from 'vue-sonner';
import CredentialField from './CredentialField.vue';
import { apiKey } from '../core/api';
import { api } from '../replica';
import { fakeServer, snapshotOf } from '../../verify/fakeNode';
import { inboxRecord } from '../../verify/records';
import type { Ack } from '../core/generated/types.generated';

vi.mock('vue-sonner', () => ({ toast: vi.fn() }));

const SECRET = 'ghp_s3cr3t-value-0123';
const item = inboxRecord({ id: 'i7', home: 'author', cardId: 'author-chair', kind: 'credential', title: 'Credential for github/default: GITHUB_TOKEN', status: 'open', source: 'github', connection: 'default', variable: 'GITHUB_TOKEN', secret: true, createdAt: '' });

// The field of the item, over a server whose answer to inboxAnswer the test settles when it chooses.
function field(answer: () => Promise<Ack>) {
  const calls: unknown[][] = [];
  fakeServer({
    appSnapshot: () => snapshotOf({}),
    inboxAnswer: async (...args) => (calls.push(args), answer()),
  });
  const w = mount(CredentialField, { props: { item }, attachTo: document.body, global: { provide: { [apiKey as symbol]: api } } });
  const input = () => w.get('input').element as HTMLInputElement;
  const type = async (text: string) => {
    input().value = text;
    await w.get('input').trigger('input');
  };
  const save = () => w.get('button');
  return { w, calls, input, type, save };
}
const toasts = () => vi.mocked(toast).mock.calls.map((c) => String(c[0]));

afterEach(() => {
  document.body.innerHTML = '';
  vi.mocked(toast).mockClear();
});

describe('CredentialField', () => {
  test('a refusal is said without the value, even when the server quotes it, and the box stays empty', async () => {
    const f = field(async () => Promise.reject({ message: `the value ${SECRET} is not accepted for GITHUB_TOKEN`, code: 'INVALID_ARGUMENT' }));
    await f.type(SECRET);
    await f.save().trigger('click');
    await flushPromises();
    expect(toasts()).toHaveLength(1);
    expect(toasts()[0]).toContain('Could not save the credential');
    expect(toasts()[0]).toContain('is not accepted for GITHUB_TOKEN');
    expect(toasts()[0]).not.toContain(SECRET);
    expect(f.input().value).toBe('');
    expect(f.w.html()).not.toContain(SECRET);
    f.w.unmount();
  });
});
