"""Render a smoke run into docs/SMOKE.md for Jared to read at will.

Each answer is exactly what the verifier passed, with the passages it cited
listed underneath as references plus their opening words, so the answer can be
checked against the text without opening a Bible. No rating is requested and
none is recorded: PLAN 6.3's measured gates are fabrication count and citation
precision, and this file is for reading.

Usage:  python pipeline/write_smoke.py qwen3-8b-smoke
"""

import json
import os
import re
import sys

sys.stdout.reconfigure(encoding='utf-8')

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RUNS = os.path.join(ROOT, 'data', 'eval', 'runs')
OUT = os.path.join(ROOT, 'docs', 'SMOKE.md')


def first_words(text, n=10):
    w = re.sub(r'\s+', ' ', text).split(' ')
    return ' '.join(w[:n]) + (' ...' if len(w) > n else '')


def main():
    tag = sys.argv[1] if len(sys.argv) > 1 else 'qwen3-8b-smoke'
    run_dir = os.path.join(RUNS, tag)
    rows = [json.loads(l) for l in
            open(os.path.join(run_dir, 'metrics.jsonl'), encoding='utf-8')
            if l.strip()]
    seen = {}
    for r in rows:
        seen[r['question_id']] = r
    rows = [seen[k] for k in sorted(seen)]

    summ_path = os.path.join(run_dir, 'summary.json')
    summ = json.load(open(summ_path, encoding='utf-8')) if os.path.exists(summ_path) else {}

    import sqlite3
    con = sqlite3.connect(
        'file:%s?mode=ro' % os.path.join(
            ROOT, 'src-tauri', 'resources', 'index.db').replace('\\', '/'),
        uri=True)

    out = []
    out.append('# SMOKE')
    out.append('')
    out.append('Ten questions run end to end through the pipeline that P3')
    out.append('selected: %s, canon 66, retrieval configuration F.' % summ.get('gguf', tag))
    out.append('')
    out.append('Each answer below is exactly what the citation verifier passed.')
    out.append('Nothing has been edited. Under each answer are the passages it')
    out.append('cited, with the first ten words of each, so you can see what the')
    out.append('answer was built from without looking anything up.')
    out.append('')
    out.append('No rating is asked for. This file exists to be read.')
    out.append('')
    out.append('Generated 2026-08-26.')
    out.append('')

    for r in rows:
        qid = r['question_id']
        raw_path = os.path.join(run_dir, 'raw', '%s.json' % qid)
        raw = json.load(open(raw_path, encoding='utf-8'))
        out.append('')
        out.append('=' * 72)
        out.append('%s  %s' % (qid, raw['question']))
        out.append('=' * 72)
        out.append('')
        if r['fallback_used']:
            out.append('The verifier rejected two attempts, so the app would show')
            out.append('the passage list instead of a synopsis. That list follows.')
            out.append('')
        out.append(raw['final'].strip())
        out.append('')
        out.append('-- passages cited --')
        by_token = {s['token']: s for s in raw['sent']}
        cited = r['cited_tokens'] or sorted(by_token)
        for t in sorted(cited, key=lambda x: int(re.sub(r'\D', '', x))):
            s = by_token.get(t)
            if not s:
                continue
            txt = con.execute('SELECT text FROM verses WHERE verse_id=?',
                              (s['verse_ids'][0],)).fetchone()
            mark = ' [Deuterocanon]' if s.get('canon') == 'deutero' else ''
            out.append('  %-5s %-16s%s  %s'
                       % (t, s['ref'], mark, first_words(txt[0] if txt else '')))
        out.append('')
        out.append('-- how it went --')
        out.append('  first pass: %s%s' % (
            r['first_pass_verdict'],
            ', retry: ' + r['retry']['verdict'] if r.get('retry') else ''))
        out.append('  themes: %d   passages cited: %d of %d sent'
                   % (r['themes'], len(r['cited_tokens'] or []),
                      r['sent_passages']))
        out.append('  time: %.0f seconds' % r['end_to_end_seconds'])

    with open(OUT, 'w', encoding='utf-8', newline='\n') as fh:
        fh.write('\n'.join(out) + '\n')
    print('wrote %s: %d questions' % (OUT, len(rows)))


if __name__ == '__main__':
    main()
