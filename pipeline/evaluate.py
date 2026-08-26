"""Retrieval evaluation for P2.

Runs the harness over the approved graded questions and reports recall by
configuration. Reads index.db and questions.json; writes numbers to stdout and,
with --json, to a file the report generator consumes.

What "recall@k" means here, stated once so the numbers are readable:
candidates come back as ranked verses, are grouped into contiguous ranges
within a chapter, and the best k ranges are the retrieved passages, matching
PLAN 5.5's "top ~25 passages". A MUST passage counts as recalled if it shares
at least one verse with a retrieved passage. Recall is the fraction of MUST
passages recalled, averaged over questions.

Usage:
    python pipeline/evaluate.py --configs A,B,C --models bge-small-en-v1.5
"""

import argparse
import json
import os
import statistics
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)

from embed import Embedder, Reranker, normalize, peak_working_set_mb  # noqa: E402
from retrieve import CONFIGS, Retriever  # noqa: E402

QUESTIONS = os.path.join(ROOT, 'data', 'eval', 'questions.json')
RERANK_GGUF = 'bge-reranker-v2-m3-Q8_0.gguf'

MODEL_GGUF = {
    'bge-small-en-v1.5': ('bge-small-en-v1.5-f16.gguf', 512),
    'nomic-embed-text-v1.5': ('nomic-embed-text-v1.5-f16.gguf', 2048),
    'qwen3-embedding-0.6b': ('Qwen3-Embedding-0.6B-Q8_0.gguf', 2048),
}


def load_questions():
    with open(QUESTIONS, encoding='utf-8') as fh:
        return json.load(fh)


def query_text(q):
    """The question plus its stored keywords.

    PLAN 5.2 has the chat model produce search queries; that is P3. Until then
    the keyword lists approved with the gold set stand in, and are combined with
    the plain question so the vector paths see natural language too.
    """
    return q['question'] + ' ' + ' '.join(q['keywords'])


def recall_at(ranges, must, k):
    """Fraction of MUST passages sharing a verse with the top k ranges."""
    top = ranges[:k]
    hit = 0
    for p in must:
        want = set(p['verse_ids'])
        if any(want & set(r['ids']) for r in top):
            hit += 1
    return hit / len(must) if must else 0.0


def rank_of_passage(ranges, passage):
    want = set(passage['verse_ids'])
    for i, r in enumerate(ranges):
        if want & set(r['ids']):
            return i + 1
    return None


def run_question(ret, q, cfg_name, qvec, reranker=None, top_n=25, pool=100):
    cfg = dict(CONFIGS[cfg_name])
    canon_mode = '66' if q['canon'] == '66' else 'both'
    t0 = time.time()
    full, _, topics = ret.search(qvec, q['keywords'], canon_mode=canon_mode,
                                 top_n=top_n, pool=pool, **cfg)
    ranges = ret.as_ranges(full)

    if reranker is not None and cfg_name == 'G':
        head = ranges[:60]
        docs = []
        for r in head:
            texts = [t for _, t in ret.text_of(r['ids'])]
            docs.append('%s %s' % (r['ref'], ' '.join(texts))[:4000])
        scores = reranker.rank(q['question'], docs)
        for r, s in zip(head, scores):
            r['score'] = s
        head.sort(key=lambda r: -r['score'])
        ranges = head + ranges[60:]

    return ranges, topics, time.time() - t0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--configs', default='A,B,C,D,E,F,G')
    ap.add_argument('--models', default=','.join(MODEL_GGUF))
    ap.add_argument('--json', default='')
    ap.add_argument('--pool', type=int, default=100)
    args = ap.parse_args()

    data = load_questions()
    graded = data['graded']
    configs = args.configs.split(',')
    models = args.models.split(',')

    results = {}      # (model, cfg) -> {qid: {...}}
    fullsets = {}     # qid -> sizes under F
    timings = {}
    peak_ram = {}

    for model_id in models:
        gguf, ctx = MODEL_GGUF[model_id]
        ret = Retriever(model_id=model_id)
        need_rerank = 'G' in configs

        with Embedder(gguf, n_ctx=ctx) as emb:
            qvecs = {}
            for q in graded:
                v = emb.embed([ret.query_prefix + query_text(q)])[0]
                qvecs[q['id']] = normalize(v)
            peak_ram['embed:' + model_id] = peak_working_set_mb(emb.proc.pid)

        reranker = None
        rr_ctx = None
        if need_rerank:
            rr_ctx = Reranker(RERANK_GGUF, n_ctx=2048)
            reranker = rr_ctx.__enter__()

        try:
            for cfg_name in configs:
                per_q = {}
                for q in graded:
                    ranges, topics, dt = run_question(
                        ret, q, cfg_name, qvecs[q['id']], reranker,
                        pool=args.pool)
                    must = q['must']
                    entry = {
                        'r10': recall_at(ranges, must, 10),
                        'r25': recall_at(ranges, must, 25),
                        'r50': recall_at(ranges, must, 50),
                        'seconds': round(dt, 3),
                        'full_passages': len(ranges),
                        'full_verses': sum(len(r['ids']) for r in ranges),
                        'topics': [t['heading'] for t in topics],
                    }
                    if q['canon'] == 'both':
                        entry['deutero_ranks'] = {
                            p['ref']: rank_of_passage(ranges, p)
                            for p in must if p['canon'] == 'deutero'}
                        entry['top25_protestant'] = [
                            r['ref'] for r in ranges[:25]
                            if r['canon'] == 'protestant']
                    if cfg_name == 'F':
                        fullsets[q['id']] = {
                            'passages': len(ranges),
                            'verses': sum(len(r['ids']) for r in ranges),
                            'ranges': [(r['ref'], r['ids']) for r in ranges],
                        }
                    per_q[q['id']] = entry
                results['%s|%s' % (model_id, cfg_name)] = per_q
                mean25 = statistics.mean(e['r25'] for e in per_q.values())
                print('%-24s %s  recall@25 %.3f' % (model_id, cfg_name, mean25),
                      flush=True)
                timings['%s|%s' % (model_id, cfg_name)] = statistics.mean(
                    e['seconds'] for e in per_q.values())
        finally:
            if rr_ctx is not None:
                peak_ram['rerank'] = peak_working_set_mb(rr_ctx.proc.pid)
                rr_ctx.__exit__(None, None, None)

    out = {'results': results, 'fullsets': fullsets, 'timings': timings,
           'peak_ram_mb': peak_ram, 'pool': args.pool}
    if args.json:
        with open(args.json, 'w', encoding='utf-8', newline='\n') as fh:
            json.dump(out, fh, indent=1)
        print('wrote', args.json)


if __name__ == '__main__':
    main()
