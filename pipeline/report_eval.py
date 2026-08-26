"""Turn evaluate.py's JSON into the plain-text tables that go in docs/EVAL.md.

Every number printed here is read from the evaluation output, which was itself
produced by querying the built index.db. Nothing is retyped.

Usage:  python pipeline/report_eval.py results.json
"""

import json
import os
import statistics
import sys

sys.stdout.reconfigure(encoding='utf-8')

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

CONFIG_LABEL = {
    'A': 'vector only, verses',
    'B': 'vector only, pericopes',
    'C': 'vector only, verses + pericopes',
    'D': 'FTS only',
    'E': 'C + D fused (hybrid, no expansion)',
    'F': 'E + topic expansion + TSK expansion',
    'G': 'F + reranker',
}

EVIDENCE = set('ABC')


def mean(xs):
    return statistics.mean(xs) if xs else 0.0


def main():
    path = sys.argv[1] if len(sys.argv) > 1 else 'eval_results.json'
    with open(path, encoding='utf-8') as fh:
        d = json.load(fh)
    res = d['results']
    models = sorted({k.split('|')[0] for k in res})
    configs = sorted({k.split('|')[1] for k in res})
    qids = sorted(next(iter(res.values())).keys())

    print('### Recall@25 against MUST, by configuration and embedding model')
    print()
    print('    %-6s %-38s %s' % ('cfg', 'what it is',
                                 '  '.join('%-22s' % m for m in models)))
    for c in configs:
        row = []
        for m in models:
            key = '%s|%s' % (m, c)
            row.append('%-22s' % ('%.3f' % mean([e['r25'] for e in res[key].values()])
                                  if key in res else '-'))
        star = ' ' if c in EVIDENCE else '~'
        print('  %s %-6s %-38s %s' % (star, c, CONFIG_LABEL[c], '  '.join(row)))
    print()
    print('  Rows marked ~ use the topic, cross-reference or keyword paths that')
    print('  the gold lists were themselves drawn from. They are reported for')
    print('  completeness and are near-circular; A, B and C are the evidence.')
    print()

    print('### Where the curve bends: recall@10, @25, @50')
    print()
    print('    %-22s %-4s %8s %8s %8s' % ('model', 'cfg', '@10', '@25', '@50'))
    for m in models:
        for c in ('C', 'G'):
            key = '%s|%s' % (m, c)
            if key not in res:
                continue
            v = res[key].values()
            print('    %-22s %-4s %8.3f %8.3f %8.3f'
                  % (m, c, mean([e['r10'] for e in v]),
                     mean([e['r25'] for e in v]), mean([e['r50'] for e in v])))
    print()

    print('### Per-question recall@25, configuration C and G')
    print()
    best = models[0]
    hdr = '    %-5s' % 'q'
    for m in models:
        hdr += ' %-13s' % m.split('-')[0][:12]
    print(hdr + '   G')
    for qid in qids:
        line = '    %-5s' % qid
        for m in models:
            key = '%s|C' % m
            line += ' %-13.3f' % (res[key][qid]['r25'] if key in res else 0)
        gkey = '%s|G' % best
        line += '   %.3f' % (res[gkey][qid]['r25'] if gkey in res else 0)
        print(line)
    print()

    fs = d.get('fullsets') or {}
    if fs:
        print('### Full retrieved set under configuration F')
        print()
        pas = [v['passages'] for v in fs.values()]
        ver = [v['verses'] for v in fs.values()]
        tok = [v.get('tokens', 0) for v in fs.values()]
        print('    %-12s %8s %8s %8s' % ('', 'min', 'median', 'max'))
        print('    %-12s %8d %8d %8d' % ('passages', min(pas),
                                         int(statistics.median(pas)), max(pas)))
        print('    %-12s %8d %8d %8d' % ('verses', min(ver),
                                         int(statistics.median(ver)), max(ver)))
        if any(tok):
            print('    %-12s %8d %8d %8d' % ('tokens', min(tok),
                                             int(statistics.median(tok)), max(tok)))
        print()

    print('### Deuterocanon, g19 and g20 under both-canon mode')
    print()
    for m in models:
        for c in configs:
            key = '%s|%s' % (m, c)
            if key not in res:
                continue
            for qid in ('g19', 'g20'):
                e = res[key].get(qid) or {}
                ranks = e.get('deutero_ranks') or {}
                if not ranks:
                    continue
                found = {k: v for k, v in ranks.items() if v and v <= 25}
                print('    %-22s %s %s  in top 25: %d of %d   %s'
                      % (m, c, qid, len(found), len(ranks),
                         ', '.join('%s@%d' % (k, v) for k, v in
                                   sorted(found.items(), key=lambda kv: kv[1]))
                         or '-'))
    print()

    tm = d.get('timings') or {}
    if tm:
        print('### Latency, mean seconds per query')
        print()
        for k in sorted(tm):
            print('    %-30s %.3f' % (k, tm[k]))
        print()
    ram = d.get('peak_ram_mb') or {}
    if ram:
        print('### Peak resident memory, MB')
        print()
        for k in sorted(ram):
            print('    %-30s %s' % (k, ram[k]))
        print()


if __name__ == '__main__':
    main()
