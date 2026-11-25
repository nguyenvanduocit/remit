-- Memory is Remit's own (ADR-0117): what each agent remembers, the words, meaning and links it is searched by, and the
-- teachings the librarian learns from. A memory is a document with the columns recall filters on; its text index, vectors
-- and edges are written with it and go with it.
CREATE TABLE IF NOT EXISTS memories (
    id TEXT PRIMARY KEY,
    holder TEXT NOT NULL,
    template TEXT NOT NULL,
    project TEXT NOT NULL,
    scope TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    doc TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS memories_holder ON memories(holder, status);
CREATE INDEX IF NOT EXISTS memories_project ON memories(project, scope, status);
CREATE INDEX IF NOT EXISTS memories_template ON memories(template, scope, status);

-- The words of each memory. Diacritics are folded, so a Vietnamese query finds its words written without them.
CREATE VIRTUAL TABLE IF NOT EXISTS memory_text USING fts5(
    memory_id UNINDEXED,
    title,
    body,
    evidence,
    about,
    tokenize = 'unicode61 remove_diacritics 2'
);

-- The meaning of each memory: its vector under one embedding model, little-endian f32.
CREATE TABLE IF NOT EXISTS memory_vectors (
    memory_id TEXT NOT NULL,
    model TEXT NOT NULL,
    vec BLOB NOT NULL,
    PRIMARY KEY (memory_id, model)
);

-- The graph: a memory to another memory (kind is a MemoryLinkKind) or to a subject it is about (kind 'about', dst the
-- subject: file:<path>, crate:<name>, tool:<name>, topic:<words>).
CREATE TABLE IF NOT EXISTS memory_edges (
    src TEXT NOT NULL,
    dst TEXT NOT NULL,
    kind TEXT NOT NULL,
    PRIMARY KEY (src, dst, kind)
);
CREATE INDEX IF NOT EXISTS memory_edges_dst ON memory_edges(dst, kind);

-- What the librarian is to learn from: one teaching the gate let through, waiting or settled.
CREATE TABLE IF NOT EXISTS teachings (
    id TEXT PRIMARY KEY,
    state TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    doc TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS teachings_state ON teachings(state, created_at);
