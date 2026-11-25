// The readings and firings behind the line of the Signals page that is open, read from the server when it is opened (the board
// carries lines, not values). A line is read again when its version changes (the board says a reading came or it fired) and when
// the link is live again, since what happened meanwhile was not pushed. Holds what the server answered; decides nothing.
import { ref, shallowRef, watch, type Ref } from 'vue';
import { kindOf } from './act';
import { useApi } from './core/api';
import { session } from './session';
import type { SignalDetail } from './core/generated/types.generated';

export type DetailStatus = 'loading' | 'ready' | 'ended' | 'failed';

export function useSignalDetail(address: Ref<string | undefined>, version: Ref<string>) {
  const api = useApi();
  const detail = shallowRef<SignalDetail | null>(null);
  const status = ref<DetailStatus>('loading');
  // Reading lines one after another ends on the last one asked for: an earlier answer arriving late is dropped.
  let seq = 0;
  async function read(wanted: string | undefined, reopened: boolean) {
    const mine = ++seq;
    if (!wanted) return;
    if (reopened) {
      detail.value = null;
      status.value = 'loading';
    }
    try {
      const answer = await api.signalGet(wanted);
      if (mine !== seq) return;
      detail.value = answer;
      status.value = 'ready';
    } catch (e) {
      if (mine !== seq) return;
      detail.value = null;
      status.value = kindOf(e) === 'NOT_FOUND' ? 'ended' : 'failed';
    }
  }
  watch(address, (now) => void read(now, true), { immediate: true });
  // Live again after a reconnect: the snapshot is in, and what happened while the link was down was not pushed.
  watch([version, () => session.link.phase === 'live'], ([, live]) => live && void read(address.value, false));
  return { detail, status };
}
