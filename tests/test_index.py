"""Invariants that must hold in the built index.db.

These tests read the database file. They do not import the pipeline and do not
re-derive anything from the sources: if the build produced something different
from what it reported, these fail.

Run:  pipeline/.venv/Scripts/python -m pytest tests -q
"""

import os
import sqlite3

import pytest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DB = os.path.join(ROOT, 'src-tauri', 'resources', 'index.db')

# Figures established in P1 and reconciled in docs/HANDOFF.md. A change to any
# of these is a change to the text or the parse, and must be explained before
# the number here is edited.
EXPECTED = {
    'books_total': 81,
    'books_protestant': 66,
    'books_deutero': 15,
    'verses_total': 38029,
    'verses_protestant': 31098,
    'verses_deutero': 6931,
    'verses_protestant_ot': 23145,
    'verses_protestant_nt': 7953,
    'chapters_total': 1402,
    'verse_bridges': 1,
    'omitted_verse_markers': 29,
}

SPOT_CHECKS = {
    ('GEN', 1, 1): 'In the beginning, God created the heavens and the earth.',
    ('PSA', 23, 1): 'Yahweh is my shepherd; I shall lack nothing.',
    ('REV', 22, 21): 'The grace of the Lord Jesus Christ be with all the saints. Amen.',
}


@pytest.fixture(scope='module')
def db():
    if not os.path.exists(DB):
        pytest.skip('index.db not built; run pipeline/build_index.py')
    con = sqlite3.connect('file:%s?mode=ro' % DB.replace('\\', '/'), uri=True)
    yield con
    con.close()


def one(db, sql, *args):
    return db.execute(sql, args).fetchone()[0]


# ---------------------------------------------------------------- structure

def test_integrity_check_ok(db):
    assert one(db, 'PRAGMA integrity_check') == 'ok'


def test_no_foreign_key_violations(db):
    assert db.execute('PRAGMA foreign_key_check').fetchall() == []


def test_book_counts(db):
    assert one(db, 'SELECT COUNT(*) FROM books') == EXPECTED['books_total']
    assert one(db, "SELECT COUNT(*) FROM books WHERE canon='protestant'") \
        == EXPECTED['books_protestant']
    assert one(db, "SELECT COUNT(*) FROM books WHERE canon='deutero'") \
        == EXPECTED['books_deutero']


def test_verse_counts(db):
    assert one(db, 'SELECT COUNT(*) FROM verses') == EXPECTED['verses_total']
    assert one(db, "SELECT COUNT(*) FROM verses v JOIN books b USING (book_id)"
                   " WHERE b.canon='protestant'") == EXPECTED['verses_protestant']
    assert one(db, "SELECT COUNT(*) FROM verses v JOIN books b USING (book_id)"
                   " WHERE b.canon='deutero'") == EXPECTED['verses_deutero']


def test_protestant_testament_split(db):
    assert one(db, "SELECT COUNT(*) FROM verses v JOIN books b USING (book_id)"
                   " WHERE b.canon='protestant' AND b.testament='OT'") \
        == EXPECTED['verses_protestant_ot']
    assert one(db, "SELECT COUNT(*) FROM verses v JOIN books b USING (book_id)"
                   " WHERE b.canon='protestant' AND b.testament='NT'") \
        == EXPECTED['verses_protestant_nt']


def test_chapter_count(db):
    assert one(db, 'SELECT COUNT(*) FROM (SELECT DISTINCT book_id, chapter'
                   ' FROM verses)') == EXPECTED['chapters_total']


def test_every_verse_has_a_book(db):
    assert one(db, 'SELECT COUNT(*) FROM verses v LEFT JOIN books b'
                   ' USING (book_id) WHERE b.book_id IS NULL') == 0


def test_every_verse_has_a_pericope(db):
    assert one(db, 'SELECT COUNT(*) FROM verses WHERE pericope_id IS NULL') == 0


def test_every_verse_has_text(db):
    assert one(db, "SELECT COUNT(*) FROM verses WHERE text IS NULL"
                   " OR TRIM(text)=''") == 0


def test_no_usfm_markup_leaked_into_text(db):
    assert one(db, "SELECT COUNT(*) FROM verses WHERE text LIKE '%\\%'"
                   " ESCAPE '\\' OR text LIKE '%strong=%'") == 0


def test_verse_id_encodes_book_chapter_verse(db):
    assert one(db, 'SELECT COUNT(*) FROM verses WHERE verse_id !='
                   ' book_id*1000000 + chapter*1000 + verse') == 0


def test_verse_bridges(db):
    assert one(db, 'SELECT COUNT(*) FROM verses WHERE verse_end IS NOT NULL') \
        == EXPECTED['verse_bridges']
    # A bridge must end after it starts.
    assert one(db, 'SELECT COUNT(*) FROM verses WHERE verse_end IS NOT NULL'
                   ' AND verse_end <= verse') == 0


