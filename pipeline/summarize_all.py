"""PLAN 5.6's summarize-all path, measured once on the largest retrieved set.

The default answer is a synopsis over the top ~25 passages. This is the other
mode: a themed summary of the *entire* retrieved set, built in batches grouped
by book and then merged. The citation verifier runs on every batch and on the
merge, so the guarantee is unchanged by the batching.

Batches run one at a time. One model, one slot, one request.

Usage:
  python pipeline/summarize_all.py --model Qwen3-8B-Q4_K_M.gguf --tag qwen3-8b
"""

import argparse
import json
import os
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)

sys.stdout.reconfigure(encoding='utf-8')

from chat import ChatServer, chat_prompt, load_prompt  # noqa: E402
from embed import Embedder, normalize  # noqa: E402
from retrieve import CONFIGS, Retriever  # noqa: E402
from run_p3 import RUNS, cited_tokens, load_set, render_passages  # noqa: E402
from verifier import Verifier  # noqa: E402

EMBED_GGUF = ('nomic-embed-text-v1.5-f16.gguf', 2048)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--model', required=True)
    ap.add_argument('--tag', required=True)
    ap.add_argument('--ctx', type=int, default=16384)
    ap.add_argument('--batch-tokens', type=int, default=6000,
                    help='approximate passage tokens per batch, leaving the '
                         'rest of the context for the instructions and output')
    args = ap.parse_args()

    data = load_set()
    ret = Retriever(model_id='nomic-embed-text-v1.5')
    ver = Verifier()

    # Which graded question has the largest full set under F?
    with Embedder(EMBED_GGUF[0], n_ctx=EMBED_GGUF[1]) as emb:
        sizes = {}
        qvecs = {}
        for qid in data['p3_graded']:
            q = next(g for g in data['graded'] if g['id'] == qid)
            text = q['question'] + ' ' + ' '.join(q['keywords'])
            v = normalize(emb.embed([ret.query_prefix + text])[0])
            qvecs[qid] = v
            full, _, _ = ret.search(v, q['keywords'], canon_mode='66',
                                    top_n=25, **CONFIGS['F'])
            sizes[qid] = len(ret.as_ranges(full))
    target = max(sizes, key=lambda k: sizes[k])
    q = next(g for g in data['graded'] if g['id'] == target)
    print('largest full set: %s with %d passages' % (target, sizes[target]))
    print('sizes: %s' % sizes)

    full, _, _ = ret.search(qvecs[target], q['keywords'], canon_mode='66',
                            top_n=25, **CONFIGS['F'])
    ranges = ret.as_ranges(full)
    passages_text, sent = render_passages(ret, ranges)
    by_token = {s['token']: s for s in sent}

    # Group by book, then split any book group that is too large for a batch.
    books = {}
    for s in sent:
        books.setdefault(s['verse_ids'][0] // 1000000, []).append(s)

    server = ChatServer(args.model, n_ctx=args.ctx)
    ok, free, need = server.safe_to_load()
    print('load check: %.1f GB needed, %.2f GB free -> %s'
          % (need, free, 'OK' if ok else 'REFUSED'))
    if not ok:
        return 2

    result = {'question_id': target, 'question': q['question'],
              'total_passages': len(sent), 'batches': [], 'ctx': args.ctx,
              'batch_token_budget': args.batch_tokens}

    # The embedding server was already stopped above, before this point. Only
    # the chat server runs from here on: never two model processes at once.
    t_all = time.time()
    with server as chat:
        # Build batches: whole books where they fit, split where they do not.
        # ~4 characters per token is the working estimate for English prose.
        budget_chars = args.batch_tokens * 4
        batches, cur, cur_chars = [], [], 0
        for bid in sorted(books):
            for s in books[bid]:
                text = ' '.join(t for _, t in ret.text_of(s['verse_ids']))
                size = len(text) + len(s['ref']) + 16
                if cur and cur_chars + size > budget_chars:
                    batches.append(cur)
                    cur, cur_chars = [], 0
                cur.append(s)
                cur_chars += size
        if cur:
            batches.append(cur)
        print('batches: %d (grouped by book, split at %d chars)'
              % (len(batches), budget_chars))

        partials = []
        for i, batch in enumerate(batches, start=1):
            lines = []
            for s in batch:
                txt = ' '.join(t for _, t in ret.text_of(s['verse_ids']))
                mark = ' (Deuterocanon)' if s.get('canon') == 'deutero' else ''
                lines.append('%s %s%s\n%s' % (s['token'], s['ref'], mark, txt))
            btext = '\n\n'.join(lines)
            p = (load_prompt('summarize_batch')
                 .replace('{question}', q['question'])
                 .replace('{passages}', btext))
            t0 = time.time()
            gen = chat.complete(chat_prompt(p), max_tokens=700)
            dt = time.time() - t0
            verdict, viol = ver.check(gen['text'], batch)
            retry_verdict = None
            text = gen['text']
            if verdict == 'violation':
                p2 = (load_prompt('retry')
                      .replace('{failure}', ver.failure_note(viol))
                      .replace('{question}', q['question'])
                      .replace('{passages}', btext))
                gen2 = chat.complete(chat_prompt(p2), max_tokens=700)
                retry_verdict, viol2 = ver.check(gen2['text'], batch)
                if retry_verdict == 'ok':
                    text = gen2['text']
                else:
                    text = ''
            partials.append(text)
            result['batches'].append({
                'index': i, 'passages': len(batch), 'seconds': round(dt, 1),
                'verdict': verdict, 'retry_verdict': retry_verdict,
                'violations': [v['text'] for v in viol],
                'tokens_out': gen['completion_tokens'],
                'cited': cited_tokens(text),
            })
            print('  batch %2d/%d  %2d passages  %s%s  %.0fs'
                  % (i, len(batches), len(batch), verdict,
                     ' -> ' + retry_verdict if retry_verdict else '', dt),
                  flush=True)

        # Merge. The verifier runs here too: the merge can invent a token.
        merged_input = '\n\n'.join(
            '--- part %d ---\n%s' % (i + 1, p) for i, p in enumerate(partials) if p)
        p = (load_prompt('summarize_merge')
             .replace('{question}', q['question'])
             .replace('{summaries}', merged_input))
        t0 = time.time()
        gen = chat.complete(chat_prompt(p), max_tokens=1200)
        t_merge = time.time() - t0
        mverdict, mviol = ver.check(gen['text'], sent)
        merge_text = gen['text']
        mretry = None
        if mverdict == 'violation':
            p2 = (load_prompt('retry')
                  .replace('{failure}', ver.failure_note(mviol))
                  .replace('{question}', q['question'])
                  .replace('{passages}', merged_input))
            gen2 = chat.complete(chat_prompt(p2), max_tokens=1200)
            mretry, mviol2 = ver.check(gen2['text'], sent)
            merge_text = gen2['text'] if mretry == 'ok' else ver.fallback(sent)
        peak = chat.peak_ram_mb()

    batch_cited = set()
    for b in result['batches']:
        batch_cited.update(b['cited'])
    merge_cited = set(cited_tokens(merge_text))
    result.update({
        'merge_seconds': round(t_merge, 1),
        'merge_verdict': mverdict,
        'merge_retry_verdict': mretry,
        'merge_violations': [v['text'] for v in mviol],
        'total_seconds': round(time.time() - t_all, 1),
        'final_chars': len(merge_text),
        'tokens_cited_by_batches': len(batch_cited),
        'tokens_carried_to_merge': len(merge_cited & batch_cited),
        'tokens_dropped_at_merge': len(batch_cited - merge_cited),
        'peak_ram_mb': peak,
        'fabrication_events': sum(
            1 for b in result['batches'] if b['verdict'] != 'ok')
            + (1 if mverdict != 'ok' else 0),
    })

    out_dir = os.path.join(RUNS, args.tag)
    os.makedirs(out_dir, exist_ok=True)
    with open(os.path.join(out_dir, 'summarize_all.json'), 'w',
              encoding='utf-8', newline='\n') as fh:
        json.dump(result, fh, indent=1, ensure_ascii=False)
    with open(os.path.join(out_dir, 'summarize_all_output.md'), 'w',
              encoding='utf-8', newline='\n') as fh:
        fh.write(merge_text)
    print(json.dumps({k: v for k, v in result.items() if k != 'batches'},
                     indent=1))
    return 0


if __name__ == '__main__':
    sys.exit(main())
