-- index.db schema for The Pastor Bible.
--
-- Follows PLAN.md 3.2 (index.db portion). Two deliberate deviations from 3.2,
-- both recorded in docs/DECISIONS.md:
--   1. verses.verse_end, to hold verse bridges faithfully.
--   2. tsk_unresolved and nave_unresolved, quarantine tables so that a
--      reference we could not resolve is inspectable rather than dropped.
--
-- The embeddings and topic_embeddings tables named in 3.2 are NOT created here.
-- They are built in P2, together with the vector store decision that P2 owns.
--
-- This file is read and executed verbatim by build_index.py.

PRAGMA foreign_keys = ON;

CREATE TABLE books (
    book_id     INTEGER PRIMARY KEY,   -- 1-based, in canonical file order
    usfm_code   TEXT    NOT NULL UNIQUE,-- GEN, EXO, ... as the source spells it
    name        TEXT    NOT NULL,       -- long name, from USFM \toc1
    abbrev      TEXT    NOT NULL,       -- short form, from USFM \toc3
    testament   TEXT    NOT NULL CHECK (testament IN ('OT', 'NT')),
    canon       TEXT    NOT NULL CHECK (canon IN ('protestant', 'deutero')),
    book_order  INTEGER NOT NULL UNIQUE -- "order" is a reserved word in SQL
);

CREATE TABLE pericopes (
    pericope_id    INTEGER PRIMARY KEY,
    book_id        INTEGER NOT NULL REFERENCES books(book_id),
    start_verse_id INTEGER NOT NULL,
    end_verse_id   INTEGER NOT NULL,
    heading        TEXT,                -- NULL where the source gives none
    source         TEXT NOT NULL        -- 'heading' or 'paragraph'
                   CHECK (source IN ('heading', 'paragraph'))
);

CREATE TABLE verses (
    verse_id    INTEGER PRIMARY KEY,   -- book_id*1000000 + chapter*1000 + verse
    book_id     INTEGER NOT NULL REFERENCES books(book_id),
    chapter     INTEGER NOT NULL,
    verse       INTEGER NOT NULL,      -- first number of a bridge
    verse_end   INTEGER,               -- last number of a bridge, else NULL
    text        TEXT    NOT NULL,
    pericope_id INTEGER REFERENCES pericopes(pericope_id),
    UNIQUE (book_id, chapter, verse)
);

CREATE INDEX idx_verses_book_chapter ON verses (book_id, chapter);
CREATE INDEX idx_pericopes_book ON pericopes (book_id);

-- Keyword path. Contentless FTS5 over verses.text: the text is stored once, in
-- verses, and the index refers back to it by rowid.
CREATE VIRTUAL TABLE verse_fts USING fts5 (
    text,
    content='verses',
    content_rowid='verse_id',
    tokenize='porter unicode61'
);

-- Treasury of Scripture Knowledge cross-reference edges.
CREATE TABLE tsk_refs (
    from_verse_id INTEGER NOT NULL REFERENCES verses(verse_id),
    to_verse_id   INTEGER NOT NULL REFERENCES verses(verse_id),
    anchor        TEXT,                -- the phrase in the source verse, if given
    PRIMARY KEY (from_verse_id, to_verse_id, anchor)
) WITHOUT ROWID;

CREATE INDEX idx_tsk_to ON tsk_refs (to_verse_id);

-- References TSK gave that could not be resolved to a verse row. Kept for
-- inspection rather than dropped, so the loss is visible and measurable.
CREATE TABLE tsk_unresolved (
    from_verse_id INTEGER,             -- NULL if the source verse itself failed
    raw           TEXT NOT NULL,       -- the reference exactly as written
    reason        TEXT NOT NULL
);

CREATE TABLE nave_topics (
    topic_id        INTEGER PRIMARY KEY,
    heading         TEXT NOT NULL,
    parent_topic_id INTEGER REFERENCES nave_topics(topic_id),
    see_also        TEXT                -- cross-topic pointer, raw text, or NULL
);

CREATE INDEX idx_nave_parent ON nave_topics (parent_topic_id);

CREATE TABLE nave_topic_verses (
    topic_id INTEGER NOT NULL REFERENCES nave_topics(topic_id),
    verse_id INTEGER NOT NULL REFERENCES verses(verse_id),
    PRIMARY KEY (topic_id, verse_id)
) WITHOUT ROWID;

CREATE INDEX idx_nave_tv_verse ON nave_topic_verses (verse_id);

CREATE TABLE nave_unresolved (
    topic_id INTEGER,
    raw      TEXT NOT NULL,
    reason   TEXT NOT NULL
);

CREATE TABLE meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- ---------------------------------------------------------------------------
-- Embeddings (P2).
--
-- Plain float32 little-endian BLOBs, unit-normalized at write time so that a
-- cosine similarity is a plain dot product. Searched brute force: about 70,000
-- small vectors per model, which scans in milliseconds and spares the installer
-- a bundled native extension. sqlite-vec is deliberately not used; see
-- DECISIONS.md and PLAN.md 3.2.
--
-- All shortlisted models are stored side by side. The retrieval harness selects
-- one by model_id, so configurations can be compared without a rebuild.
-- ---------------------------------------------------------------------------

CREATE TABLE verse_embeddings (
    model_id TEXT    NOT NULL,
    verse_id INTEGER NOT NULL REFERENCES verses(verse_id),
    dim      INTEGER NOT NULL,
    vec      BLOB    NOT NULL,
    PRIMARY KEY (model_id, verse_id)
) WITHOUT ROWID;

CREATE TABLE pericope_embeddings (
    model_id    TEXT    NOT NULL,
    pericope_id INTEGER NOT NULL REFERENCES pericopes(pericope_id),
    part        INTEGER NOT NULL DEFAULT 0,  -- >0 when a pericope was split
    dim         INTEGER NOT NULL,
    vec         BLOB    NOT NULL,
    start_verse_id INTEGER NOT NULL,
    end_verse_id   INTEGER NOT NULL,
    PRIMARY KEY (model_id, pericope_id, part)
) WITHOUT ROWID;

CREATE TABLE topic_embeddings (
    model_id TEXT    NOT NULL,
    topic_id INTEGER NOT NULL REFERENCES nave_topics(topic_id),
    dim      INTEGER NOT NULL,
    vec      BLOB    NOT NULL,
    PRIMARY KEY (model_id, topic_id)
) WITHOUT ROWID;

CREATE TABLE embedding_models (
    model_id      TEXT PRIMARY KEY,
    gguf_file     TEXT NOT NULL,
    sha256        TEXT NOT NULL,
    dim           INTEGER NOT NULL,
    n_ctx         INTEGER NOT NULL,
    doc_prefix    TEXT NOT NULL,
    query_prefix  TEXT NOT NULL,
    normalized    INTEGER NOT NULL DEFAULT 1
);
