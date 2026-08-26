"""Embed a fixed sample of verses twice and report how far the two runs differ.

Byte-identical output is not expected from a threaded CPU matmul: the order in
which partial sums are accumulated can vary between runs, and float addition is
not associative. What matters is that the variance is far below anything that
could reorder a retrieval result. This measures it rather than assuming it.

Usage:  python pipeline/check_determinism.py [model_id]
"""

import os
import sqlite3
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)

sys.stdout.reconfigure(encoding='utf-8')

from build_embeddings import MODELS, verse_rows  # noqa: E402
from embed import Embedder, normalize  # noqa: E402

DB = os.path.join(ROOT, 'src-tauri', 'resources', 'index.db')
SAMPLE = 200


def main():
    wanted = sys.argv[1] if len(sys.argv) > 1 else None
    con = sqlite3.connect('file:%s?mode=ro' % DB.replace('\\', '/'), uri=True)

    # A fixed sample: every 190th verse, so it spans the whole canon and is the
    # same set on every run.
    rows = list(verse_rows(con.cursor()))
    step = max(1, len(rows) // SAMPLE)
    sample = [t for _, t in rows[::step]][:SAMPLE]
    print('sample: %d verses, every %dth row' % (len(sample), step))

    for spec in MODELS:
        if wanted and spec['model_id'] != wanted:
            continue
        with Embedder(spec['gguf'], n_ctx=spec['n_ctx']) as e:
            texts = [e.fit(spec['doc_prefix'] + t) for t in sample]
            a = [normalize(v) for v in e.embed(texts)]
            b = [normalize(v) for v in e.embed(texts)]
        worst = 0.0
        worst_cos = 1.0
        for va, vb in zip(a, b):
            worst = max(worst, max(abs(x - y) for x, y in zip(va, vb)))
            worst_cos = min(worst_cos, sum(x * y for x, y in zip(va, vb)))
        identical = sum(1 for va, vb in zip(a, b) if va == vb)
        print('%-24s max abs diff %.3e   min cosine %.12f   identical %d/%d'
              % (spec['model_id'], worst, worst_cos, identical, len(a)))


if __name__ == '__main__':
    main()
