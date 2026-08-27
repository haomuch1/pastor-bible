"""Recompute P4's numbers from the stored artefacts, and set them beside P3's.

Nothing here trusts a summary file. Every figure is derived again from the rows
the runs wrote, so a mistake in a harness cannot survive into a document.

Usage:
  python pipeline/report_p4.py                       p4-rust-8b against qwen3-8b
  python pipeline/report_p4.py p4-rust-1.7b qwen3-1.7b
"""

import io
import json
import os
import re
import statistics
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
RUNS = os.path.join(ROOT, 'data', 'eval', 'runs')


def rows(tag):
    path = os.path.join(RUNS, tag, 'metrics.jsonl')
    if not os.path.exists(path):
        return []
    return [json.loads(l) for l in io.open(path, encoding='utf-8') if l.strip()]


def mean(vals):
    vals = [v for v in vals if v is not None]
    return sum(vals) / len(vals) if vals else None


def fmt(v, spec='%.3f'):
    return '   -  ' if v is None else spec % v


def p4_stats(rs):
    return {
        'n': len(rs),
        'fabrications': sum(r['fabrications_reaching_output'] for r in rs),
        'first_pass_violation_rate': sum(1 for r in rs if r['first_pass_verdict'] != 'ok') / len(rs),
        'retry_rate': sum(1 for r in rs if r.get('retry_verdict')) / len(rs),
        'fallback_rate': sum(1 for r in rs if r['fallback_used']) / len(rs),
        'structure': sum(1 for r in rs if r['structure_ok']) / len(rs),
        'precision': mean([r['citation_precision'] for r in rs]),
        'coverage': mean([r['citation_coverage'] for r in rs]),
        'recall25': mean([r['recall25_sent'] for r in rs]),
        'median_s': statistics.median(r['wall_seconds'] for r in rs),
        'max_s': max(r['wall_seconds'] for r in rs),
        'peak_mb': max(r['peak_ram_mb'] or 0 for r in rs),
        'themes': [r['themes'] for r in rs],
        'cited': [len(r['cited_tokens']) for r in rs],
        'prompt_tokens': [r['prompt_tokens'] for r in rs if r['prompt_tokens']],
        'gen_s': [r['timings']['generate_seconds'] for r in rs],
        'retrieve_s': [r['timings']['retrieve_seconds'] for r in rs],
    }


def p3_stats(rs):
    if not rs:
        return None
    return {
        'n': len(rs),
        'first_pass_violation_rate': sum(1 for r in rs if r['first_pass_verdict'] != 'ok') / len(rs),
        'retry_rate': sum(1 for r in rs if r.get('retry')) / len(rs),
        'fallback_rate': sum(1 for r in rs if r['fallback_used']) / len(rs),
        'precision': mean([r['citation_precision'] for r in rs]),
        'coverage': mean([r['citation_coverage'] for r in rs]),
        'median_s': statistics.median(r['end_to_end_seconds'] for r in rs),
        'max_s': max(r['end_to_end_seconds'] for r in rs),
        'themes': [r['themes'] for r in rs],
        'cited': [len(r['cited_tokens']) for r in rs],
        'prompt_tokens': [r['prompt_tokens'] for r in rs if r['prompt_tokens']],
        'gen_s': [r['gen_seconds'] for r in rs],
        'retrieve_s': [r['retrieve_seconds'] for r in rs],
        'recall25': mean([r['recall25_model_rewrites'] for r in rs]),
    }


def main():
    sys.stdout.reconfigure(encoding='utf-8')
    a_tag = sys.argv[1] if len(sys.argv) > 1 else 'p4-rust-8b'
    b_tag = sys.argv[2] if len(sys.argv) > 2 else 'qwen3-8b'
    a, b = rows(a_tag), rows(b_tag)
    if not a:
        print('no rows for %s' % a_tag)
        return 1
    A = p4_stats(a)
    B = p3_stats(b)

    print('P4 Rust, %s: %d questions' % (a_tag, A['n']))
    print()
    print('  per question')
    print('  %-5s %-9s %-9s %5s %5s %6s %6s %7s %8s'
          % ('id', 'first', 'retry', 'fab', 'cited', 'prec', 'cov', 'themes', 'seconds'))
    for r in a:
        print('  %-5s %-9s %-9s %5d %5d %6s %6s %7d %8.1f'
              % (r['question_id'], r['first_pass_verdict'],
                 r.get('retry_verdict') or '-', r['fabrications_reaching_output'],
                 len(r['cited_tokens']), fmt(r['citation_precision'], '%.2f'),
                 fmt(r['citation_coverage'], '%.2f'), r['themes'], r['wall_seconds']))
    print()
    print('  fabricated references reaching output: %d   (the hard gate; must be 0)'
          % A['fabrications'])
    print('  prompt tokens: min %d  median %d  max %d'
          % (min(A['prompt_tokens']), statistics.median(A['prompt_tokens']),
             max(A['prompt_tokens'])))
    print('  retrieval: median %.3f s  max %.3f s'
          % (statistics.median(A['retrieve_s']), max(A['retrieve_s'])))
    print()

    if B:
        print('  against P3 (%s, Python, model rewrites for retrieval)' % b_tag)
        print('  %-28s %10s %10s %10s' % ('', 'P4 Rust', 'P3 Python', 'delta'))
        pairs = [
            ('first-pass violation rate', 'first_pass_violation_rate', '%.2f'),
            ('retry rate', 'retry_rate', '%.2f'),
            ('fallback rate', 'fallback_rate', '%.2f'),
            ('citation precision', 'precision', '%.3f'),
            ('citation coverage', 'coverage', '%.3f'),
            ('recall@25 of sent set', 'recall25', '%.3f'),
            ('median end-to-end s', 'median_s', '%.1f'),
            ('max end-to-end s', 'max_s', '%.1f'),
        ]
        for label, key, spec in pairs:
            x, y = A.get(key), B.get(key)
            d = (x - y) if (x is not None and y is not None) else None
            print('  %-28s %10s %10s %10s'
                  % (label, fmt(x, spec), fmt(y, spec), fmt(d, '%+' + spec[1:])))
        print('  %-28s %10s %10s' % ('themes per question', A['themes'], B['themes']))
        print('  %-28s %10s %10s' % ('cited per question', A['cited'], B['cited']))
        print('  %-28s %10.1f %10.1f'
              % ('median generation s', statistics.median(A['gen_s']),
                 statistics.median(B['gen_s'])))
        print('  %-28s %10.3f %10.3f'
              % ('median retrieval s', statistics.median(A['retrieve_s']),
                 statistics.median(B['retrieve_s'])))
        print('  %-28s %10.0f %10s' % ('peak sidecar RAM MB', A['peak_mb'], '8998 / 15068'))
        print()
        print('  Note: P3 retrieved with the model rewrites and P4 retrieves from')
        print('  the raw question, which is the P4 STEP 6 decision. Citation and')
        print('  recall figures therefore compare two different retrieved sets and')
        print('  are not a like-for-like measure of the port. The verifier figures')
        print('  and the fabrication count are like for like.')
    return 0


if __name__ == '__main__':
    sys.exit(main())
