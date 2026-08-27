"""STEP 6: does the query rewrite earn its place?

P3 measured that every model's rewrites lowered recall@25 against the
hand-written keyword lists in questions.json. That is a true finding about the
wrong comparison: hand-written keyword lists do not exist at run time. The
alternative to a rewrite is the reader's own question, so that is what this
measures.

Three modes, configuration F, canon 66, the ten P3 graded questions, recall@25
against the MUST lists:

  raw      the question embedded on its own; keyword terms are its content
           words, because a whole question used as one FTS term is quoted as a
           phrase and matches nothing
  rewrite  what P3 actually ran: question plus rewrites embedded together,
           keyword terms are the rewrites
  fused    the same vector as rewrite, keyword terms are the content words and
           the rewrites together

The rewrites are P3's own stored output for the selected model, so no
generation runs here. Retrieval only.

Run:  python pipeline/rewrite_decision.py
"""

import io
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)

from embed import Embedder, normalize  # noqa: E402
from retrieve import CONFIGS, Retriever  # noqa: E402

QUESTIONS = os.path.join(ROOT, 'data', 'eval', 'questions.json')
RUNS = os.path.join(ROOT, 'data', 'eval', 'runs')
EMBED_MODEL = 'nomic-embed-text-v1.5'
EMBED_GGUF = ('nomic-embed-text-v1.5-f16.gguf', 2048)
TOP_N = 25
RUST = os.path.join(ROOT, 'src-tauri', 'core', 'src', 'pipeline.rs')


def rust_stopwords():
    """Read the stopword list out of the Rust source.

    One list, one place. If the two ever differ the raw mode measured here is
    not the raw mode the app runs, and this measurement would be describing
    something that does not ship.
    """
    text = io.open(RUST, encoding='utf-8').read()
    m = re.search(r'const STOPWORDS: &\[&str\] = &\[(.*?)\];', text, re.S)
    if not m:
        raise RuntimeError('STOPWORDS not found in %s' % RUST)
    return set(re.findall(r'"([^"]+)"', m.group(1)))


STOPWORDS = rust_stopwords()


def question_terms(question):
    """Mirrors pastor_bible_core::pipeline::question_terms."""
    out = []
    for word in re.split(r"[^0-9A-Za-z']+", question):
        w = word.strip("'").lower()
        if len(w) < 3 or w in STOPWORDS or w in out:
            continue
        out.append(w)
    return out


def recall_at(ranges, must, k=TOP_N):
    if not must:
        return None
    top = ranges[:k]
    ids = {i for r in top for i in r['ids']}
    hit = sum(1 for p in must if set(p['verse_ids']) & ids)
    return hit / len(must)


def stored_rewrites(tag):
    path = os.path.join(RUNS, tag, 'metrics.jsonl')
    out = {}
    for line in io.open(path, encoding='utf-8'):
        row = json.loads(line)
        out[row['question_id']] = row['rewrites']
    return out


def main():
    sys.stdout.reconfigure(encoding='utf-8')
    tag = sys.argv[1] if len(sys.argv) > 1 else 'qwen3-8b'
    data = json.load(io.open(QUESTIONS, encoding='utf-8'))
    by_id = {g['id']: g for g in data['graded']}
    ids = data['p3_graded']
    rewrites = stored_rewrites(tag)
    ret = Retriever(model_id=EMBED_MODEL)

    # Two distinct texts per question; embed them all in one server session.
    texts, keys = [], []
    for qid in ids:
        q = by_id[qid]['question']
        texts.append(ret.query_prefix + q)
        keys.append((qid, 'raw'))
        texts.append(ret.query_prefix + q + ' ' + ' '.join(rewrites[qid]))
        keys.append((qid, 'rw'))
    with Embedder(EMBED_GGUF[0], n_ctx=EMBED_GGUF[1]) as emb:
        vecs = [normalize(v) for v in emb.embed(texts)]
    print('embedding server down')
    vec = {k: v for k, v in zip(keys, vecs)}

    cfg = dict(CONFIGS['F'])
    rows = []
    for qid in ids:
        q = by_id[qid]
        must = q.get('must') or []
        modes = {
            'raw': (vec[(qid, 'raw')], question_terms(q['question'])),
            'rewrite': (vec[(qid, 'rw')], rewrites[qid]),
            'fused': (vec[(qid, 'rw')], question_terms(q['question']) + rewrites[qid]),
            # Reported for context only. Hand keywords are not available at run
            # time and are not a candidate; they are what P3 compared against.
            'hand': (vec[(qid, 'raw')], q.get('keywords', [])),
        }
        row = {'id': qid}
        for name, (qv, kw) in modes.items():
            full, _, _ = ret.search(qv, kw, canon_mode='66', top_n=TOP_N, **cfg)
            row[name] = recall_at(ret.as_ranges(full), must)
            row[name + '_terms'] = len(kw)
        rows.append(row)
        print('  %-4s raw %.3f  rewrite %.3f  fused %.3f  (hand %.3f)'
              % (qid, row['raw'], row['rewrite'], row['fused'], row['hand']))

    print()
    print('recall@25 against MUST, canon 66, configuration F, %d questions' % len(rows))
    means = {}
    for name in ('raw', 'rewrite', 'fused', 'hand'):
        means[name] = sum(r[name] for r in rows) / len(rows)
        print('  %-8s %.4f' % (name, means[name]))
    print()
    wins = {n: sum(1 for r in rows if r[n] >= max(r['raw'], r['rewrite'], r['fused']))
            for n in ('raw', 'rewrite', 'fused')}
    print('questions at or above the best of the three: %s' % wins)
    best = max(('raw', 'rewrite', 'fused'), key=lambda n: (means[n], n == 'raw'))
    if means['raw'] >= max(means['rewrite'], means['fused']):
        best = 'raw'   # ties go to raw, per the session brief
    print('choice: %s' % best)

    out = {'model_run': tag, 'questions': len(rows), 'means': means,
           'wins': wins, 'choice': best, 'rows': rows}
    path = os.path.join(RUNS, 'rewrite_decision.json')
    with io.open(path, 'w', encoding='utf-8', newline='\n') as fh:
        json.dump(out, fh, indent=1)
    print('wrote %s' % os.path.relpath(path, ROOT))
    return 0


if __name__ == '__main__':
    sys.exit(main())
