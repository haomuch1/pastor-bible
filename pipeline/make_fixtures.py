"""Generate the parity fixtures the Rust port is tested against.

The Rust backend must reproduce this harness exactly: the same ranked passage
list, the same scores, the same origin tags, the same matched topics, and the
same verifier verdicts and violation records. That claim is only worth anything
if the Python side is pinned, so it is pinned here, as committed JSON, and the
Rust tests read these files rather than a live Python process.

Three fixtures are written:

  verifier_vectors.json   the 35 contract vectors with the sent set and the
                          exact violation records Python produces
  p3_verifier.json        every model output P3 stored, with its sent set and
                          Python's verdict, so the port is checked against real
                          model prose and not only against the contract
  retrieval/<case>.json   query vector, keywords and the full retrieval result
                          for the graded questions

Run:  python pipeline/make_fixtures.py
"""

import io
import json
import os
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)

from chat import load_prompt, prompt_version  # noqa: E402
from embed import Embedder, normalize  # noqa: E402
from retrieve import CONFIGS, Retriever  # noqa: E402
from verifier import TEST_VECTORS, Verifier, sent_set  # noqa: E402

OUT = os.path.join(ROOT, 'src-tauri', 'core', 'tests', 'fixtures')
QUESTIONS = os.path.join(ROOT, 'data', 'eval', 'questions.json')
RUNS = os.path.join(ROOT, 'data', 'eval', 'runs')
EMBED_MODEL = 'nomic-embed-text-v1.5'
EMBED_GGUF = ('nomic-embed-text-v1.5-f16.gguf', 2048)
TOP_N = 25
DEUT_N = 8


def write(path, obj):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with io.open(path, 'w', encoding='utf-8', newline='\n') as fh:
        json.dump(obj, fh, indent=1, ensure_ascii=False)
        fh.write('\n')
    print('wrote %s (%.1f KB)' % (os.path.relpath(path, ROOT),
                                  os.path.getsize(path) / 1024.0))


def verifier_fixtures():
    v = Verifier()
    sent = sent_set(v.con)
    cases = []
    for i, (text, expected) in enumerate(TEST_VECTORS, start=1):
        verdict, violations = v.check(text, sent)
        assert verdict == expected, 'vector %d does not hold in Python' % i
        cases.append({
            'n': i, 'text': text, 'expected': expected,
            'violations': [{'kind': x['kind'], 'text': x['text'],
                            'reason': x['reason'], 'span': list(x['span'])}
                           for x in violations],
        })
    write(os.path.join(OUT, 'verifier_vectors.json'),
          {'sent': [{'token': p['token'], 'ref': p['ref'],
                     'verse_ids': p['verse_ids']} for p in sent],
           'vectors': cases})

    rows = []
    for tag in sorted(os.listdir(RUNS)):
        mp = os.path.join(RUNS, tag, 'metrics.jsonl')
        if not os.path.exists(mp):
            continue
        for line in io.open(mp, encoding='utf-8'):
            row = json.loads(line)
            raw = os.path.join(RUNS, tag, 'raw', '%s.json' % row['question_id'])
            if not os.path.exists(raw):
                print('  missing raw output: %s/%s' % (tag, row['question_id']))
                continue
            d = json.load(io.open(raw, encoding='utf-8'))
            if 'first_pass' not in d or 'sent' not in d:
                # A P4 run stores the API answer, not P3's dump. Those are
                # checked by the Rust pipeline's own tests, not here.
                continue
            for stage in ('first_pass', 'final'):
                verdict, violations = v.check(d[stage], d['sent'])
                rows.append({
                    'run': tag, 'question_id': row['question_id'],
                    'stage': stage,
                    'recorded_verdict': (row['first_pass_verdict']
                                         if stage == 'first_pass' else None),
                    'text': d[stage],
                    'sent': [{'token': s['token'], 'ref': s['ref'],
                              'verse_ids': s['verse_ids']} for s in d['sent']],
                    'verdict': verdict,
                    'violations': [{'kind': x['kind'], 'text': x['text'],
                                    'reason': x['reason'],
                                    'span': list(x['span'])}
                                   for x in violations],
                    'stripped': v.strip(d[stage], violations),
                    'failure_note': v.failure_note(violations),
                    'fallback': v.fallback(d['sent']),
                })
    write(os.path.join(OUT, 'p3_verifier.json'), {'rows': rows})
    return len(cases), len(rows)


