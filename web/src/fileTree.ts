// The files a landing changes, as the owner reads them: an explorer of folders and files, the one to open first, whether the
// change is to how the thing is deployed. Pure.
import type { FileChange } from './core/generated/types.generated';
import { filePatch } from './patch';

// A change to how the thing is deployed, not just to what it does.
export function isInfra(files: FileChange[]): boolean {
  return files.some((f) => /(^|\/)(serverless|.*stack.*|.*\.tf|helm|chart)[^/]*\.ya?ml$|slsconfig\//.test(f.path));
}

export type TreeRow =
  | { kind: 'dir'; path: string; name: string; depth: number; open: boolean }
  | { kind: 'file'; path: string; name: string; depth: number; file: FileChange };

interface Dir { name: string; path: string; dirs: Map<string, Dir>; files: FileChange[] }

// The changed files as the rows of a file explorer, in the order they are drawn: folders before files, each sorted by name.
// A folder holding only one folder is joined with it (crates/orch), as editors do. Folders in `closed` hide their rows.
export function treeRows(files: FileChange[], closed: ReadonlySet<string> = new Set()): TreeRow[] {
  const root: Dir = { name: '', path: '', dirs: new Map(), files: [] };
  for (const f of files) {
    const parts = f.path.split('/');
    let dir = root;
    for (const name of parts.slice(0, -1)) {
      if (!dir.dirs.has(name)) dir.dirs.set(name, { name, path: dir.path ? `${dir.path}/${name}` : name, dirs: new Map(), files: [] });
      dir = dir.dirs.get(name)!;
    }
    dir.files.push(f);
  }
  const rows: TreeRow[] = [];
  const walk = (dir: Dir, depth: number) => {
    for (let d of [...dir.dirs.values()].sort((a, b) => a.name.localeCompare(b.name))) {
      let name = d.name;
      while (!d.files.length && d.dirs.size === 1) {
        d = [...d.dirs.values()][0];
        name += `/${d.name}`;
      }
      const open = !closed.has(d.path);
      rows.push({ kind: 'dir', path: d.path, name, depth, open });
      if (open) walk(d, depth + 1);
    }
    for (const file of [...dir.files].sort((a, b) => a.path.localeCompare(b.path))) {
      rows.push({ kind: 'file', path: file.path, name: file.path.slice(file.path.lastIndexOf('/') + 1), depth, file });
    }
  };
  walk(root, 0);
  return rows;
}

// The file a landing's diff opens on when the address names none: the first in the explorer with a textual diff to read (not
// a binary one), else the first.
export function firstFile(files: FileChange[]): FileChange | undefined {
  const listed = treeRows(files).flatMap((r) => (r.kind === 'file' ? [r.file] : []));
  return listed.find((f) => filePatch(f)) ?? listed[0];
}
