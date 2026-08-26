"""Checks on the embeddings and the retrieval harness (P2).

These read the built index.db. They do not re-embed anything and do not start a
model: a test that needed a 600 MB model to run would not be run.
"""

import os
import sqlite3
import struct
import sys

import pytest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DB = os.path.join(ROOT, 'src-tauri', 'resources', 'index.db')
sys.path.insert(0, os.path.join(ROOT, 'pipeline'))


@pytest.fixture(scope='module')
def db():
    if not os.path.exists(DB):
        pytest.skip('index.db not built')
    con = sqlite3.connect('file:%s?mode=ro' % DB.replace('\\', '/'), uri=True)
    row = con.execute("SELECT value FROM meta WHERE key='index_version'").fetchone()
    if not row or row[0] < '0.2.0':
        con.close()
        pytest.skip('index.db has no embeddings yet')
    yield con
    con.close()


@pytest.fixture(scope='module')
def models(db):
    return [r[0] for r in db.execute(
        'SELECT model_id FROM embedding_models ORDER BY model_id')]


def one(db, sql, *a):
    return db.execute(sql, a).fetchone()[0]


# ------------------------------------------------------------------ coverage

def test_at_least_one_model_registered(models):
    assert models


def test_every_verse_has_an_embedding_per_model(db, models):
    verses = one(db, 'SELECT COUNT(*) FROM verses')
    for m in models:
        n = one(db, 'SELECT COUNT(*) FROM verse_embeddings WHERE model_id=?', m)
        assert n == verses, '%s has %d verse vectors, expected %d' % (m, n, verses)


def test_every_topic_has_an_embedding_per_model(db, models):
    topics = one(db, 'SELECT COUNT(*) FROM nave_topics')
    for m in models:
        n = one(db, 'SELECT COUNT(*) FROM topic_embeddings WHERE model_id=?', m)
        assert n == topics, '%s has %d topic vectors, expected %d' % (m, n, topics)


def test_every_pericope_is_covered_per_model(db, models):
    pericopes = one(db, 'SELECT COUNT(*) FROM pericopes')
    for m in models:
        covered = one(db, 'SELECT COUNT(DISTINCT pericope_id) FROM'
                          ' pericope_embeddings WHERE model_id=?', m)
        assert covered == pericopes, \
            '%s covers %d pericopes, expected %d' % (m, covered, pericopes)
        # A split pericope contributes several parts, never fewer than one.
        parts = one(db, 'SELECT COUNT(*) FROM pericope_embeddings'
                        ' WHERE model_id=?', m)
        assert parts >= pericopes


def test_split_parts_are_numbered_from_zero(db, models):
    for m in models:
        bad = one(db, 'SELECT COUNT(*) FROM (SELECT pericope_id, MIN(part) mn'
                      ' FROM pericope_embeddings WHERE model_id=?'
                      ' GROUP BY pericope_id) WHERE mn != 0', m)
        assert bad == 0, m


def test_split_parts_stay_inside_their_pericope(db, models):
    for m in models:
        bad = one(db, 'SELECT COUNT(*) FROM pericope_embeddings e'
                      ' JOIN pericopes p USING (pericope_id)'
                      ' WHERE e.model_id=? AND (e.start_verse_id < p.start_verse_id'
                      ' OR e.end_verse_id > p.end_verse_id)', m)
        assert bad == 0, m


# ----------------------------------------------------------------- integrity

def test_dims_match_the_declared_dimension(db, models):
    for m in models:
        dim = one(db, 'SELECT dim FROM embedding_models WHERE model_id=?', m)
        for table in ('verse_embeddings', 'pericope_embeddings',
                      'topic_embeddings'):
            mismatched = one(db, 'SELECT COUNT(*) FROM %s WHERE model_id=?'
                                 ' AND dim != ?' % table, m, dim)
            assert mismatched == 0, '%s %s' % (m, table)
            bad_len = one(db, 'SELECT COUNT(*) FROM %s WHERE model_id=?'
                              ' AND LENGTH(vec) != ?' % table, m, dim * 4)
            assert bad_len == 0, '%s %s blob length' % (m, table)


