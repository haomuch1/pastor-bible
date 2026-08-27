"""The P4 run artifacts hold the property the whole project rests on.

These read the committed metrics, not the models. The report is not trusted:
the numbers are re-derived here from the rows the runs wrote, so a mistake in
pipeline/report_p4.py cannot make a failing run look like a passing one.
"""

import json
import os

import pytest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RUNS = os.path.join(ROOT, 'data', 'eval', 'runs')
GRADED = 'p4-rust-8b'


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


@pytest.mark.parametrize('tag', [GRADED, 'p4-rust-1.7b', 'p4-rust-8b-deutero',
                                 'p4-rust-8b-ctx8448', 'p4-rust-8b-vulkan'])
def test_no_fabricated_reference_reached_a_reader(tag):
    """The hard gate of PLAN section 1, on every run this session made.

    The count is of things found in the text a reader would actually see: a
    [P#] token that was never sent, or any scripture reference written out in
    prose. Either one is a fabrication reaching output, whatever else the run
    reported.
    """
    for r in rows(tag):
        assert r['unsent_tokens_in_output'] == [], (
            '%s/%s put unsent tokens in front of a reader: %s'
            % (tag, r['question_id'], r['unsent_tokens_in_output']))
        assert r['written_references_in_output'] == [], (
            '%s/%s wrote references in prose: %s'
            % (tag, r['question_id'], r['written_references_in_output']))
        assert r['fabrications_reaching_output'] == 0


def test_the_graded_set_is_the_ten_p3_graded_questions():
    with open(os.path.join(ROOT, 'data', 'eval', 'questions.json'),
              encoding='utf-8') as fh:
        want = sorted(json.load(fh)['p3_graded'])
    got = sorted(r['question_id'] for r in rows(GRADED))
    assert got == want, 'the P4 graded run is not the P3 graded set'


def test_every_graded_answer_was_verified_not_fallen_back():
    """Not a requirement of the design, but a fact about this run.

    The fallback is a normal outcome and the app is built for it. Recording
    here that it never happened means a future run that starts falling back is
    visible rather than silent.
    """
    rs = rows(GRADED)
    assert all(r['verdict'] == 'ok' for r in rs)
    assert all(not r['fallback_used'] for r in rs)
    assert all(r['first_pass_verdict'] == 'ok' for r in rs)


def test_structure_held_on_every_graded_answer():
    rs = rows(GRADED)
    assert all(r['structure_ok'] for r in rs)
    assert all(2 <= r['themes'] <= 6 for r in rs), [r['themes'] for r in rs]


def test_every_cited_token_was_one_of_the_sent_ones():
    for tag in (GRADED, 'p4-rust-1.7b', 'p4-rust-8b-deutero'):
        for r in rows(tag):
            n = r['sent_passages']
            for tok in r['cited_tokens']:
                i = int(tok[2:-1])
                assert 1 <= i <= n, (
                    '%s/%s cited %s of %d sent' % (tag, r['question_id'], tok, n))


def test_both_canon_sent_deuterocanonical_passages_and_labelled_them():
    rs = rows('p4-rust-8b-deutero')
    assert rs, 'the both-canon run is missing'
    for r in rs:
        assert r['canon'] == 'both'
        # The additive slice means both-canon sends more than the canon-66 cut.
        assert r['sent_passages'] > 25, r['sent_passages']


def test_the_run_used_the_decided_query_mode():
    """STEP 6 chose the raw question. A run that quietly used another mode is
    not measuring what the app does."""
    assert all(r['query_mode'] == 'raw' for r in rows(GRADED))


def test_the_rewrite_decision_is_recorded_with_its_numbers():
    path = os.path.join(RUNS, 'rewrite_decision.json')
    if not os.path.exists(path):
        pytest.skip('rewrite decision not measured')
    with open(path, encoding='utf-8') as fh:
        d = json.load(fh)
    assert d['questions'] == 10
    for mode in ('raw', 'rewrite', 'fused', 'hand'):
        assert 0.0 <= d['means'][mode] <= 1.0
    # The default in the Rust pipeline must be the mode the graded run used.
    assert all(r['query_mode'] == 'raw' for r in rows(GRADED))
