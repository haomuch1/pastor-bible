"""STEP 5: the graded questions through the Rust CLI, one at a time.

This driver does no retrieval and no generation of its own. It invokes
pastor-bible-cli once per question, exactly as a user would, and scores the
answer structure the CLI returns. Nothing here can make the Rust pipeline look
better than it is, because nothing here is part of it.

One process at a time, one question at a time. Each row is written before the
next question starts, so an interrupted run still leaves usable evidence.

Usage:
  python pipeline/run_p4.py --tag p4-rust-8b
  python pipeline/run_p4.py --tag p4-rust-1.7b --model fallback --ids g01,g13
  python pipeline/run_p4.py --tag p4-rust-8b-deutero --ids g19 --canon both
"""

import argparse
import io
import json
import os
import re
import statistics
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)

QUESTIONS = os.path.join(ROOT, 'data', 'eval', 'questions.json')
RUNS = os.path.join(ROOT, 'data', 'eval', 'runs')
CLI = os.path.join(ROOT, 'src-tauri', 'target', 'release', 'pastor-bible-cli.exe')
if not os.path.exists(CLI):
    CLI = os.path.join(ROOT, 'src-tauri', 'target', 'release', 'pastor-bible-cli')

REF_RE = re.compile(
    r'\b(?:Gen|Exo|Lev|Num|Deu|Jos|Jdg|Rut|1Sa|2Sa|1Ki|2Ki|1Ch|2Ch|Ezr|Neh|Est'
    r'|Job|Psa|Pro|Ecc|Sng|Isa|Jer|Lam|Eze|Dan|Hos|Jol|Amo|Oba|Jon|Mic|Nah|Hab'
    r'|Zep|Hag|Zec|Mal|Mat|Mrk|Luk|Jhn|Act|Rom|1Co|2Co|Gal|Eph|Php|Col|1Th|2Th'
    r'|1Ti|2Ti|Tit|Phm|Heb|Jas|1Pe|2Pe|1Jn|2Jn|3Jn|Jud|Rev)\s+\d+[:.]\d+')


def free_ram_gb():
    out = subprocess.run(
        ['powershell', '-NoProfile', '-Command',
         "(Get-Counter '\\Memory\\Available MBytes').CounterSamples[0].CookedValue"],
        capture_output=True, text=True, timeout=60)
    return round(float(out.stdout.strip()) / 1024, 2)


