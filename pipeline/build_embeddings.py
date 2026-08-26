"""Add embeddings to index.db, taking it from 0.1.0 to 0.2.0.

Runs on our machines only. Reads the verses, pericopes and Nave's topics that
P1 built and writes one vector per row per shortlisted model, through
llama-server, so index vectors and query vectors come from identical code.

Usage:
    python pipeline/build_embeddings.py --report      # pericope sizes only
    python pipeline/build_embeddings.py [--models a,b]
"""

import argparse
import hashlib
import os
import sqlite3
import statistics
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)

from embed import Embedder, pack  # noqa: E402

DB = os.path.join(ROOT, 'src-tauri', 'resources', 'index.db')

INDEX_VERSION = '0.2.0'
SCHEMA_VERSION = '2'

# The shortlist. doc_prefix and query_prefix are the conventions each model card
# states; using the wrong one, or none, costs real recall.
MODELS = [
    {
        'model_id': 'bge-small-en-v1.5',
        'gguf': 'bge-small-en-v1.5-f16.gguf',
        'n_ctx': 512,
        'doc_prefix': '',
        'query_prefix': 'Represent this sentence for searching relevant passages: ',
    },
    {
        'model_id': 'nomic-embed-text-v1.5',
        'gguf': 'nomic-embed-text-v1.5-f16.gguf',
        'n_ctx': 2048,
        'doc_prefix': 'search_document: ',
        'query_prefix': 'search_query: ',
    },
    {
        'model_id': 'qwen3-embedding-0.6b',
        'gguf': 'Qwen3-Embedding-0.6B-Q8_0.gguf',
        'n_ctx': 2048,
        'doc_prefix': '',
        'query_prefix': ('Instruct: Given a question about the Bible, retrieve'
                         ' passages of scripture that answer it\nQuery: '),
    },
]


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, 'rb') as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b''):
            h.update(chunk)
    return h.hexdigest()


def connect():
    con = sqlite3.connect(DB)
    con.execute('PRAGMA foreign_keys = ON')
    return con


# ---------------------------------------------------------------------------
# Text templates. Recorded here and in docs/EVAL.md; changing one invalidates
# every vector in the database.
#
#   verse     "{abbrev} {chapter}:{verse}[ — {heading}]\n{text}"
#   pericope  "{abbrev} {chapter}:{first}-{last}[ — {heading}]\n{verse texts}"
#   topic     "{heading}"  or  "{parent heading} — {heading}" for a subtopic
# ---------------------------------------------------------------------------

def verse_rows(con):
    q = ('SELECT v.verse_id, b.abbrev, v.chapter, v.verse, v.text, p.heading'
         ' FROM verses v JOIN books b USING (book_id)'
         ' LEFT JOIN pericopes p ON p.pericope_id = v.pericope_id'
         ' ORDER BY v.verse_id')
    for vid, abbrev, ch, vs, text, heading in con.execute(q):
        label = '%s %d:%d' % (abbrev, ch, vs)
        if heading:
            label += ' — %s' % heading
        yield vid, '%s\n%s' % (label, text)


def pericope_rows(con):
    # Two cursors, deliberately. The inner lookup must not run on the cursor
    # the outer loop is iterating, or the outer result set is discarded after
    # the first row and the build silently embeds one pericope out of 10,052.
    outer = con.cursor()
    inner = con.cursor()
    q = ('SELECT p.pericope_id, b.abbrev, p.heading, p.start_verse_id,'
         ' p.end_verse_id FROM pericopes p JOIN books b USING (book_id)'
         ' ORDER BY p.pericope_id')
    for pid, abbrev, heading, sv, ev in outer.execute(q).fetchall():
        verses = inner.execute(
            'SELECT verse_id, chapter, verse, text FROM verses'
            ' WHERE verse_id BETWEEN ? AND ? ORDER BY verse_id',
            (sv, ev)).fetchall()
        if not verses:
            continue
        yield pid, abbrev, heading, verses


def topic_rows(con):
    q = ('SELECT t.topic_id, t.heading, p.heading FROM nave_topics t'
         ' LEFT JOIN nave_topics p ON p.topic_id = t.parent_topic_id'
         ' ORDER BY t.topic_id')
    for tid, heading, parent in con.execute(q):
        text = '%s — %s' % (parent, heading) if parent else heading
        yield tid, text


# ---------------------------------------------------------------------------

