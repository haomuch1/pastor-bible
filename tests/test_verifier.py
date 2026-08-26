"""The citation verifier's contract, from docs/VERIFIER.md.

These vectors are shared with the Rust port in P4. A change here is a change to
the promise the project makes about references, and must be made in both
implementations at once.
"""

import os
import sys

import pytest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, 'pipeline'))
DB = os.path.join(ROOT, 'src-tauri', 'resources', 'index.db')

if not os.path.exists(DB):
    pytest.skip('index.db not built', allow_module_level=True)

from verifier import TEST_VECTORS, Verifier, run_vectors, sent_set  # noqa: E402

RESULTS = run_vectors(DB)


@pytest.mark.parametrize('i,text,expected,got,violations', RESULTS)
def test_vector(i, text, expected, got, violations):
    assert got == expected, '%d: %r -> %s (%s)' % (
        i, text, got, [v['text'] for v in violations])


def test_all_twenty_five_vectors_present():
    assert len(TEST_VECTORS) == 25


def test_stripping_removes_only_the_offending_span():
    v = Verifier(DB)
    passages = sent_set(v.con)
    text = 'Trust God [P1], and also see [P9] for more.'
    verdict, violations = v.check(text, passages)
    assert verdict == 'violation'
    stripped = v.strip(text, violations)
    assert '[P9]' not in stripped
    assert '[P1]' in stripped
    assert 'Trust God' in stripped


def test_failure_note_names_the_offender():
    v = Verifier(DB)
    passages = sent_set(v.con)
    _, violations = v.check('See [P9] and John 3:16.', passages)
    note = v.failure_note(violations)
    assert '[P9]' in note
    assert 'John 3:16' in note


def test_fallback_groups_by_book_and_carries_the_note():
    v = Verifier(DB)
    passages = sent_set(v.con)
    out = v.fallback(passages)
    assert 'synthesis could not be produced' in out
    for p in passages:
        assert p['ref'].split()[0] in out or p['ref'] in out
