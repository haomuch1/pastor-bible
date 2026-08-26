"""P3 graded runs: one model, one question at a time, canon 66.

Order of operations per question, strictly sequential:

  1. query rewrite      chat model
  2. embed the queries  embedding server  (the chat server is stopped for this)
  3. retrieve           configuration F, canon 66
  4. synopsis           chat model
  5. verify             pipeline/verifier.py, one retry, then fallback
  6. write the row      before the next question begins

Two model processes are never alive at once. The embedding server runs first,
for every question in the set, and is stopped before the chat server starts;
which is recorded in the output as embed_phase_separate.

Usage:
  python pipeline/run_p3.py --model Qwen3-4B-Q4_K_M.gguf --tag qwen3-4b
  python pipeline/run_p3.py --model ... --questions smoke --ids s01,s02
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

sys.stdout.reconfigure(encoding='utf-8')

from chat import ChatServer, chat_prompt, free_ram_gb, load_prompt, prompt_version  # noqa: E402
from embed import Embedder, normalize  # noqa: E402
from retrieve import CONFIGS, Retriever  # noqa: E402
from verifier import Verifier  # noqa: E402

QUESTIONS = os.path.join(ROOT, 'data', 'eval', 'questions.json')
RUNS = os.path.join(ROOT, 'data', 'eval', 'runs')
EMBED_MODEL = 'nomic-embed-text-v1.5'
EMBED_GGUF = ('nomic-embed-text-v1.5-f16.gguf', 2048)
TOP_N = 25


def load_set():
    with open(QUESTIONS, encoding='utf-8') as fh:
        return json.load(fh)


def parse_queries(text, fallback):
    """Pull a JSON array of strings out of the rewrite output."""
    import re
    m = re.search(r'\[.*?\]', text, re.S)
    if m:
        try:
            got = json.loads(m.group(0))
            out = [str(x).strip() for x in got
                   if isinstance(x, (str, int, float)) and str(x).strip()]
            if out:
                return out[:5], True
        except Exception:  # noqa: BLE001
            pass
    # Fall back to lines that look like queries.
    lines = [l.strip(' -*"\'') for l in text.splitlines()]
    out = [l for l in lines if l and len(l.split()) <= 6 and not l.startswith('#')]
    if out:
        return out[:5], False
    return fallback, False


def render_passages(ret, ranges, canon_labels=True):
    """Passages as [P1] .. [Pn] with reference, text, and canon marker."""
    out, sent = [], []
    for i, r in enumerate(ranges, start=1):
        token = '[P%d]' % i
        texts = [t for _, t in ret.text_of(r['ids'])]
        marker = ''
        if canon_labels and r.get('canon') == 'deutero':
            marker = ' (Deuterocanon)'
        out.append('%s %s%s\n%s' % (token, r['ref'], marker, ' '.join(texts)))
        sent.append({'token': token, 'ref': r['ref'], 'verse_ids': r['ids'],
                     'canon': r.get('canon')})
    return '\n\n'.join(out), sent


def recall_at(ranges, must, k=TOP_N):
    if not must:
        return None
    top = ranges[:k]
    hit = sum(1 for p in must
              if set(p['verse_ids']) & {i for r in top for i in r['ids']})
    return hit / len(must)


def cited_tokens(text):
    import re
    return sorted(set(re.findall(r'\[P\d+\]', text)))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--model', required=True)
    ap.add_argument('--tag', required=True)
    ap.add_argument('--ids', default='')
    ap.add_argument('--questions', default='graded')
    ap.add_argument('--ctx', type=int, default=8192)
    ap.add_argument('--canon', default='66')
    args = ap.parse_args()

    data = load_set()
    if args.ids:
        want = args.ids.split(',')
    elif args.questions == 'graded':
        want = data['p3_graded']
    else:
        want = data['smoke_pool'][:10]

    by_id = {g['id']: g for g in data['graded']}
    by_id.update({s['id']: s for s in data['smoke']})
    questions = [by_id[i] for i in want]

    os.makedirs(os.path.join(RUNS, args.tag), exist_ok=True)
    out_path = os.path.join(RUNS, args.tag, 'metrics.jsonl')
    raw_dir = os.path.join(RUNS, args.tag, 'raw')
    os.makedirs(raw_dir, exist_ok=True)

    ret = Retriever(model_id=EMBED_MODEL)
    ver = Verifier()

    free_at_start = free_ram_gb()
    print('free RAM before any model load: %.2f GB' % free_at_start, flush=True)

    # ---- phase 1: the chat model writes the rewrites. Embedding server down.
    rows = []
    server = ChatServer(args.model, n_ctx=args.ctx)
    ok, free, need = server.safe_to_load()
    print('load check %s: %.1f GB needed, %.2f GB free -> %s'
          % (args.model, need, free, 'OK' if ok else 'REFUSED'), flush=True)
    if not ok:
        print('REFUSED: not loading %s' % args.model)
        return 2

    rewrites = {}
    rewrite_stats = {}
    with server as chat:
        print('chat server up, single slot, ctx %d' % args.ctx, flush=True)
        for q in questions:
            p = load_prompt('rewrite').replace('{question}', q['question'])
            r = chat.complete(chat_prompt(p), max_tokens=200)
            qs, was_json = parse_queries(r['text'], q.get('keywords', []))
            rewrites[q['id']] = qs
            rewrite_stats[q['id']] = {'seconds': r['seconds'],
                                      'json_clean': was_json,
                                      'raw': r['text'][:2000]}
            print('  rewrite %s -> %s' % (q['id'], qs), flush=True)
        peak_rewrite = chat.peak_ram_mb()
    print('chat server down for embedding phase', flush=True)

    # ---- phase 2: embed the queries. Chat server is stopped. Never both.
    qvecs, qvecs_hand = {}, {}
    with Embedder(EMBED_GGUF[0], n_ctx=EMBED_GGUF[1]) as emb:
        for q in questions:
            model_q = q['question'] + ' ' + ' '.join(rewrites[q['id']])
            qvecs[q['id']] = normalize(emb.embed([ret.query_prefix + model_q])[0])
            hand = q['question'] + ' ' + ' '.join(q.get('keywords', []))
            qvecs_hand[q['id']] = normalize(
                emb.embed([ret.query_prefix + hand])[0])
        peak_embed = emb.__dict__.get('proc') and __import__('embed').peak_working_set_mb(emb.proc.pid)
    print('embedding server down', flush=True)

    # ---- phase 3: retrieve, generate, verify. Chat server back up alone.
    server2 = ChatServer(args.model, n_ctx=args.ctx)
    with server2 as chat:
        for q in questions:
            t_start = time.time()
            qid = q['id']
            cfg = dict(CONFIGS['F'])

            t0 = time.time()
            full, _, topics = ret.search(qvecs[qid], rewrites[qid],
                                         canon_mode=args.canon, top_n=TOP_N,
                                         **cfg)
            ranges = ret.as_ranges(full)
            t_retrieve = time.time() - t0

            must = q.get('must') or []
            r_model = recall_at(ranges, must)
            fullh, _, _ = ret.search(qvecs_hand[qid], q.get('keywords', []),
                                     canon_mode=args.canon, top_n=TOP_N, **cfg)
            r_hand = recall_at(ret.as_ranges(fullh), must)

            top = ranges[:TOP_N]
            passages_text, sent = render_passages(ret, top)

            p = (load_prompt('synopsis')
                 .replace('{question}', q['question'])
                 .replace('{passages}', passages_text))
            t0 = time.time()
            gen = chat.complete(chat_prompt(p), max_tokens=900)
            t_gen = time.time() - t0

            verdict, violations = ver.check(gen['text'], sent)
            attempt2 = None
            final_text = gen['text']
            fallback_used = False
            if verdict == 'violation':
                note = ver.failure_note(violations)
                p2 = (load_prompt('retry')
                      .replace('{failure}', note)
                      .replace('{question}', q['question'])
                      .replace('{passages}', passages_text))
                t0 = time.time()
                gen2 = chat.complete(chat_prompt(p2), max_tokens=900)
                t_retry = time.time() - t0
                v2, viol2 = ver.check(gen2['text'], sent)
                attempt2 = {'verdict': v2, 'seconds': gen2['seconds'],
                            'violations': [dict(v, span=list(v['span']))
                                           for v in viol2],
                            'completion_tokens': gen2['completion_tokens']}
                if v2 == 'ok':
                    final_text = gen2['text']
                else:
                    final_text = ver.fallback(sent)
                    fallback_used = True
            else:
                t_retry = 0.0

            toks = cited_tokens(final_text) if not fallback_used else []
            sent_by_token = {s['token']: s for s in sent}
            must_ids = [set(p['verse_ids']) for p in must]
            cited_ids = set()
            for t in toks:
                if t in sent_by_token:
                    cited_ids.update(sent_by_token[t]['verse_ids'])
            should_ids = set()
            for p_ in (q.get('should') or []):
                should_ids.update(p_['verse_ids'])
            gold_ids = set().union(*must_ids) if must_ids else set()
            precision = None
            if toks:
                good = sum(1 for t in toks
                           if set(sent_by_token[t]['verse_ids'])
                           & (gold_ids | should_ids))
                precision = good / len(toks)
            coverage = None
            if must:
                sent_ids = {i for s in sent for i in s['verse_ids']}
                present = [m for m in must_ids if m & sent_ids]
                if present:
                    coverage = sum(1 for m in present if m & cited_ids) / len(present)

            headings = [l for l in final_text.splitlines()
                        if l.strip().startswith('##')]
            # The prompt asks for 2 to 5 themes. One heading followed by a
            # passage-by-passage paraphrase is not a themed synopsis, so the
            # count is part of the check rather than mere presence.
            themes_ok = (2 <= len(headings) <= 6) and not fallback_used
            if themes_ok:
                blocks, cur = [], []
                for line in final_text.splitlines():
                    if line.strip().startswith('##'):
                        if cur:
                            blocks.append('\n'.join(cur))
                        cur = [line]
                    elif cur:
                        cur.append(line)
                if cur:
                    blocks.append('\n'.join(cur))
                themes_ok = all(cited_tokens(b) for b in blocks)

            row = {
                'model': args.tag, 'question_id': qid, 'canon': args.canon,
                'rewrites': rewrites[qid],
                'rewrite_json_clean': rewrite_stats[qid]['json_clean'],
                'rewrite_seconds': rewrite_stats[qid]['seconds'],
                'recall25_model_rewrites': r_model,
                'recall25_hand_keywords': r_hand,
                'retrieve_seconds': round(t_retrieve, 3),
                'full_set_passages': len(ranges),
                'sent_passages': len(sent),
                'gen_seconds': round(t_gen, 2),
                'retry_seconds': round(t_retry, 2),
                'prompt_tokens': gen['prompt_tokens'],
                'completion_tokens': gen['completion_tokens'],
                'tokens_per_second': gen['predicted_per_second'],
                'first_pass_verdict': verdict,
                'first_pass_violations': [dict(v, span=list(v['span']))
                                          for v in violations],
                'retry': attempt2,
                'fallback_used': fallback_used,
                'citation_precision': precision,
                'citation_coverage': coverage,
                'cited_tokens': toks,
                'themes': len(headings),
                'structure_ok': themes_ok,
                'end_to_end_seconds': round(time.time() - t_start, 2),
                'topics': [t['heading'] for t in topics],
            }
            rows.append(row)
            with open(out_path, 'a', encoding='utf-8', newline='\n') as fh:
                fh.write(json.dumps(row, ensure_ascii=False) + '\n')
            with open(os.path.join(raw_dir, '%s.json' % qid), 'w',
                      encoding='utf-8', newline='\n') as fh:
                json.dump({'question': q['question'], 'passages': passages_text,
                           'first_pass': gen['text'], 'final': final_text,
                           'sent': sent}, fh, indent=1, ensure_ascii=False)
            print('  %s  recall %.2f/%.2f  %s%s  %.1fs'
                  % (qid, r_model or 0, r_hand or 0, verdict,
                     ' -> ' + attempt2['verdict'] if attempt2 else '',
                     row['end_to_end_seconds']), flush=True)
        peak_chat = chat.peak_ram_mb()

    summary = {
        'model': args.tag, 'gguf': args.model, 'canon': args.canon,
        'questions': len(rows),
        'peak_ram_chat_mb': max(x for x in [peak_chat, peak_rewrite] if x),
        'peak_ram_embed_mb': peak_embed,
        'free_ram_before_gb': free,
        'embed_phase_separate': True,
        'prompt_versions': {n: prompt_version(n)
                            for n in ('rewrite', 'synopsis', 'retry')},
        'first_pass_violation_rate': sum(
            1 for r in rows if r['first_pass_verdict'] != 'ok') / len(rows),
        'fallback_rate': sum(1 for r in rows if r['fallback_used']) / len(rows),
        'structure_ok_rate': sum(1 for r in rows if r['structure_ok']) / len(rows),
        'median_end_to_end_s': statistics.median(
            r['end_to_end_seconds'] for r in rows),
        'max_end_to_end_s': max(r['end_to_end_seconds'] for r in rows),
    }
    with open(os.path.join(RUNS, args.tag, 'summary.json'), 'w',
              encoding='utf-8', newline='\n') as fh:
        json.dump(summary, fh, indent=1)
    print(json.dumps(summary, indent=1))
    return 0


if __name__ == '__main__':
    sys.exit(main())