def prompt_fixtures():
    """The prompt bodies as the Python harness strips them.

    The Rust loader has to drop the same header and keep the same body, byte
    for byte: an instruction lost at the top of the prompt is a behaviour
    change nobody would see in a diff.
    """
    out = {}
    for name in ('synopsis', 'retry', 'rewrite', 'summarize_batch',
                 'summarize_merge'):
        out[name] = {'version': prompt_version(name), 'body': load_prompt(name)}
    write(os.path.join(OUT, 'prompts.json'), out)
    return out


def retrieval_fixtures():
    data = json.load(io.open(QUESTIONS, encoding='utf-8'))
    by_id = {g['id']: g for g in data['graded']}
    ret = Retriever(model_id=EMBED_MODEL)

    cases = [(qid, '66') for qid in data['p3_graded']]
    cases += [('g19', '66'), ('g20', '66'), ('g19', 'both'), ('g20', 'both')]

    want = sorted({qid for qid, _ in cases})
    qvecs = {}
    with Embedder(EMBED_GGUF[0], n_ctx=EMBED_GGUF[1]) as emb:
        for qid in want:
            q = by_id[qid]['question']
            qvecs[qid] = normalize(emb.embed([ret.query_prefix + q])[0])
    print('embedding server down')

    cfg = dict(CONFIGS['F'])
    index = []
    for qid, canon in cases:
        q = by_id[qid]
        t0 = time.time()
        full, top, topics = ret.search(qvecs[qid], q['keywords'],
                                       canon_mode=canon, top_n=TOP_N,
                                       deutero_slice=DEUT_N, **cfg)
        ranges = ret.as_ranges(full)
        seconds = time.time() - t0
        if canon == 'both':
            prot = [r for r in ranges if r['canon'] == 'protestant'][:TOP_N]
            deut = [r for r in ranges if r['canon'] == 'deutero'][:DEUT_N]
            cut = prot + deut
        else:
            cut = ranges[:TOP_N]
        name = '%s-%s' % (qid, canon)
        write(os.path.join(OUT, 'retrieval', '%s.json' % name), {
            'case': name,
            'question_id': qid,
            'question': q['question'],
            'canon': canon,
            'keywords': q['keywords'],
            'query_text': ret.query_prefix + q['question'],
            'embedding_model': EMBED_MODEL,
            'qvec': qvecs[qid],
            'top_n': TOP_N,
            'deutero_slice': DEUT_N,
            'python_seconds': round(seconds, 4),
            'full_set': [{'verse_id': c['verse_id'], 'score': c['score'],
                          'origins': c['origins'], 'canon': c['canon']}
                         for c in full],
            'topics': topics,
            'ranges': [{'ref': r['ref'], 'ids': r['ids'], 'score': r['score'],
                        'origins': r['origins'], 'canon': r['canon']}
                       for r in ranges],
            'cut': [{'ref': r['ref'], 'ids': r['ids'], 'score': r['score'],
                     'origins': r['origins'], 'canon': r['canon']}
                    for r in cut],
        })
        index.append({'case': name, 'question_id': qid, 'canon': canon,
                      'full_set': len(full), 'ranges': len(ranges),
                      'cut': len(cut), 'python_seconds': round(seconds, 4)})
    write(os.path.join(OUT, 'retrieval', 'index.json'), {'cases': index})
    return index


def main():
    only = sys.argv[1] if len(sys.argv) > 1 else 'all'
    n_vec, n_rows = verifier_fixtures()
    print('verifier: %d contract vectors, %d P3 output rows' % (n_vec, n_rows))
    p = prompt_fixtures()
    print('prompts: %s' % ', '.join('%s v%s' % (k, v['version']) for k, v in p.items()))
    if only == 'no-model':
        # The retrieval fixtures need the embedding server, and no model may be
        # loaded while another run is in progress.
        print('skipping the retrieval fixtures: no model may be loaded now')
        return
    idx = retrieval_fixtures()
    print('retrieval: %d cases' % len(idx))
    for c in idx:
        print('  %-9s full %5d  ranges %4d  cut %3d  %.3fs'
              % (c['case'], c['full_set'], c['ranges'], c['cut'],
                 c['python_seconds']))


if __name__ == '__main__':
    sys.stdout.reconfigure(encoding='utf-8')
    main()
