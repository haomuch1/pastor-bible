"""Per-model P3 metrics, recomputed uniformly from the stored run artifacts.

Structure compliance is scored here rather than trusted from the run rows, so
that every model is scored by one definition even though the harness's own
check changed mid-session. The definition is the one P3 set out: theme headings
present, every theme cites at least one token, and no theme cites a token that
was not sent.

Usage:  python pipeline/report_p3.py qwen3-1.7b qwen3-4b qwen3-8b
"""

import json
import os
import re
import statistics
import sys

sys.stdout.reconfigure(encoding='utf-8')

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RUNS = os.path.join(ROOT, 'data', 'eval', 'runs')

TOKEN_RE = re.compile(r'\[P\d+\]')


def theme_blocks(text):
    blocks, cur = [], []
    for line in text.splitlines():
        if line.strip().startswith('##'):
            if cur:
                blocks.append('\n'.join(cur))
            cur = [line]
        elif cur:
            cur.append(line)
    if cur:
        blocks.append('\n'.join(cur))
    return blocks


def structure_ok(final_text, sent_tokens, fallback_used):
    """P3's definition, applied identically to every model."""
    if fallback_used:
        return False
    blocks = theme_blocks(final_text)
    if not blocks:
        return False
    for b in blocks:
        toks = set(TOKEN_RE.findall(b))
        if not toks:
            return False
        if not toks <= sent_tokens:
            return False
    return True


def load(tag):
    path = os.path.join(RUNS, tag, 'metrics.jsonl')
    if not os.path.exists(path):
        return None
    rows = [json.loads(l) for l in open(path, encoding='utf-8') if l.strip()]
    # De-duplicate by question id, keeping the last run of each.
    seen = {}
    for r in rows:
        seen[r['question_id']] = r
    rows = [seen[k] for k in sorted(seen)]
    for r in rows:
        raw_path = os.path.join(RUNS, tag, 'raw', '%s.json' % r['question_id'])
        if os.path.exists(raw_path):
            raw = json.load(open(raw_path, encoding='utf-8'))
            sent_tokens = {s['token'] for s in raw['sent']}
            r['_structure_ok'] = structure_ok(raw['final'], sent_tokens,
                                              r['fallback_used'])
            r['_final_chars'] = len(raw['final'])
        else:
            r['_structure_ok'] = r.get('structure_ok', False)
            r['_final_chars'] = None
    return rows


def m(xs):
    xs = [x for x in xs if x is not None]
    return statistics.mean(xs) if xs else float('nan')


def main():
    tags = sys.argv[1:] or ['qwen3-1.7b', 'qwen3-4b', 'qwen3-8b']
    data = {t: load(t) for t in tags}
    data = {t: r for t, r in data.items() if r}

    print('P3 graded runs, canon 66, %d questions each'
          % (len(next(iter(data.values()))) if data else 0))
    print()
    hdr = ('%-14s %6s %6s %6s %7s %7s %7s %8s %7s %7s %8s'
           % ('model', 'fab1st', 'retry', 'fallbk', 'cit-prec', 'cit-cov',
              'struct', 'rewrite', 'med s', 'max s', 'peak MB'))
    print(hdr)
    print('-' * len(hdr))
    for tag, rows in data.items():
        n = len(rows)
        summ_path = os.path.join(RUNS, tag, 'summary.json')
        summ = json.load(open(summ_path, encoding='utf-8')) if os.path.exists(summ_path) else {}
        fab = sum(1 for r in rows if r['first_pass_verdict'] != 'ok') / n
        retried = sum(1 for r in rows if r.get('retry')) / n
        fb = sum(1 for r in rows if r['fallback_used']) / n
        struct = sum(1 for r in rows if r['_structure_ok']) / n
        rewrite = m([r['recall25_model_rewrites'] for r in rows]) - \
            m([r['recall25_hand_keywords'] for r in rows])
        print('%-14s %6.2f %6.2f %6.2f %7.3f %7.3f %7.2f %+8.3f %7.1f %7.1f %8.0f'
              % (tag, fab, retried, fb,
                 m([r['citation_precision'] for r in rows]),
                 m([r['citation_coverage'] for r in rows]),
                 struct, rewrite,
                 statistics.median([r['end_to_end_seconds'] for r in rows]),
                 max(r['end_to_end_seconds'] for r in rows),
                 summ.get('peak_ram_chat_mb') or 0))
    print()
    print('  fab1st   first-pass verifier violation rate')
    print('  retry    fraction of questions that needed a second generation')
    print('  fallbk   fraction that fell back to the passage list')
    print('  cit-prec fraction of cited passages that are in MUST or SHOULD')
    print('  cit-cov  fraction of sent MUST passages the synopsis cited')
    print('  struct   headings present, every theme cites a sent token')
    print('  rewrite  recall@25 with model rewrites minus with hand keywords')
    print()

    print('Themes per answer, and generation speed')
    print()
    print('%-14s %-34s %9s %9s' % ('model', 'themes per question', 'tok/s',
                                   'out chars'))
    for tag, rows in data.items():
        print('%-14s %-34s %9.1f %9.0f'
              % (tag, str([r['themes'] for r in rows]),
                 statistics.median([r['tokens_per_second'] for r in rows
                                    if r['tokens_per_second']]),
                 m([r['_final_chars'] for r in rows])))
    print()

    print('Per-question recall@25, model rewrites vs hand keywords')
    print()
    qids = sorted({r['question_id'] for rows in data.values() for r in rows})
    print('%-6s %s' % ('q', ' '.join('%-18s' % t for t in data)))
    for qid in qids:
        line = '%-6s' % qid
        for tag, rows in data.items():
            r = next((x for x in rows if x['question_id'] == qid), None)
            line += ' %-18s' % ('%.2f / %.2f' % (r['recall25_model_rewrites'],
                                                 r['recall25_hand_keywords'])
                                if r else '-')
        print(line)


if __name__ == '__main__':
    main()
