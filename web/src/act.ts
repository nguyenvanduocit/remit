// What the owner asks of the server, and how the screen says it went: a line of feedback (shown by the <Toaster> in
// Shell.vue) and the two shapes every action on the owner's behalf takes: a button (useAct) and a box of text that goes
// when sent (useSend). The calls are the core's (replica.ts api, provided under apiKey): it gives a changing call its
// request id, asks nothing while the link is down (the call fails with words that say so) and retries an action whose
// answer never came with the same request id, so the server does it once.
import { ref, type Ref } from 'vue';
import { toast as sonner } from 'vue-sonner';
import { useApi } from './core/api';
import type { Api } from './core/generated/api.generated';
import type { ErrorCode } from './core/generated/replica/types.generated';
import type { Call } from './core/client';
import { callIn, newPlace } from './replica';

export function toast(msg: string): void {
  sonner(msg);
}

// What went wrong, in words: a thrown Error's message (a call's failure is one, CallError), or a plain object's.
export const messageOf = (e: unknown) => (e instanceof Error ? e.message : ((e as { message?: string })?.message ?? String(e)));

// The codes a handler answers when the server did not do what it was asked; the rest the link says (no answer came,
// no session).
export type ErrorKind = Extract<ErrorCode, 'NOT_FOUND' | 'ENDED' | 'UNAVAILABLE' | 'FAILED'>;
const KINDS: readonly ErrorKind[] = ['NOT_FOUND', 'ENDED', 'UNAVAILABLE', 'FAILED'];

// Why the server did not do what it was asked, as the code of its answer; undefined for a failure that is not the
// server's answer (no answer came, the page's own error).
export function kindOf(e: unknown): ErrorKind | undefined {
  const code = (e as { code?: string } | null)?.code;
  return KINDS.includes(code as ErrorKind) ? (code as ErrorKind) : undefined;
}

// Runs a call and says how it went: `done` when it went, the error (after `failed`, if given) when it did not. The
// answer, or undefined when it failed.
export async function attempt<T>(call: () => Promise<T>, done?: string, failed?: string): Promise<T | undefined> {
  try {
    const answer = await call();
    if (done) toast(done);
    return answer;
  } catch (e) {
    toast(failed ? `${failed}: ${messageOf(e)}` : messageOf(e));
    return undefined;
  }
}

type ArgsOf<M extends keyof Api> = Parameters<Api[M]>;

// A button's action: the method and its arguments are the contract's, so a wrong name or argument does not compile.
export function useAct() {
  const api = useApi();
  return <M extends keyof Api>(method: M, args: ArgsOf<M>, done?: string) =>
    attempt(() => (api[method] as (...a: ArgsOf<M>) => Promise<Awaited<ReturnType<Api[M]>>>)(...args), done);
}

// A box of text that goes when sent: what was sent is cleared once it went, what did not go stays (and the owner is told
// why), and what was typed while it was on its way is kept. `send` answers what the server answered, or undefined when
// it did not go. Sending the same words again after an answer that never came is a retry of the same action (the core's
// Retry, by this box's place).
export function useSend(draft: Ref<string>, call: (text: string) => Call, failed = 'Could not send') {
  const sending = ref(false);
  const place = newPlace('send');
  async function send(): Promise<unknown> {
    const text = draft.value.trim();
    if (!text || sending.value) return undefined;
    sending.value = true;
    const answer = await attempt(() => callIn(place, text, call(text)), undefined, failed);
    sending.value = false;
    if (answer !== undefined && draft.value.trim() === text) draft.value = '';
    return answer;
  }
  return { sending, send };
}