def report_pericopes(con, embedder=None):
    """Verses and tokens per pericope, per testament and for the deuterocanon."""
    groups = {'OT protestant': [], 'NT protestant': [], 'Deuterocanon': []}
    texts = {'OT protestant': [], 'NT protestant': [], 'Deuterocanon': []}
    q = ('SELECT p.pericope_id, b.testament, b.canon, b.abbrev, p.heading,'
         ' p.start_verse_id, p.end_verse_id FROM pericopes p'
         ' JOIN books b USING (book_id) ORDER BY p.pericope_id')
    for pid, testament, canon, abbrev, heading, sv, ev in con.execute(q):
        verses = con.execute(
            'SELECT chapter, verse, text FROM verses WHERE verse_id'
            ' BETWEEN ? AND ? ORDER BY verse_id', (sv, ev)).fetchall()
        if not verses:
            continue
        key = ('Deuterocanon' if canon == 'deutero'
               else '%s protestant' % testament)
        groups[key].append(len(verses))
        label = '%s %d:%d-%d' % (abbrev, verses[0][0], verses[0][1], verses[-1][1])
        if heading:
            label += ' — %s' % heading
        texts[key].append('%s\n%s' % (label, ' '.join(v[2] for v in verses)))

    def stats(xs):
        if not xs:
            return '-'
        xs = sorted(xs)
        p90 = xs[min(len(xs) - 1, int(0.9 * len(xs)))]
        return '%d  %d  %d  %d' % (xs[0], int(statistics.median(xs)), p90, xs[-1])

    print('pericope size, verses per pericope   (min  median  p90  max)')
    for k in ('OT protestant', 'NT protestant', 'Deuterocanon'):
        print('  %-16s n=%-6d %s' % (k, len(groups[k]), stats(groups[k])))
    print('  %-16s n=%-6d %s'
          % ('ALL', sum(len(v) for v in groups.values()),
             stats([x for v in groups.values() for x in v])))

    if embedder is None:
        return
    print()
    print('pericope size, tokens per pericope, tokenizer of %s'
          % embedder.model_file)
    allt = []
    for k in ('OT protestant', 'NT protestant', 'Deuterocanon'):
        counts = []
        batch = texts[k]
        for i in range(0, len(batch), 200):
            counts.extend(embedder.token_counts(batch[i:i + 200]))
        allt.extend(counts)
        over = sum(1 for c in counts if c > embedder.n_ctx)
        print('  %-16s n=%-6d %s   over %d ctx: %d'
              % (k, len(counts), stats(counts), embedder.n_ctx, over))
    print('  %-16s n=%-6d %s' % ('ALL', len(allt), stats(allt)))


def split_pericope(verses, embedder, max_tokens, abbrev, heading):
    """Split a pericope at verse boundaries until each part fits the context."""
    parts, cur = [], []
    for v in verses:
        trial = cur + [v]
        label = '%s %d:%d-%d' % (abbrev, trial[0][1], trial[0][2], trial[-1][2])
        if heading:
            label += ' — %s' % heading
        text = '%s\n%s' % (label, ' '.join(x[3] for x in trial))
        if cur and embedder.token_counts([text])[0] > max_tokens:
            parts.append(cur)
            cur = [v]
        else:
            cur = trial
    if cur:
        parts.append(cur)
    out = []
    for part in parts:
        label = '%s %d:%d-%d' % (abbrev, part[0][1], part[0][2], part[-1][2])
        if heading:
            label += ' — %s' % heading
        out.append((part[0][0], part[-1][0],
                    '%s\n%s' % (label, ' '.join(x[3] for x in part))))
    return out


