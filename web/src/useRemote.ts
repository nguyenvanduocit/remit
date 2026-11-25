// Something the screen asks the server for that no push keeps current (a chain's details on the board, a landing's diff,
// the merge queues): asked again whenever what it is asked with changes, and an answer that comes back for an older request
// dropped. `of` says what the held `data` was asked with: an older answer stays until a newer one comes, so the
// caller decides whether it still fits. `retry` asks again with the same args, after a failure.
import { ref, shallowRef, watch, type Ref } from 'vue';

export interface Remote<A, T> {
  data: Ref<T | undefined>;
  of: Ref<A | undefined>;
  error: Ref<string>;
  loading: Ref<boolean>;
  retry: () => void;
}

// `args` holds plain values (it is compared by its JSON): when any of them changes, the server is asked again. Nothing
// is asked while it is undefined.
export function useRemote<A, T>(args: () => A | undefined, load: (args: A) => Promise<T>): Remote<A, T> {
  const data = shallowRef<T>();
  const of = shallowRef<A>();
  const error = ref('');
  const loading = ref(false);
  // Bumped by retry, so the same args are asked again.
  const tries = ref(0);
  watch(
    () => {
      const a = args();
      return a === undefined ? undefined : [JSON.stringify(a), tries.value] as const;
    },
    (key, _, onCleanup) => {
      if (key === undefined) return void (loading.value = false);
      const asked = JSON.parse(key[0]) as A;
      let dropped = false;
      onCleanup(() => (dropped = true));
      loading.value = true;
      error.value = '';
      load(asked).then(
        (answer) => {
          if (dropped) return;
          data.value = answer;
          of.value = asked;
          error.value = '';
          loading.value = false;
        },
        (e) => {
          if (dropped) return;
          error.value = e instanceof Error ? e.message : String(e?.message ?? e);
          loading.value = false;
        },
      );
    },
    { immediate: true },
  );
  return { data, of, error, loading, retry: () => void tries.value++ };
}