def score(answer, q):
    """Metrics for one answer, computed from the structure the CLI returned."""
    must = q.get('must') or []
    should = q.get('should') or []
    must_sets = [set(p['verse_ids']) for p in must]
    should_ids = set()
    for p in should:
        should_ids.update(p['verse_ids'])
    gold_ids = set().union(*must_sets) if must_sets else set()

    sent = [p for p in answer['passages'] if p['sent']]
    sent_ids = {i for p in sent for i in p['verse_ids']}
    by_token = {p['token']: p for p in sent}

    cited = answer['cited_tokens']
    cited_ids = set()
    for t in cited:
        if t in by_token:
            cited_ids.update(by_token[t]['verse_ids'])

    precision = None
    if cited:
        good = sum(1 for t in cited
                   if t in by_token
                   and set(by_token[t]['verse_ids']) & (gold_ids | should_ids))
        precision = good / len(cited)

    coverage = None
    if must_sets:
        present = [m for m in must_sets if m & sent_ids]
        if present:
            coverage = sum(1 for m in present if m & cited_ids) / len(present)

    recall = None
    if must_sets:
        recall = sum(1 for m in must_sets if m & sent_ids) / len(must_sets)

    shown = answer.get('synopsis_markdown') or answer.get('fallback_markdown') or ''
    headings = [l for l in shown.splitlines() if l.strip().startswith('##')]
    structure_ok = (2 <= len(headings) <= 6) and not answer['fallback_used']
    if structure_ok:
        blocks, cur = [], []
        for line in shown.splitlines():
            if line.strip().startswith('##'):
                if cur:
                    blocks.append('\n'.join(cur))
                cur = [line]
            elif cur:
                cur.append(line)
        if cur:
            blocks.append('\n'.join(cur))
        structure_ok = all(re.findall(r'\[P\d+\]', b) for b in blocks)

    # The hard gate, checked against the text a reader would actually see and
    # not against any intermediate: no reference of any kind may appear in it,
    # and every token in it must be one that was sent.
    unsent = [t for t in re.findall(r'\[P\d+\]', shown) if t not in by_token]
    written_refs = REF_RE.findall(shown)

    return {
        'citation_precision': precision,
        'citation_coverage': coverage,
        'recall25_sent': recall,
        'themes': len(headings),
        'structure_ok': structure_ok,
        'unsent_tokens_in_output': unsent,
        'written_references_in_output': written_refs,
        'fabrications_reaching_output': len(unsent) + len(written_refs),
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--tag', required=True)
    ap.add_argument('--model', default='default')
    ap.add_argument('--canon', default='66')
    ap.add_argument('--query', default='raw')
    ap.add_argument('--ctx', default='8192')
    ap.add_argument('--ids', default='')
    args = ap.parse_args()

    sys.stdout.reconfigure(encoding='utf-8')
    if not os.path.exists(CLI):
        print('build the CLI first: cargo build --release -p pastor-bible-core')
        return 2

    data = json.load(io.open(QUESTIONS, encoding='utf-8'))
    by_id = {g['id']: g for g in data['graded']}
    by_id.update({s['id']: s for s in data['smoke']})
    ids = args.ids.split(',') if args.ids else data['p3_graded']

    out_dir = os.path.join(RUNS, args.tag)
    raw_dir = os.path.join(out_dir, 'raw')
    os.makedirs(raw_dir, exist_ok=True)
    metrics_path = os.path.join(out_dir, 'metrics.jsonl')

    print('free RAM before any model load: %.2f GB' % free_ram_gb())
    rows = []
    for qid in ids:
        q = by_id[qid]
        json_path = os.path.join(raw_dir, '%s.json' % qid)
        cmd = [CLI, 'ask', q['question'], '--canon', args.canon, '--model',
               args.model, '--query', args.query, '--ctx', args.ctx,
               '--json', json_path, '--quiet']
        t0 = time.time()
        proc = subprocess.run(cmd, capture_output=True, text=True,
                              encoding='utf-8', errors='replace')
        wall = time.time() - t0
        if proc.returncode != 0:
            print('  %s FAILED rc=%d\n%s' % (qid, proc.returncode, proc.stderr[-2000:]))
            return 1
        answer = json.load(io.open(json_path, encoding='utf-8'))

        row = {'model': args.tag, 'question_id': qid, 'canon': args.canon,
               'query_mode': answer['query_mode'],
               'model_id': answer['model_id'],
               'verdict': answer['verdict'],
               'first_pass_verdict': answer['attempts'][0]['verdict'],
               'retry_verdict': (answer['attempts'][1]['verdict']
                                 if len(answer['attempts']) > 1 else None),
               'fallback_used': answer['fallback_used'],
               'first_pass_violations': answer['attempts'][0]['violations'],
               'sent_passages': answer['sent_count'],
               'full_set_passages': len(answer['passages']),
               'cited_tokens': answer['cited_tokens'],
               'deuterocanon_cited': answer['deuterocanon_cited'],
               'crisis': answer['crisis'],
               'prompt_tokens': answer['attempts'][0]['prompt_tokens'],
               'completion_tokens': answer['attempts'][0]['completion_tokens'],
               'peak_ram_mb': answer['peak_ram_mb'],
               'topics': [t['heading_display'] for t in answer['topics']],
               'wall_seconds': round(wall, 2)}
        row.update(score(answer, q))
        row.update({'timings': answer['timings']})
        rows.append(row)
        with io.open(metrics_path, 'a', encoding='utf-8', newline='\n') as fh:
            fh.write(json.dumps(row, ensure_ascii=False) + '\n')
        print('  %-4s %-9s%s  fab %d  cited %2d  prec %s  %.1fs  peak %s MB'
              % (qid, row['first_pass_verdict'],
                 ' -> ' + row['retry_verdict'] if row['retry_verdict'] else '',
                 row['fabrications_reaching_output'], len(row['cited_tokens']),
                 '%.2f' % row['citation_precision'] if row['citation_precision'] is not None else '  - ',
                 wall, row['peak_ram_mb']))

    def mean(key):
        vals = [r[key] for r in rows if r[key] is not None]
        return sum(vals) / len(vals) if vals else None

    summary = {
        'tag': args.tag, 'model': args.model, 'canon': args.canon,
        'query_mode': rows[0]['query_mode'], 'questions': len(rows),
        'fabrications_reaching_output': sum(r['fabrications_reaching_output'] for r in rows),
        'first_pass_violation_rate': sum(1 for r in rows if r['first_pass_verdict'] != 'ok') / len(rows),
        'retry_rate': sum(1 for r in rows if r['retry_verdict']) / len(rows),
        'fallback_rate': sum(1 for r in rows if r['fallback_used']) / len(rows),
        'structure_ok_rate': sum(1 for r in rows if r['structure_ok']) / len(rows),
        'citation_precision': mean('citation_precision'),
        'citation_coverage': mean('citation_coverage'),
        'recall25_sent': mean('recall25_sent'),
        'median_wall_s': statistics.median(r['wall_seconds'] for r in rows),
        'max_wall_s': max(r['wall_seconds'] for r in rows),
        'peak_ram_mb': max(r['peak_ram_mb'] or 0 for r in rows),
        'themes': [r['themes'] for r in rows],
    }
    with io.open(os.path.join(out_dir, 'summary.json'), 'w', encoding='utf-8',
                 newline='\n') as fh:
        json.dump(summary, fh, indent=1)
    print()
    print(json.dumps(summary, indent=1))
    return 0


if __name__ == '__main__':
    sys.exit(main())
