// A file's change as the one-file git patch the diff viewer reads. A forge hands over only the hunks, so the headers are
// made here from the path and whether the file is new or deleted. Empty when there are no hunks (binary, or too large). Pure.
import type { FileChange } from './core/generated/types.generated';

export function filePatch(f: FileChange): string {
  const at = f.diff.search(/^@@ /m);
  if (at < 0) return '';
  const head = [`diff --git a/${f.path} b/${f.path}`];
  if (f.new) head.push('new file mode 100644');
  if (f.deleted) head.push('deleted file mode 100644');
  head.push(`--- ${f.new ? '/dev/null' : `a/${f.path}`}`, `+++ ${f.deleted ? '/dev/null' : `b/${f.path}`}`);
  return `${head.join('\n')}\n${f.diff.slice(at)}`;
}
