<script setup lang="ts">
import { computed } from 'vue';
import { useRouter } from 'vue-router';
import { Marked } from 'marked';
import DOMPurify from 'dompurify';

// Agent text is untrusted: rendered as Markdown, then sanitised before it reaches the DOM. `type` sets its type: `body` for
// what an agent says, `quiet` for a brief or a goal quoted in a panel. Without one it takes the type around it, and the
// parent styles it (a folded prompt, a thought). With `chainTab`, every chain id it names (ch_…) links to that tab of
// the chain, opened in the app (an empty one: the chain's default tab). With `wrap`, a code block wraps its long lines instead of
// scrolling, for text the reader must see whole before he answers (a program he is asked to apply). With `writing`, the text is still being written and
// ends in a caret.
const props = defineProps<{ text: string; type?: 'body' | 'quiet'; chainTab?: string; writing?: boolean; wrap?: boolean }>();
const router = useRouter();
const CHAIN_ID = /\bch_[a-z0-9]+\b/;
// A page has one h1, the Shell's (pageHeading.ts): the headings an agent writes sit beneath the page's, one level down.
const markdown = new Marked({
  renderer: {
    heading({ tokens, depth }) {
      const level = Math.min(depth + 1, 6);
      return `<h${level}>${this.parser.parseInline(tokens)}</h${level}>\n`;
    },
  },
});

// Links the chain ids in the text of html, outside any link it already has.
function linkChains(html: string, tab: string): string {
  const root = document.createElement('div');
  root.innerHTML = html;
  const walk = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  const texts: Text[] = [];
  for (let n = walk.nextNode(); n; n = walk.nextNode()) if (!n.parentElement?.closest('a')) texts.push(n as Text);
  for (const t of texts) {
    const parts = t.data.split(/(\bch_[a-z0-9]+\b)/);
    if (parts.length === 1) continue;
    t.replaceWith(...parts.map((p, i) => {
      if (i % 2 === 0) return p;
      const a = document.createElement('a');
      a.href = tab ? `/chains/${p}/${tab}` : `/chains/${p}`;
      a.dataset.chain = p;
      a.textContent = p;
      return a;
    }));
  }
  return root.innerHTML;
}
// Puts a caret right after the last character of the text in html, inside whatever paragraph, list item or code holds it,
// so it sits where the writing goes on.
function endInCaret(html: string): string {
  const root = document.createElement('div');
  root.innerHTML = html;
  const walk = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  let last: Text | null = null;
  for (let n = walk.nextNode(); n; n = walk.nextNode()) if ((n as Text).data.trim()) last = n as Text;
  const caret = document.createElement('i');
  caret.className = 'caret';
  if (last) last.after(caret);
  else root.append(caret);
  return root.innerHTML;
}
const html = computed(() => {
  const safe = DOMPurify.sanitize(markdown.parse(props.text ?? '', { async: false, gfm: true, breaks: true }) as string);
  const linked = props.chainTab !== undefined && CHAIN_ID.test(safe) ? linkChains(safe, props.chainTab) : safe;
  return props.writing ? endInCaret(linked) : linked;
});
// A chain link opens the chain in the app, not as a new page load.
function follow(e: MouseEvent) {
  const a = (e.target as Element).closest<HTMLAnchorElement>('a[data-chain]');
  if (!a || !router || e.metaKey || e.ctrlKey || e.shiftKey || e.button !== 0) return;
  e.preventDefault();
  void router.push(a.getAttribute('href')!);
}
</script>

<template>
  <div class="md" :class="[type, { wrap }]" v-html="html" @click="follow"></div>
</template>

<style scoped>
@reference "../theme.css";

.md {
  word-break: break-word;
}
/* what an agent says is read at length: the reading size, not the console's */
.body {
  font-size: var(--size-read);
  line-height: var(--leading-read);
  color: var(--text);
}
.quiet {
  font-size: 12.5px;
  line-height: 1.6;
  color: var(--text2);
}
.md :deep(p) {
  margin: 0 0 8px;
}
.md :deep(p:last-child) {
  margin-bottom: 0;
}
.md :deep(:is(h2, h3, h4, h5, h6)) {
  margin: 10px 0 6px;
  font-size: 13px;
  font-weight: 700;
}
.md :deep(:is(ul, ol)) {
  margin: 4px 0 8px;
  padding-left: 20px;
}
.md :deep(ul) {
  list-style: disc;
}
.md :deep(ol) {
  list-style: decimal;
}
.md :deep(li) {
  margin: 2px 0;
}
.md :deep(code) {
  @apply code-chip;
}
.md :deep(pre) {
  margin: 6px 0;
  padding: 9px 11px;
  overflow: auto;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--bg);
}
.md.wrap :deep(pre) {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
.md :deep(pre code) {
  padding: 0;
  background: none;
}
.md :deep(table) {
  margin: 6px 0;
  border-collapse: collapse;
  font-size: 12px;
}
.md :deep(:is(th, td)) {
  padding: 4px 8px;
  border: 1px solid var(--line2);
}
.md :deep(a) {
  color: var(--blue);
  text-decoration: underline;
}
.md :deep(blockquote) {
  margin: 6px 0;
  padding-left: 10px;
  border-left: 2px solid var(--line2);
  color: var(--mut);
}
/* The text being written ends in a blinking caret. */
.md :deep(.caret) {
  display: inline-block;
  width: 7px;
  height: 14px;
  margin-left: 2px;
  vertical-align: -2px;
  background: var(--blue);
  animation: blink 1s infinite;
}
@keyframes blink {
  50% {
    opacity: 0.25;
  }
}
@media (prefers-reduced-motion: reduce) {
  .md :deep(.caret) {
    animation: none;
  }
}
</style>
