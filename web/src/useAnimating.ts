// Whether a looping animation on `el` is worth running: the element is on screen and the page is shown. Off screen (a
// card scrolled out of its column) or in a hidden tab it pauses, so the compositor has nothing to draw for it.
import { computed, type Ref } from 'vue';
import { useDocumentVisibility, useElementVisibility } from '@vueuse/core';

export function useAnimating(el: Ref<HTMLElement | null | undefined>) {
  const seen = useElementVisibility(el);
  const page = useDocumentVisibility();
  return computed(() => (seen.value && page.value === 'visible' ? 'running' : 'paused'));
}
