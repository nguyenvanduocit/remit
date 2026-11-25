import { inject, type InjectionKey } from 'vue';

// The Shell gives each page its one h1, the name the browser tab shows (Shell.vue), so a header a page draws for what it is about
// (PageHead, EmptyPage) is a heading beneath it. Drawn by itself, as a verify page does, the header is the page's h1.
export const pageNamed: InjectionKey<true> = Symbol('pageNamed');
export const useHeadingTag = (): 'h1' | 'h2' => (inject(pageNamed, false) ? 'h2' : 'h1');
