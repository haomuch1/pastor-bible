"""Strip an index.db down to the one embedding model the app actually uses.

P2 built vectors for three candidate models so that P2 could choose between
them. The choice was made — nomic-embed-text-v1.5 — and the other two models'
vectors have been dead weight in the file ever since. They are two thirds of it.

This runs at build time, between the pipeline and the installer. It never
touches the index the pipeline writes: it copies, deletes, vacuums, and reports
both sizes, so the full index stays available for a later re-evaluation.

    python pipeline/strip_index.py                     # in place, to resources/
    python pipeline/strip_index.py --in X --out Y
    python pipeline/strip_index.py --check             # report, change nothing

The model kept is the one src-tauri/core/src/pipeline.rs names as
EMBED_MODEL_ID, read from that file rather than repeated here, so the two
cannot drift.
"""

import argparse
import os
import re
import shutil
import sqlite3
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FULL = os.path.join(ROOT, 'data', 'index-full.db')
BUNDLED = os.path.join(ROOT, 'src-tauri', 'resources', 'index.db')

TABLES = ('verse_embeddings', 'pericope_embeddings', 'topic_embeddings')


def kept_model():
    """The model id the Rust pipeline searches with."""
    src = os.path.join(ROOT, 'src-tauri', 'core', 'src', 'pipeline.rs')
    with open(src, encoding='utf-8') as fh:
        m = re.search(r'EMBED_MODEL_ID:\s*&str\s*=\s*"([^"]+)"', fh.read())
    if not m:
        raise SystemExit('cannot find EMBED_MODEL_ID in %s' % src)
    return m.group(1)


def mb(n):
    return '%.1f MB' % (n / float(1 << 20))


def report(path, keep):
    con = sqlite3.connect('file:%s?mode=ro' % path.replace(os.sep, '/'), uri=True)
    print('%s  %s' % (path, mb(os.path.getsize(path))))
    total_other = 0
    for t in TABLES:
        for model, n, b in con.execute(
            'SELECT model_id, COUNT(*), SUM(LENGTH(vec)) FROM %s GROUP BY model_id '
            'ORDER BY model_id' % t
        ):
            mark = 'keep' if model == keep else 'drop'
            print('  %-4s %-20s %-24s %7d rows  %s' % (mark, t, model, n, mb(b)))
            if model != keep:
                total_other += b
    print('  vectors belonging to models this app does not use: %s' % mb(total_other))
    con.close()


def strip(src, dst, keep):
    if os.path.abspath(src) != os.path.abspath(dst):
        shutil.copyfile(src, dst)
    con = sqlite3.connect(dst)
    con.execute('PRAGMA journal_mode = DELETE')
    for t in TABLES:
        con.execute('DELETE FROM %s WHERE model_id <> ?' % t, (keep,))
    con.execute('DELETE FROM embedding_models WHERE model_id <> ?', (keep,))
    # The meta rows that list the models must say what the file now holds.
    dim = con.execute('SELECT dim FROM embedding_models WHERE model_id = ?', (keep,)).fetchone()
    if not dim:
        raise SystemExit('%s has no vectors for %s' % (src, keep))
    con.execute("UPDATE meta SET value = ? WHERE key = 'embedding_models'", (keep,))
    con.execute(
        "UPDATE meta SET value = ? WHERE key = 'embedding_dims'", ('%s=%d' % (keep, dim[0]),)
    )
    con.commit()
    con.execute('VACUUM')
    con.close()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--in', dest='src', default=None)
    ap.add_argument('--out', dest='dst', default=BUNDLED)
    ap.add_argument('--check', action='store_true', help='report only')
    args = ap.parse_args()

    keep = kept_model()
    src = args.src or (FULL if os.path.exists(FULL) else BUNDLED)
    if not os.path.exists(src):
        raise SystemExit('no index at %s' % src)

    print('keeping %s' % keep)
    report(src, keep)
    if args.check:
        return 0

    # The full index is the one the pipeline wrote and the one a later
    # re-evaluation would need. Keep a copy of it before the first strip.
    if not os.path.exists(FULL) and os.path.abspath(src) == os.path.abspath(BUNDLED):
        print('\nkeeping the full index at %s' % FULL)
        os.makedirs(os.path.dirname(FULL), exist_ok=True)
        shutil.copyfile(src, FULL)
        src = FULL

    print('\nstripping -> %s' % args.dst)
    strip(src, args.dst, keep)
    print()
    report(args.dst, keep)
    return 0


if __name__ == '__main__':
    sys.exit(main())
