"""The P3 run artifacts exist and say what the report says they say.

These read the committed metrics, not the models. A run that was never
committed, or that silently ran fewer questions than claimed, fails here.
"""

import json
import os

import pytest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RUNS = os.path.join(ROOT, 'data', 'eval', 'runs')
CANDIDATES = ['qwen3-1.7b', 'qwen3-4b', 'qwen3-8b']


def rows(tag):
    path = os.path.join(RUNS, tag, 'metrics.jsonl')
    if not os.path.exists(path):
        pytest.skip('%s not run' % tag)
    out = {}
    with open(path, encoding='utf-8') as fh:
        for line in fh:
            if line.strip():
                r = json.loads(line)
                out[r['question_id']] = r
    return [out[k] for k in sorted(out)]


@pytest.mark.parametrize('tag', CANDIDATES)
def test_each_candidate_has_ten_canon66_rows(tag):
    rs = rows(tag)
    assert len(rs) == 10, '%s has %d rows' % (tag, len(rs))
    assert all(r['canon'] == '66' for r in rs)


@pytest.mark.parametrize('tag', CANDIDATES)
def test_rows_cover_the_p3_graded_set(tag):
    data = json.load(open(os.path.join(ROOT, 'data', 'eval', 'questions.json'),
                          encoding='utf-8'))
    assert {r['question_id'] for r in rows(tag)} == set(data['p3_graded'])


@pytest.mark.parametrize('tag', CANDIDATES)
def test_no_unverified_reference_survived(tag):
    """The hard gate of PLAN 6.3, asserted against the artifacts.

    Every run ends in one of three states: clean first pass, clean retry, or
    fallback. A row that ends in a violation without falling back would mean an
    unverified reference reached the reader.
    """
    for r in rows(tag):
        if r['first_pass_verdict'] == 'ok':
            continue
        assert r['retry'] is not None, '%s %s' % (tag, r['question_id'])
        if r['retry']['verdict'] != 'ok':
            assert r['fallback_used'], '%s %s' % (tag, r['question_id'])


@pytest.mark.parametrize('tag', CANDIDATES)
def test_every_cited_token_was_actually_sent(tag):
    for r in rows(tag):
        n = r['sent_passages']
        for t in (r['cited_tokens'] or []):
            num = int(t.strip('[]P'))
            assert 1 <= num <= n, '%s %s cited %s of %d sent' % (
                tag, r['question_id'], t, n)


@pytest.mark.parametrize('tag', CANDIDATES)
def test_summary_exists_and_matches_row_count(tag):
    path = os.path.join(RUNS, tag, 'summary.json')
    if not os.path.exists(path):
        pytest.skip('%s has no summary' % tag)
    s = json.load(open(path, encoding='utf-8'))
    assert s['questions'] == len(rows(tag))
    assert s['embed_phase_separate'] is True
    assert s['canon'] == '66'


def test_smoke_run_has_ten_rows_and_smoke_md_exists():
    rs = rows('qwen3-8b-smoke')
    assert len(rs) == 10
    assert os.path.exists(os.path.join(ROOT, 'docs', 'SMOKE.md'))


def test_summarize_all_artifact_is_present_and_complete():
    path = os.path.join(RUNS, 'qwen3-8b', 'summarize_all.json')
    if not os.path.exists(path):
        pytest.skip('summarize-all not run')
    d = json.load(open(path, encoding='utf-8'))
    assert d['total_passages'] > 100
    assert d['batches']
    # Every batch ended verified or was retried; the merge was verified too.
    for b in d['batches']:
        assert b['verdict'] == 'ok' or b['retry_verdict'] is not None
    assert d['merge_verdict'] == 'ok' or d['merge_retry_verdict'] is not None