def test_pericope_bounds_are_sane(db):
    assert one(db, 'SELECT COUNT(*) FROM pericopes WHERE end_verse_id'
                   ' < start_verse_id') == 0
    assert one(db, 'SELECT COUNT(*) FROM pericopes p LEFT JOIN books b'
                   ' USING (book_id) WHERE b.book_id IS NULL') == 0


# ------------------------------------------------------------------- corpora

def test_every_tsk_edge_resolves(db):
    assert one(db, 'SELECT COUNT(*) FROM tsk_refs t LEFT JOIN verses v'
                   ' ON v.verse_id = t.from_verse_id WHERE v.verse_id IS NULL') == 0
    assert one(db, 'SELECT COUNT(*) FROM tsk_refs t LEFT JOIN verses v'
                   ' ON v.verse_id = t.to_verse_id WHERE v.verse_id IS NULL') == 0


def test_tsk_has_edges(db):
    assert one(db, 'SELECT COUNT(*) FROM tsk_refs') > 500000


def test_every_nave_reference_resolves(db):
    assert one(db, 'SELECT COUNT(*) FROM nave_topic_verses n LEFT JOIN verses v'
                   ' USING (verse_id) WHERE v.verse_id IS NULL') == 0


def test_nave_parents_exist(db):
    assert one(db, 'SELECT COUNT(*) FROM nave_topics t WHERE t.parent_topic_id'
                   ' IS NOT NULL AND NOT EXISTS (SELECT 1 FROM nave_topics p'
                   ' WHERE p.topic_id = t.parent_topic_id)') == 0


def test_nave_hierarchy_is_two_levels(db):
    # A subtopic's parent must itself be top level; no deeper nesting exists.
    assert one(db, 'SELECT COUNT(*) FROM nave_topics t JOIN nave_topics p'
                   ' ON p.topic_id = t.parent_topic_id'
                   ' WHERE p.parent_topic_id IS NOT NULL') == 0


def test_unresolved_stay_quarantined_not_dropped(db):
    # The quarantine tables exist and are queryable; TSK's marginal-note
    # markers land there rather than being silently discarded.
    assert one(db, 'SELECT COUNT(*) FROM tsk_unresolved') > 0
    assert one(db, "SELECT COUNT(*) FROM tsk_unresolved WHERE reason=''"
                   " OR reason IS NULL") == 0
    assert one(db, "SELECT COUNT(*) FROM nave_unresolved WHERE reason=''"
                   " OR reason IS NULL") == 0


# ----------------------------------------------------------------------- FTS

def test_fts_row_count_matches_verses(db):
    assert one(db, 'SELECT COUNT(*) FROM verse_fts') == one(
        db, 'SELECT COUNT(*) FROM verses')


def test_fts_finds_a_known_word(db):
    hits = one(db, "SELECT COUNT(*) FROM verse_fts WHERE verse_fts MATCH 'anxious'")
    assert hits > 0
    # The match must join back to a real verse.
    assert one(db, "SELECT COUNT(*) FROM verse_fts f JOIN verses v"
                   " ON v.verse_id = f.rowid WHERE verse_fts MATCH 'anxious'") == hits


# ------------------------------------------------------------------ contents

@pytest.mark.parametrize('ref,expected', sorted(SPOT_CHECKS.items()))
def test_spot_check_verse_text(db, ref, expected):
    code, ch, vs = ref
    got = one(db, 'SELECT v.text FROM verses v JOIN books b USING (book_id)'
                  ' WHERE b.usfm_code=? AND v.chapter=? AND v.verse=?',
              code, ch, vs)
    assert got == expected


def test_john_3_16_is_present_and_sane(db):
    got = one(db, "SELECT v.text FROM verses v JOIN books b USING (book_id)"
                  " WHERE b.usfm_code='JHN' AND v.chapter=3 AND v.verse=16")
    assert got.startswith('For God so loved the world')


def test_deuterocanon_present_and_flagged(db):
    got = one(db, "SELECT b.canon FROM verses v JOIN books b USING (book_id)"
                  " WHERE b.usfm_code='TOB' AND v.chapter=1 AND v.verse=1")
    assert got == 'deutero'


def test_meta_matches_the_data(db):
    meta = dict(db.execute('SELECT key, value FROM meta').fetchall())
    assert meta['schema_version'] == '1'
    assert meta['index_version'] == '0.1.0'
    assert int(meta['omitted_verse_markers']) == EXPECTED['omitted_verse_markers']
    assert len(meta['build_checksum']) == 64
    for k in ('source_sha256_web', 'source_sha256_tsk', 'source_sha256_nave'):
        assert len(meta[k]) == 64


def test_embedding_tables_exist(db):
    # P1 asserted these were absent. P2 created them; the guard now runs the
    # other way, and the single generic "embeddings" table sketched in PLAN 3.2
    # is deliberately still absent, replaced by three specific ones.
    names = {r[0] for r in db.execute(
        "SELECT name FROM sqlite_master WHERE type='table'")}
    assert 'embeddings' not in names
    assert {'verse_embeddings', 'pericope_embeddings', 'topic_embeddings',
            'embedding_models'} <= names
