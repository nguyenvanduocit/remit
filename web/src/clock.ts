// The clock that times on screen count against: the current time, once a second, from one timer however many read it.
import { createSharedComposable, useIntervalFn, useNow } from '@vueuse/core';

export const useClock = createSharedComposable(() => useNow({ scheduler: (tick) => useIntervalFn(tick, 1000) }));
