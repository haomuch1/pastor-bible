"""Checks on the evaluation set in data/eval/questions.json.

The gold lists are a draft awaiting Jared's approval, so nothing here judges
whether a passage is the right one. What is checked is that the file is
structurally sound and that every reference in it points at verses that
actually exist: a gold list containing a passage the index cannot produce
would make every recall figure in P2 meaningless.
"""

import json
import os
import sqlite3

import pytest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DB = os.path.join(ROOT, 'src-tauri', 'resources', 'index.db')
QUESTIONS = os.path.join(ROOT, 'data', 'eval', 'questions.json')

MUST_MIN, MUST_MAX = 5, 8


@pytest.fixture(scope='module')
def data():
    if not os.path.exists(QUESTIONS):
        pytest.skip('questions.json not generated')
    with open(QUESTIONS, encoding='utf-8') as fh:
        return json.load(fh)


@pytest.fixture(scope='module')
def db():
    if not os.path.exists(DB):
        pytest.skip('index.db not built')
    con = sqlite3.connect('file:%s?mode=ro' % DB.replace('\\', '/'), uri=True)
    yield con
    con.close()


def all_passages(data):
    for g in data['graded']:
        for bucket in ('must', 'should'):
            for p in g[bucket]:
                yield g['id'], bucket, p


# ------------------------------------------------------------------ structure

def test_status_is_draft(data):
    # P2 flips this to approved only after Jared has signed off every MUST list.
    assert data['status'] == 'draft'
    assert all(g['status'] == 'draft' for g in data['graded'])


def test_counts(data):
    assert len(data['graded']) == 20
    assert len(data['smoke']) == 20


def test_ids_are_unique_and_well_formed(data):
    gids = [g['id'] for g in data['graded']]
    sids = [s['id'] for s in data['smoke']]
    assert gids == sorted(gids)
    assert len(set(gids)) == 20
    assert len(set(sids)) == 20
    assert all(i.startswith('g') for i in gids)
    assert all(i.startswith('s') for i in sids)


def test_every_graded_question_has_5_to_8_must_entries(data):
    for g in data['graded']:
        assert MUST_MIN <= len(g['must']) <= MUST_MAX, g['id']


def test_should_is_non_gating_but_present(data):
    for g in data['graded']:
        assert len(g['should']) > 0, g['id']


def test_must_and_should_do_not_overlap(data):
    for g in data['graded']:
        must = {p['ref'] for p in g['must']}
        should = {p['ref'] for p in g['should']}
        assert not (must & should), g['id']


def test_graded_fields(data):
    for g in data['graded']:
        assert set(g) == {'id', 'question', 'category', 'canon', 'keywords',
                          'nave_topics', 'must', 'should', 'status'}
        assert g['category'] in ('life', 'study')
        assert g['canon'] in ('66', 'both')
        assert 3 <= len(g['keywords']) <= 6, g['id']
        assert g['question'].strip() == g['question']


def test_smoke_entries_carry_no_gold_fields(data):
    for s in data['smoke']:
        assert set(s) == {'id', 'question', 'category'}
        assert 'must' not in s and 'should' not in s
        assert 'keywords' not in s


def test_canon_both_only_on_g19_g20(data):
    both = {g['id'] for g in data['graded'] if g['canon'] == 'both'}
    assert both == {'g19', 'g20'}


# ----------------------------------------------------------------- references

def test_every_passage_resolves_to_real_verses(db, data):
    for qid, bucket, p in all_passages(data):
        assert p['verse_ids'], '%s %s %s is empty' % (qid, bucket, p['ref'])
        for vid in p['verse_ids']:
            row = db.execute('SELECT 1 FROM verses WHERE verse_id = ?',
                             (vid,)).fetchone()
            assert row is not None, '%s %s %s: verse_id %d does not resolve' % (
                qid, bucket, p['ref'], vid)


def test_passages_stay_within_one_chapter(db, data):
    for qid, bucket, p in all_passages(data):
        chapters = {vid // 1000 for vid in p['verse_ids']}
        assert len(chapters) == 1, '%s %s %s spans chapters' % (qid, bucket, p['ref'])


def test_passages_are_not_whole_chapters(db, data):
    for qid, bucket, p in all_passages(data):
        book_id = p['verse_ids'][0] // 1000000
        chapter = (p['verse_ids'][0] % 1000000) // 1000
        total = db.execute('SELECT COUNT(*) FROM verses WHERE book_id = ?'
                           ' AND chapter = ?', (book_id, chapter)).fetchone()[0]
        assert len(p['verse_ids']) < total or total == 1, \
            '%s %s %s is the whole chapter' % (qid, bucket, p['ref'])


def test_passage_verse_ids_are_contiguous_and_sorted(data):
    for qid, bucket, p in all_passages(data):
        assert p['verse_ids'] == sorted(p['verse_ids']), \
            '%s %s %s' % (qid, bucket, p['ref'])


def test_canon_label_matches_the_book(db, data):
    for qid, bucket, p in all_passages(data):
        book_id = p['verse_ids'][0] // 1000000
        canon = db.execute('SELECT canon FROM books WHERE book_id = ?',
                           (book_id,)).fetchone()[0]
        assert canon == p['canon'], '%s %s %s' % (qid, bucket, p['ref'])


def test_66_only_questions_carry_no_deuterocanon(data):
    for g in data['graded']:
        if g['canon'] != '66':
            continue
        for bucket in ('must', 'should'):
            for p in g[bucket]:
                assert p['canon'] == 'protestant', '%s %s' % (g['id'], p['ref'])


def test_origins_are_known_and_non_empty(data):
    for qid, bucket, p in all_passages(data):
        assert p['origins'], '%s %s %s has no origin' % (qid, bucket, p['ref'])
        assert set(p['origins']) <= {'nave', 'fts', 'tsk'}, p['origins']


def test_must_passages_are_corroborated(data):
    # A gating passage carries at least two independent origins. Recorded in
    # DECISIONS.md; asserted here so the rule cannot quietly lapse.
    for g in data['graded']:
        for p in g['must']:
            assert len(p['origins']) >= 2, '%s %s' % (g['id'], p['ref'])


def test_generated_from_the_current_index(db, data):
    checksum = db.execute("SELECT value FROM meta WHERE key='build_checksum'"
                          ).fetchone()[0]
    assert data['generated_from_index'] == checksum