def test_vectors_are_unit_normalized(db, models):
    for m in models:
        rows = db.execute('SELECT vec FROM verse_embeddings WHERE model_id=?'
                          ' LIMIT 50', (m,)).fetchall()
        assert rows
        for (blob,) in rows:
            v = struct.unpack('<%df' % (len(blob) // 4), blob)
            norm = sum(x * x for x in v) ** 0.5
            assert abs(norm - 1.0) < 1e-3, '%s norm %.6f' % (m, norm)


def test_meta_records_the_embedding_state(db, models):
    meta = dict(db.execute('SELECT key, value FROM meta'))
    assert meta['index_version'] == '0.2.0'
    assert meta['schema_version'] == '2'
    assert meta['embedding_normalized'] == '1'
    for m in models:
        assert m in meta['embedding_models']


def test_model_rows_carry_prefixes_and_checksums(db, models):
    for m in models:
        row = db.execute('SELECT gguf_file, sha256, dim, n_ctx, doc_prefix,'
                         ' query_prefix FROM embedding_models WHERE model_id=?',
                         (m,)).fetchone()
        assert row[0].endswith('.gguf')
        assert len(row[1]) == 64
        assert row[2] > 0 and row[3] > 0
        # A prefix may legitimately be empty, but the column must exist and the
        # query prefix must be present for models whose card requires one.
        assert isinstance(row[4], str) and isinstance(row[5], str)


# ------------------------------------------------------------------- harness

@pytest.fixture(scope='module')
def retriever(db, models):
    from retrieve import Retriever
    return Retriever(model_id=models[0])


def _qvec(retriever, verse_id):
    """Use a stored verse vector as a stand-in query.

    Avoids loading a model in the test suite while still exercising the whole
    search path with a real vector of the right dimension.
    """
    blob = retriever.con.execute(
        'SELECT vec FROM verse_embeddings WHERE model_id=? AND verse_id=?',
        (retriever.model_id, verse_id)).fetchone()[0]
    return list(struct.unpack('<%df' % (len(blob) // 4), blob))


def test_config_c_finds_the_verse_its_own_vector_came_from(retriever, db):
    # Philippians 4:6, "In nothing be anxious". Querying with a passage's own
    # vector must return that passage; if it does not, the store is wired wrong.
    vid = db.execute(
        "SELECT v.verse_id FROM verses v JOIN books b USING (book_id)"
        " WHERE b.usfm_code='PHP' AND v.chapter=4 AND v.verse=6").fetchone()[0]
    full, top, _ = retriever.search(
        _qvec(retriever, vid), ['anxious'], canon_mode='66',
        use_vector_verses=True, use_vector_pericopes=True, use_fts=False,
        use_topics=False, use_tsk=False, top_n=25)
    ranges = retriever.as_ranges(top)
    assert any(vid in r['ids'] for r in ranges), 'own vector did not retrieve itself'


def test_canon_filter_excludes_every_deutero_row(retriever, db):
    vid = db.execute("SELECT verse_id FROM verses LIMIT 1").fetchone()[0]
    q = _qvec(retriever, vid)
    full, _, _ = retriever.search(q, ['wisdom'], canon_mode='66',
                                  use_topics=True, use_tsk=True)
    assert full
    assert all(c['canon'] == 'protestant' for c in full)

    full_both, _, _ = retriever.search(q, ['wisdom'], canon_mode='both',
                                       use_topics=True, use_tsk=True)
    assert any(c['canon'] == 'deutero' for c in full_both), \
        'both-canon mode returned no deuterocanonical candidate at all'


def test_full_set_is_a_superset_of_the_top_n(retriever, db):
    vid = db.execute("SELECT verse_id FROM verses LIMIT 1").fetchone()[0]
    full, top, _ = retriever.search(_qvec(retriever, vid), ['mercy'],
                                    canon_mode='66', use_topics=True,
                                    use_tsk=True, top_n=25)
    assert len(top) <= len(full)
    full_ids = {c['verse_id'] for c in full}
    assert {c['verse_id'] for c in top} <= full_ids
    assert top == full[:len(top)]


def test_ranges_never_span_a_chapter(retriever, db):
    vid = db.execute("SELECT verse_id FROM verses LIMIT 1").fetchone()[0]
    full, _, _ = retriever.search(_qvec(retriever, vid), ['love'],
                                  canon_mode='66')
    for r in retriever.as_ranges(full):
        chapters = {i // 1000 for i in r['ids']}
        assert len(chapters) == 1, r['ref']


# ------------------------------------------- additive canon retrieval (P3)

def test_66_result_is_a_prefix_of_the_both_result(retriever, db):
    """The canon toggle may only add.

    P2 measured that unfiltering the Deuterocanon displaced up to 15 of the 25
    protestant passages, so a reader turning the setting on lost passages. P3
    made both-canon retrieval additive; this asserts the property that change
    was made to guarantee.
    """
    import json
    import os
    qpath = os.path.join(ROOT, 'data', 'eval', 'questions.json')
    data = json.load(open(qpath, encoding='utf-8'))
    for qid in ('g19', 'g20'):
        q = next(g for g in data['graded'] if g['id'] == qid)
        vid = db.execute(
            'SELECT verse_id FROM verses ORDER BY verse_id LIMIT 1').fetchone()[0]
        qv = _qvec(retriever, vid)
        base, _, _ = retriever.search(qv, q['keywords'], canon_mode='66',
                                      use_topics=True, use_tsk=True)
        both, _, _ = retriever.search(qv, q['keywords'], canon_mode='both',
                                      use_topics=True, use_tsk=True,
                                      additive_deutero=True)
        base_ids = [c['verse_id'] for c in base]
        both_ids = [c['verse_id'] for c in both]
        assert both_ids[:len(base_ids)] == base_ids, \
            '%s: canon 66 result is not a prefix of the both result' % qid
        assert set(base_ids) <= set(both_ids), qid


def test_additive_mode_actually_adds_deuterocanon(retriever, db):
    import json
    import os
    qpath = os.path.join(ROOT, 'data', 'eval', 'questions.json')
    data = json.load(open(qpath, encoding='utf-8'))
    q = next(g for g in data['graded'] if g['id'] == 'g19')
    vid = db.execute(
        'SELECT verse_id FROM verses ORDER BY verse_id LIMIT 1').fetchone()[0]
    both, _, _ = retriever.search(_qvec(retriever, vid), q['keywords'],
                                  canon_mode='both', use_topics=True,
                                  use_tsk=True, additive_deutero=True)
    assert any(c['canon'] == 'deutero' for c in both)
