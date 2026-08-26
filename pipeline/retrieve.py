"""Retrieval harness. Implements PLAN 5.1 to 5.5 in Python against index.db.

This is the measurement rig for P2, not the shipped retriever; the Rust backend
in P4 reimplements the same pipeline. Every stage is a flag so that
configurations can be ablated against each other, which is the only way to see
what the embeddings actually add.

Nothing here generates text. No chat model runs in P2; query rewriting (PLAN
5.2) belongs to P3, and until then the keyword lists stored in questions.json
stand in for it.
"""

import os
import sqlite3
import struct
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)

DB = os.environ.get('TPB_INDEX_DB') or os.path.join(
    ROOT, 'src-tauri', 'resources', 'index.db')

RRF_K = 60          # reciprocal rank fusion constant, the usual 60
TOPIC_CAP = 60      # verses one matched topic may contribute
TSK_CAP = 200       # verses one TSK expansion may contribute
TOPIC_HITS = 5      # how many topic matches to expand
TSK_ANCHORS = 25    # candidates whose cross-references are followed


def unpack(blob):
    return struct.unpack('<%df' % (len(blob) // 4), blob)


class Retriever(object):
    def __init__(self, db_path=DB, model_id='bge-small-en-v1.5'):
        self.con = sqlite3.connect('file:%s?mode=ro' % db_path.replace('\\', '/'),
                                   uri=True)
        self.model_id = model_id
        row = self.con.execute(
            'SELECT dim, doc_prefix, query_prefix, n_ctx, gguf_file FROM'
            ' embedding_models WHERE model_id = ?', (model_id,)).fetchone()
        if not row:
            raise ValueError('no embeddings for model %r' % model_id)
        self.dim, self.doc_prefix, self.query_prefix, self.n_ctx, self.gguf = row
        self.canon_of = dict(self.con.execute(
            'SELECT book_id, canon FROM books'))
        self.abbrev_of = dict(self.con.execute(
            'SELECT book_id, abbrev FROM books'))
        self._verse_vecs = None
        self._peri_vecs = None
        self._topic_vecs = None

    # -- vector stores, loaded once and scanned brute force -----------------

    def _load(self, table, key):
        """Keys as a list, vectors as one contiguous (n, dim) float32 matrix.

        The whole point of storing plain BLOBs is that this is all it takes:
        one buffer, one matrix-vector product per query. The Rust backend in P4
        does the same thing without numpy.
        """
        rows = self.con.execute(
            'SELECT %s, vec FROM %s WHERE model_id = ? ORDER BY %s'
            % (key, table, key), (self.model_id,)).fetchall()
        keys = [k for k, _ in rows]
        mat = np.frombuffer(b''.join(v for _, v in rows),
                            dtype='<f4').reshape(len(rows), self.dim)
        return keys, mat

    def verse_vecs(self):
        if self._verse_vecs is None:
            self._verse_vecs = self._load('verse_embeddings', 'verse_id')
        return self._verse_vecs

    def peri_vecs(self):
        if self._peri_vecs is None:
            rows = self.con.execute(
                'SELECT pericope_id, part, start_verse_id, end_verse_id, vec'
                ' FROM pericope_embeddings WHERE model_id = ?'
                ' ORDER BY pericope_id, part', (self.model_id,)).fetchall()
            meta = [(r[0], r[1], r[2], r[3]) for r in rows]
            mat = np.frombuffer(b''.join(r[4] for r in rows),
                                dtype='<f4').reshape(len(rows), self.dim)
            self._peri_vecs = (meta, mat)
        return self._peri_vecs

    def topic_vecs(self):
        if self._topic_vecs is None:
            self._topic_vecs = self._load('topic_embeddings', 'topic_id')
        return self._topic_vecs

    def _canon_ok(self, verse_id, canon_mode):
        return canon_mode != '66' or self.canon_of[verse_id // 1000000] == 'protestant'

    # -- individual retrieval paths ----------------------------------------

    @staticmethod
    def _top(scores, keep):
        """Indices of the highest `keep` scores, best first."""
        n = min(keep, len(scores))
        if n <= 0:
            return np.empty(0, dtype=int)
        idx = np.argpartition(-scores, n - 1)[:n]
        return idx[np.argsort(-scores[idx])]

    def vector_verses(self, qvec, canon_mode, limit=100):
        keys, mat = self.verse_vecs()
        q = np.asarray(qvec, dtype='<f4')
        scores = mat @ q
        # Ask for extra so the canon filter cannot empty the result.
        out = []
        for i in self._top(scores, limit * 4):
            vid = keys[i]
            if not self._canon_ok(vid, canon_mode):
                continue
            out.append((float(scores[i]), vid))
            if len(out) >= limit:
                break
        return out

    def vector_pericopes(self, qvec, canon_mode, limit=100):
        """Returns (score, verse_ids) for the best pericope parts."""
        meta, mat = self.peri_vecs()
        q = np.asarray(qvec, dtype='<f4')
        scores = mat @ q
        out = []
        for i in self._top(scores, limit * 4):
            pid, part, sv, ev = meta[i]
            if not self._canon_ok(sv, canon_mode):
                continue
            vids = [r[0] for r in self.con.execute(
                'SELECT verse_id FROM verses WHERE verse_id BETWEEN ? AND ?'
                ' ORDER BY verse_id', (sv, ev))]
            out.append((float(scores[i]), vids))
            if len(out) >= limit:
                break
        return out

    def vector_topics(self, qvec, limit=TOPIC_HITS):
        keys, mat = self.topic_vecs()
        q = np.asarray(qvec, dtype='<f4')
        scores = mat @ q
        return [(float(scores[i]), keys[i]) for i in self._top(scores, limit)]

    def fts(self, terms, canon_mode, limit=100):
        """Union of per-term FTS5 results, each term ranked separately."""
        ranked = []
        for term in terms:
            t = '"%s"' % term.replace('"', '') if ' ' in term else term
            sql = ('SELECT f.rowid FROM verse_fts f JOIN verses v'
                   ' ON v.verse_id = f.rowid JOIN books b ON b.book_id = v.book_id'
                   ' WHERE verse_fts MATCH ?')
            args = [t]
            if canon_mode == '66':
                sql += " AND b.canon = 'protestant'"
            sql += ' ORDER BY bm25(verse_fts) LIMIT ?'
            args.append(limit)
            try:
                ranked.append([r[0] for r in self.con.execute(sql, args)])
            except sqlite3.OperationalError:
                ranked.append([])
        return ranked

    def topic_expand(self, topic_ids, canon_mode):
        out = {}
        for tid in topic_ids:
            vids = [r[0] for r in self.con.execute(
                'SELECT verse_id FROM nave_topic_verses WHERE topic_id = ?'
                ' LIMIT ?', (tid, TOPIC_CAP))]
            for v in vids:
                if self._canon_ok(v, canon_mode):
                    out.setdefault(v, 0)
                    out[v] += 1
        return out

    def tsk_expand(self, anchor_ids, canon_mode):
        if not anchor_ids:
            return {}
        marks = ','.join('?' * len(anchor_ids))
        out = {}
        for (tid,) in self.con.execute(
                'SELECT to_verse_id FROM tsk_refs WHERE from_verse_id IN (%s)'
                ' LIMIT ?' % marks, list(anchor_ids) + [TSK_CAP * 4]):
            if self._canon_ok(tid, canon_mode):
                out[tid] = out.get(tid, 0) + 1
        keep = sorted(out.items(), key=lambda kv: -kv[1])[:TSK_CAP]
        return dict(keep)

    # -- fusion -------------------------------------------------------------

    @staticmethod
    def rrf(ranked_lists, weights=None):
        """Reciprocal rank fusion over lists of verse ids."""
        weights = weights or [1.0] * len(ranked_lists)
        score = {}
        for lst, w in zip(ranked_lists, weights):
            for pos, vid in enumerate(lst):
                score[vid] = score.get(vid, 0.0) + w / (RRF_K + pos + 1)
        return score

    # -- the pipeline -------------------------------------------------------

    def search(self, qvec, keywords, canon_mode='66', use_vector_verses=True,
               use_vector_pericopes=True, use_fts=True, use_topics=False,
               use_tsk=False, top_n=25, pool=100):
        """Returns (full_set, top_n_cut, matched_topics).

        full_set is every candidate with its score and origin tags, before any
        cutoff. That whole set is what the passage panel and the summarize-all
        mode consume in P4 and P5, so the harness returns it rather than
        throwing it away.
        """
        lists, weights, origins = [], [], {}

        def tag(vid, name):
            origins.setdefault(vid, set()).add(name)

        if use_vector_verses:
            hits = self.vector_verses(qvec, canon_mode, pool)
            ids = [vid for _, vid in hits]
            for vid in ids:
                tag(vid, 'vector-verse')
            lists.append(ids)
            weights.append(1.0)

        if use_vector_pericopes:
            flat = []
            for _, vids in self.vector_pericopes(qvec, canon_mode, pool):
                for vid in vids:
                    if vid not in flat:
                        flat.append(vid)
                    tag(vid, 'vector-pericope')
            lists.append(flat[:pool * 4])
            weights.append(1.0)

        if use_fts:
            for ids in self.fts(keywords, canon_mode, pool):
                for vid in ids:
                    tag(vid, 'fts')
                lists.append(ids)
                weights.append(1.0)

        score = self.rrf(lists, weights)

        matched_topics = []
        if use_topics:
            top = self.vector_topics(qvec)
            tids = [tid for _, tid in top]
            for s, tid in top:
                row = self.con.execute(
                    'SELECT heading, (SELECT COUNT(*) FROM nave_topic_verses v'
                    ' WHERE v.topic_id = t.topic_id) FROM nave_topics t'
                    ' WHERE topic_id = ?', (tid,)).fetchone()
                matched_topics.append({'topic_id': tid, 'heading': row[0],
                                       'verses': row[1], 'score': round(s, 4)})
            for vid, n in self.topic_expand(tids, canon_mode).items():
                tag(vid, 'topic')
                score[vid] = score.get(vid, 0.0) + 0.5 / (RRF_K + 1) * min(n, 3)

        if use_tsk:
            anchors = [v for v, _ in sorted(score.items(), key=lambda kv: -kv[1])
                       ][:TSK_ANCHORS]
            for vid, n in self.tsk_expand(anchors, canon_mode).items():
                tag(vid, 'tsk')
                score[vid] = score.get(vid, 0.0) + 0.25 / (RRF_K + 1) * min(n, 4)

        full = sorted(score.items(), key=lambda kv: (-kv[1], kv[0]))
        full_set = [{'verse_id': vid, 'score': round(s, 6),
                     'origins': sorted(origins.get(vid, [])),
                     'canon': self.canon_of[vid // 1000000]}
                    for vid, s in full]
        return full_set, full_set[:top_n], matched_topics

    # -- presentation -------------------------------------------------------

    def as_ranges(self, items, gap=1):
        """Group a scored verse list into ranges within a chapter."""
        by_id = sorted(items, key=lambda d: d['verse_id'])
        out, cur = [], None
        for it in by_id:
            vid = it['verse_id']
            b, c, v = vid // 1000000, (vid % 1000000) // 1000, vid % 1000
            if (cur and cur['b'] == b and cur['c'] == c
                    and v - cur['last'] <= gap):
                cur['last'] = v
                cur['ids'].append(vid)
                cur['origins'] |= set(it['origins'])
                cur['score'] = max(cur['score'], it['score'])
            else:
                if cur:
                    out.append(cur)
                cur = {'b': b, 'c': c, 'first': v, 'last': v, 'ids': [vid],
                       'origins': set(it['origins']), 'score': it['score'],
                       'canon': it['canon']}
        if cur:
            out.append(cur)
        for r in out:
            a = self.abbrev_of[r['b']]
            r['ref'] = ('%s %d:%d' % (a, r['c'], r['first']) if r['first'] == r['last']
                        else '%s %d:%d-%d' % (a, r['c'], r['first'], r['last']))
            r['origins'] = sorted(r['origins'])
        out.sort(key=lambda r: -r['score'])
        return out

    def text_of(self, verse_ids):
        if not verse_ids:
            return []
        marks = ','.join('?' * len(verse_ids))
        return self.con.execute(
            'SELECT verse_id, text FROM verses WHERE verse_id IN (%s)'
            ' ORDER BY verse_id' % marks, list(verse_ids)).fetchall()


CONFIGS = {
    'A': dict(use_vector_verses=True, use_vector_pericopes=False,
              use_fts=False, use_topics=False, use_tsk=False),
    'B': dict(use_vector_verses=False, use_vector_pericopes=True,
              use_fts=False, use_topics=False, use_tsk=False),
    'C': dict(use_vector_verses=True, use_vector_pericopes=True,
              use_fts=False, use_topics=False, use_tsk=False),
    'D': dict(use_vector_verses=False, use_vector_pericopes=False,
              use_fts=True, use_topics=False, use_tsk=False),
    'E': dict(use_vector_verses=True, use_vector_pericopes=True,
              use_fts=True, use_topics=False, use_tsk=False),
    'F': dict(use_vector_verses=True, use_vector_pericopes=True,
              use_fts=True, use_topics=True, use_tsk=True),
    'G': dict(use_vector_verses=True, use_vector_pericopes=True,
              use_fts=True, use_topics=True, use_tsk=True),  # + rerank
}