def embed_all(con, spec, batch_size=64):
    model_id = spec['model_id']
    path = os.path.join(ROOT, 'models', spec['gguf'])
    t0 = time.time()

    with Embedder(spec['gguf'], n_ctx=spec['n_ctx']) as e:
        dim = e.dim
        con.execute('DELETE FROM embedding_models WHERE model_id=?', (model_id,))
        con.execute(
            'INSERT INTO embedding_models (model_id, gguf_file, sha256, dim,'
            ' n_ctx, doc_prefix, query_prefix, normalized)'
            ' VALUES (?,?,?,?,?,?,?,1)',
            (model_id, spec['gguf'], sha256_file(path), dim, spec['n_ctx'],
             spec['doc_prefix'], spec['query_prefix']))

        def run(rows, insert_sql, make_params):
            buf = []
            n = 0
            for item in rows:
                buf.append(item)
                if len(buf) >= batch_size:
                    n += flush(buf, insert_sql, make_params)
                    buf = []
            if buf:
                n += flush(buf, insert_sql, make_params)
            return n

        def flush(buf, insert_sql, make_params):
            vecs = e.embed([e.fit(spec['doc_prefix'] + t) for _, t in buf])
            con.executemany(insert_sql,
                            [make_params(key, dim, pack(v))
                             for (key, _), v in zip(buf, vecs)])
            return len(buf)

        con.execute('DELETE FROM verse_embeddings WHERE model_id=?', (model_id,))
        nv = run(verse_rows(con.cursor()),
                 'INSERT INTO verse_embeddings (model_id, verse_id, dim, vec)'
                 ' VALUES (?,?,?,?)',
                 lambda k, d, b: (model_id, k, d, b))
        con.commit()

        con.execute('DELETE FROM topic_embeddings WHERE model_id=?', (model_id,))
        nt = run(topic_rows(con.cursor()),
                 'INSERT INTO topic_embeddings (model_id, topic_id, dim, vec)'
                 ' VALUES (?,?,?,?)',
                 lambda k, d, b: (model_id, k, d, b))
        con.commit()

        # Pericopes, splitting any that do not fit the model's context.
        con.execute('DELETE FROM pericope_embeddings WHERE model_id=?',
                    (model_id,))
        limit = spec['n_ctx'] - 8
        pending, np_, splits = [], 0, 0
        for pid, abbrev, heading, verses in pericope_rows(con):
            label = '%s %d:%d-%d' % (abbrev, verses[0][1], verses[0][2],
                                     verses[-1][2])
            if heading:
                label += ' — %s' % heading
            text = '%s\n%s' % (label, ' '.join(v[3] for v in verses))
            if e.token_counts([text])[0] <= limit:
                pending.append((pid, 0, verses[0][0], verses[-1][0], text))
            else:
                splits += 1
                for i, (sv, ev, ptext) in enumerate(
                        split_pericope(verses, e, limit, abbrev, heading)):
                    pending.append((pid, i, sv, ev, ptext))
            if len(pending) >= batch_size:
                np_ += flush_pericopes(con, e, spec, model_id, dim, pending)
                pending = []
        if pending:
            np_ += flush_pericopes(con, e, spec, model_id, dim, pending)
        con.commit()

    wall = time.time() - t0
    return {'model_id': model_id, 'dim': dim, 'verses': nv, 'topics': nt,
            'pericopes': np_, 'splits': splits, 'wall_s': round(wall, 1)}


def flush_pericopes(con, e, spec, model_id, dim, pending):
    vecs = e.embed([e.fit(spec['doc_prefix'] + t) for _, _, _, _, t in pending])
    con.executemany(
        'INSERT INTO pericope_embeddings (model_id, pericope_id, part, dim,'
        ' vec, start_verse_id, end_verse_id) VALUES (?,?,?,?,?,?,?)',
        [(model_id, pid, part, dim, pack(v), sv, ev)
         for (pid, part, sv, ev, _), v in zip(pending, vecs)])
    return len(pending)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--report', action='store_true')
    ap.add_argument('--models', default='')
    args = ap.parse_args()

    con = connect()

    if args.report:
        with Embedder(MODELS[0]['gguf'], n_ctx=MODELS[0]['n_ctx']) as e:
            report_pericopes(con, e)
        return

    wanted = ([m for m in MODELS if m['model_id'] in args.models.split(',')]
              if args.models else MODELS)
    results = []
    for spec in wanted:
        print('embedding with %s ...' % spec['model_id'], flush=True)
        r = embed_all(con, spec)
        results.append(r)
        print('  %(verses)d verses, %(pericopes)d pericope parts (%(splits)d '
              'split), %(topics)d topics, dim %(dim)d, %(wall_s)ss' % r,
              flush=True)

    # Provenance for the gold lists. They were drawn from the 0.1.0 index,
    # whose checksum no longer matches this file once embeddings are added, so
    # the link is recorded explicitly rather than inferred from a checksum.
    gold_index = ''
    qpath = os.path.join(ROOT, 'data', 'eval', 'questions.json')
    if os.path.exists(qpath):
        import json
        with open(qpath, encoding='utf-8') as fh:
            gold_index = json.load(fh).get('generated_from_index', '')

    con.executemany('INSERT OR REPLACE INTO meta (key, value) VALUES (?,?)', [
        ('schema_version', SCHEMA_VERSION),
        ('index_version', INDEX_VERSION),
        ('gold_lists_index', gold_index),
        ('embedding_normalized', '1'),
        ('embedding_models', ','.join(r['model_id'] for r in results)),
        ('embedding_dims', ','.join('%s=%d' % (r['model_id'], r['dim'])
                                    for r in results)),
    ])
    con.commit()
    con.execute('VACUUM')
    con.commit()
    con.close()
    print('done')


if __name__ == '__main__':
    main()
