"""Draft candidate gold passage lists for the evaluation set.

Every candidate comes out of index.db. Nothing here is written from memory:
a passage reaches the draft only because Nave's, the FTS5 keyword index, or a
Treasury of Scripture Knowledge hop put it there, and every passage is checked
back against real verse rows before it is written out.

The output is a DRAFT. Plan 6.2 is explicit that gold lists are judgment and
are Jared's to approve. This script proposes; it does not decide.

Usage:  python pipeline/draft_gold.py
"""

import json
import os
import re
import sqlite3
import sys

sys.stdout.reconfigure(encoding='utf-8')

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DB = os.path.join(ROOT, 'src-tauri', 'resources', 'index.db')
OUT_JSON = os.path.join(ROOT, 'data', 'eval', 'questions.json')
OUT_REVIEW = os.path.join(ROOT, 'docs', 'EVAL-GOLD-REVIEW.md')

MUST_MIN, MUST_MAX = 5, 8
SHOULD_MIN, SHOULD_MAX = 8, 12
MAX_RANGE_LEN = 12      # a gold passage is never a whole chapter
MERGE_GAP = 2           # verses this far apart still join one range
FTS_LIMIT = 60          # top hits per keyword

# ---------------------------------------------------------------------------
# The question set. Wording is fixed by the session brief and must not drift.
# keywords are biblical vocabulary, stored so that P2 can compare them against
# the model's own query rewrites. nave lists candidate topic headings; the
# script records which ones actually existed.
# ---------------------------------------------------------------------------

GRADED = [
    ('g01', 'What does the Bible say about anxiety and worry?', 'life', '66',
     ['anxious', 'careful', 'worry', 'troubled', 'cast thy care'],
     ['ANXIETY', 'CARE']),
    ('g02', 'How should I deal with anger?', 'life', '66',
     ['anger', 'wrath', 'angry', 'slow to anger', 'strife'],
     ['ANGER', 'HATRED']),
    ('g03', 'What does the Bible say about forgiving someone who hurt me?',
     'life', '66',
     ['forgive', 'forgiveness', 'trespasses', 'reconcile', 'mercy'],
     ['FORGIVENESS', 'RECONCILIATION']),
    ('g04', 'What does the Bible say about grief and losing someone I love?',
     'life', '66',
     ['mourn', 'comfort', 'sorrow', 'weep', 'tears'],
     ['GRIEF', 'MOURNING', 'SORROW', 'DEATH']),
    ('g05', 'What does the Bible say about money and debt?', 'life', '66',
     ['love of money', 'debt', 'usury', 'lend', 'riches', 'borrow'],
     ['MONEY', 'DEBT', 'RICHES', 'DEBTOR']),
    ('g06', 'What does the Bible say about marriage?', 'life', '66',
     ['one flesh', 'husband', 'wife', 'marriage', 'joined to his wife'],
     ['MARRIAGE', 'HUSBAND', 'WIFE']),
    ('g07', 'What does the Bible say about pride and humility?', 'life', '66',
     ['pride', 'proud', 'humble', 'humility', 'lowly'],
     ['PRIDE', 'HUMILITY']),
    ('g08', 'What does the Bible say about fear?', 'life', '66',
     ['fear not', "don't be afraid", 'fear of Yahweh', 'afraid', 'dismayed'],
     ['COURAGE', 'CONFIDENCE']),
    ('g09', 'What does the Bible say about temptation?', 'life', '66',
     ['tempt', 'temptation', 'tempted', 'lust', 'snare'],
     ['TEMPTATION']),
    ('g10', 'How should I treat my enemies?', 'life', '66',
     ['love your enemies', 'persecute', 'avenge', 'overcome evil', 'enemies'],
     ['ENEMY', 'LOVE']),
    ('g11', 'Why do bad things happen to good people?', 'life', '66',
     ['affliction', 'chasten', 'tribulation', 'wicked prosper', 'suffer'],
     ['AFFLICTIONS AND ADVERSITIES', 'SUFFERING', 'CHASTISEMENT']),
    ('g12', 'I feel burned out and hopeless. What does the Bible say?',
     'life', '66',
     ['weary', 'faint', 'hope', 'strength', 'rest'],
     ['DESPONDENCY', 'HOPE', 'REST']),
    ('g13', 'What is love, according to the Bible?', 'study', '66',
     ['love one another', 'God is love', 'love your neighbor', 'charity', 'love'],
     ['LOVE', 'LOVE OF GOD', 'BROTHERLY LOVE']),
    ('g14', 'How does the Bible say we should pray?', 'study', '66',
     ['pray', 'prayer', 'supplication', 'ask', 'intercession'],
     ['PRAYER', 'PRAYERFULNESS', 'INTERCESSION']),
    ('g15', 'What does the Bible say about faith?', 'study', '66',
     ['faith', 'believe', 'trust', 'faithful'],
     ['FAITH', 'FAITH IN CHRIST', 'TRUST']),
    ('g16', 'What does the Bible say about repentance?', 'study', '66',
     ['repent', 'repentance', 'turn from', 'contrite', 'confess'],
     ['REPENTANCE', 'CONFESSION']),
    ('g17', 'What are the fruits of the Spirit?', 'study', '66',
     ['fruit of the Spirit', 'longsuffering', 'gentleness', 'temperance',
      'love, joy, peace'],
     ['HOLY SPIRIT', 'FRUITS', 'RIGHTEOUSNESS']),
    ('g18', 'What does the Bible say about the resurrection of Jesus?',
     'study', '66',
     ['resurrection', 'risen', 'raised', 'third day', 'empty tomb'],
     ['RESURRECTION', 'JESUS, THE CHRIST']),
    ('g19', 'What does the Bible say about almsgiving and mercy to the poor?',
     'study', 'both',
     ['alms', 'give to the poor', 'needy', 'mercy', 'poor'],
     ['ALMS', 'POOR', 'MERCY', 'LIBERALITY']),
    ('g20', 'What does the Bible say about seeking wisdom and where it comes'
     ' from?', 'study', 'both',
     ['get wisdom', 'fear of Yahweh', 'understanding', 'wisdom', 'prudence'],
     ['WISDOM', 'KNOWLEDGE', 'UNDERSTANDING']),
]

SMOKE = [
    ('s01', 'What does the Bible say about raising children?', 'life'),
    ('s02', 'What does the Bible say about honoring my parents?', 'life'),
    ('s03', 'What does the Bible say about work and laziness?', 'life'),
    ('s04', 'What does the Bible say about lying and telling the truth?', 'life'),
    ('s05', 'What does the Bible say about loneliness?', 'life'),
    ('s06', 'What does the Bible say about lust and sexual sin?', 'life'),
    ('s07', 'What does the Bible say about drunkenness?', 'life'),
    ('s08', 'What does the Bible say about caring for the poor?', 'life'),
    ('s09', 'What does the Bible say about patience and waiting on God?', 'life'),
    ('s10', 'What does the Bible say about gossip and controlling my tongue?', 'life'),
    ('s11', 'What does the Bible say about friendship?', 'life'),
    ('s12', 'What does the Bible say about contentment and envy?', 'life'),
    ('s13', 'What does the Bible say about rest and the Sabbath?', 'life'),
    ('s14', 'What does the Bible say about wisdom?', 'study'),
    ('s15', 'What is the fear of the Lord?', 'study'),
    ('s16', 'What does the Bible say about angels?', 'study'),
    ('s17', 'What does the Bible say about justice?', 'study'),
    ('s18', 'What does the Bible say about fasting?', 'study'),
    ('s19', 'What does the Bible say about giving and generosity?', 'study'),
    ('s20', 'What does the Bible say about idolatry?', 'study'),
]


# ---------------------------------------------------------------------------

class Index(object):
    def __init__(self, con):
        self.con = con
        self.books = {}
        self.canon = {}
        for bid, code, abbrev, name, canon in con.execute(
                'SELECT book_id, usfm_code, abbrev, name, canon FROM books'):
            self.books[bid] = abbrev
            self.canon[bid] = canon

    def fts(self, phrase, canon_mode, limit=FTS_LIMIT):
        """Top verse ids for one keyword or phrase."""
        # A multi-word keyword is matched as a phrase; a single word as a term.
        term = '"%s"' % phrase.replace('"', '') if ' ' in phrase else phrase
        sql = ("SELECT f.rowid FROM verse_fts f"
               " JOIN verses v ON v.verse_id = f.rowid"
               " JOIN books b ON b.book_id = v.book_id"
               " WHERE verse_fts MATCH ?")
        args = [term]
        if canon_mode == '66':
            sql += " AND b.canon = 'protestant'"
        sql += " ORDER BY bm25(verse_fts) LIMIT ?"
        args.append(limit)
        try:
            return [r[0] for r in self.con.execute(sql, args)]
        except sqlite3.OperationalError:
            return []

    def nave_topic(self, heading):
        """Verse ids for a top-level topic, following a see_also pointer once."""
        row = self.con.execute(
            'SELECT topic_id, see_also FROM nave_topics WHERE heading = ?'
            ' AND parent_topic_id IS NULL', (heading,)).fetchone()
        if not row:
            return None, []
        tid, see = row
        vs = [r[0] for r in self.con.execute(
            'SELECT verse_id FROM nave_topic_verses WHERE topic_id = ?', (tid,))]
        if not vs and see:
            for alt in [s.strip() for s in see.split(',')]:
                row2 = self.con.execute(
                    'SELECT topic_id FROM nave_topics WHERE heading = ?'
                    ' AND parent_topic_id IS NULL', (alt,)).fetchone()
                if row2:
                    vs = [r[0] for r in self.con.execute(
                        'SELECT verse_id FROM nave_topic_verses WHERE topic_id = ?',
                        (row2[0],))]
                    if vs:
                        return '%s -> %s' % (heading, alt), vs
        return heading, vs

    def tsk_hop(self, verse_ids):
        if not verse_ids:
            return {}
        out = {}
        chunk = list(verse_ids)
        marks = ','.join('?' * len(chunk))
        for tid, in self.con.execute(
                'SELECT to_verse_id FROM tsk_refs WHERE from_verse_id IN (%s)'
                % marks, chunk):
            out[tid] = out.get(tid, 0) + 1
        return out

    def text(self, verse_id):
        r = self.con.execute('SELECT text FROM verses WHERE verse_id = ?',
                             (verse_id,)).fetchone()
        return r[0] if r else None

    def exists(self, verse_id):
        return self.con.execute('SELECT 1 FROM verses WHERE verse_id = ?',
                                (verse_id,)).fetchone() is not None

    def ref(self, book_id, chapter, v1, v2):
        name = self.books[book_id]
        return ('%s %d:%d' % (name, chapter, v1) if v1 == v2
                else '%s %d:%d-%d' % (name, chapter, v1, v2))


def decode(vid):
    return vid // 1000000, (vid % 1000000) // 1000, vid % 1000


def group_ranges(scored, index):
    """Turn scored verse ids into ranges within a single chapter."""
    ranges = []
    cur = None
    for vid in sorted(scored):
        b, c, v = decode(vid)
        if (cur and cur['book'] == b and cur['chapter'] == c
                and v - cur['last'] <= MERGE_GAP
                and (v - cur['first'] + 1) <= MAX_RANGE_LEN):
            cur['last'] = v
            cur['ids'].append(vid)
        else:
            if cur:
                ranges.append(cur)
            cur = {'book': b, 'chapter': c, 'first': v, 'last': v, 'ids': [vid]}
    if cur:
        ranges.append(cur)

    out = []
    for r in ranges:
        origins = set()
        total = 0
        for vid in r['ids']:
            origins |= scored[vid]['origins']
            total += scored[vid]['score']
        # Fill the range so that verse_ids covers every verse in it, not only
        # the ones that scored: a passage is contiguous by definition.
        ids = []
        for v in range(r['first'], r['last'] + 1):
            vid = r['book'] * 1000000 + r['chapter'] * 1000 + v
            if index.exists(vid):
                ids.append(vid)
        if not ids:
            continue
        # Rank on score density, not raw total: a long range should not beat
        # a short one merely by covering more verses.
        density = total / (len(ids) ** 0.5)
        out.append({
            'ref': index.ref(r['book'], r['chapter'], r['first'], r['last']),
            'verse_ids': ids,
            'origins': sorted(origins),
            'canon': index.canon[r['book']],
            '_score': round(density, 3),
            '_agreement': len(origins),
            '_hits': len(r['ids']),
        })
    return out


def draft(index, q):
    qid, question, category, canon, keywords, nave_names = q
    modes = ['66'] if canon == '66' else ['both']
    canon_mode = '66' if canon == '66' else 'both'

    scored = {}

    def bump(vid, points, origin):
        e = scored.setdefault(vid, {'score': 0, 'origins': set()})
        e['score'] += points
        e['origins'].add(origin)

    # 1. Nave's topics.
    #
    # A topic's weight falls as the topic grows. Membership of a 40-verse topic
    # is strong evidence about a verse; membership of an 1855-verse topic such
    # as AFFLICTIONS AND ADVERSITIES says very little, and left unweighted it
    # swamps the keyword evidence and fills a list with passages that share a
    # theme but do not answer the question asked.
    matched_topics = []
    for name in nave_names:
        label, vs = index.nave_topic(name)
        if label is None or not vs:
            continue
        matched_topics.append('%s (%d)' % (label, len(vs)))
        weight = 3.0 * (200.0 / (200.0 + len(vs)))
        for vid in set(vs):
            b, _, _ = decode(vid)
            if canon_mode == '66' and index.canon[b] != 'protestant':
                continue
            bump(vid, weight, 'nave')

    # 2. FTS keywords, weighted by rank so a top hit counts for more than a
    #    thirtieth one, and summed so a verse matching several keywords wins.
    for kw in keywords:
        for pos, vid in enumerate(index.fts(kw, canon_mode)):
            bump(vid, 4.0 / (1.0 + pos / 5.0), 'fts')

    # 3. Anchors, then one TSK hop. Deliberately light: a cross-reference is a
    #    hint that a passage is related, not evidence that it is central.
    anchors = sorted(scored, key=lambda v: -scored[v]['score'])[:25]
    for vid, n in index.tsk_hop(anchors).items():
        b, _, _ = decode(vid)
        if canon_mode == '66' and index.canon[b] != 'protestant':
            continue
        bump(vid, 0.5 * min(n, 4), 'tsk')

    ranges = group_ranges(scored, index)
    # Score first, agreement as the tie-break: with three sources feeding in,
    # almost everything carries all three tags, so agreement alone discriminates
    # poorly and lets a broad topic outrank a direct answer.
    ranges.sort(key=lambda r: (-r['_score'], -r['_agreement'], -r['_hits'],
                               r['verse_ids'][0]))

    # A MUST passage is one the answer cannot omit, so it must be corroborated:
    # at least two of Nave's, the keyword index and TSK have to point at it.
    # A single-origin passage can still be right, so it drops to SHOULD rather
    # than being discarded. Deuterocanon passages can only ever carry one
    # origin, because neither study corpus indexes those books; that is the
    # known limitation g19 and g20 exist to measure, not a fault in the range.
    strong = [r for r in ranges if r['_agreement'] >= 2]
    weak = [r for r in ranges if r['_agreement'] < 2]
    must = strong[:MUST_MAX]
    rest = strong[MUST_MAX:] + weak
    rest.sort(key=lambda r: (-r['_score'], -r['_agreement'], r['verse_ids'][0]))
    should = rest[:SHOULD_MAX]
    deutero = [r for r in ranges if r['canon'] == 'deutero']

    return {
        'id': qid,
        'question': question,
        'category': category,
        'canon': canon,
        'keywords': keywords,
        'nave_topics': matched_topics or ['none'],
        'must': must,
        'should': should,
        'status': 'draft',
        '_deutero_candidates': len(deutero),
        '_deutero_in_must': len([r for r in must if r['canon'] == 'deutero']),
        '_modes': modes,
    }


def first_words(text, n=10):
    words = re.sub(r'\s+', ' ', text).split(' ')
    out = ' '.join(words[:n])
    return out + (' ...' if len(words) > n else '')


def main():
    con = sqlite3.connect('file:%s?mode=ro' % DB.replace('\\', '/'), uri=True)
    index = Index(con)

    graded = [draft(index, q) for q in GRADED]

    # Every proposed passage must resolve. A passage that does not is a bug.
    for g in graded:
        for bucket in ('must', 'should'):
            for p in g[bucket]:
                for vid in p['verse_ids']:
                    if not index.exists(vid):
                        raise AssertionError('%s %s %s does not resolve'
                                             % (g['id'], p['ref'], vid))
                if not p['verse_ids']:
                    raise AssertionError('%s %s is empty' % (g['id'], p['ref']))

    # ---- JSON
    payload = {
        'schema': 1,
        'status': 'draft',
        'generated_from_index': con.execute(
            "SELECT value FROM meta WHERE key='build_checksum'").fetchone()[0],
        'index_version': con.execute(
            "SELECT value FROM meta WHERE key='index_version'").fetchone()[0],
        'graded': [{k: v for k, v in g.items() if not k.startswith('_')}
                   for g in graded],
        'smoke': [{'id': i, 'question': q, 'category': c} for i, q, c in SMOKE],
    }
    for g in payload['graded']:
        for bucket in ('must', 'should'):
            g[bucket] = [{k: v for k, v in p.items() if not k.startswith('_')}
                         for p in g[bucket]]

    os.makedirs(os.path.dirname(OUT_JSON), exist_ok=True)
    with open(OUT_JSON, 'w', encoding='utf-8', newline='\n') as fh:
        json.dump(payload, fh, indent=2, ensure_ascii=False)
        fh.write('\n')

    # ---- review file
    lines = []
    lines.append('Strike a line to remove that passage from the list.')
    lines.append('Add a line in the same format to include a passage.')
    lines.append('Write APPROVED after a question\'s MUST list when it is final.')
    lines.append('')
    for g in graded:
        lines.append('%s  %s' % (g['id'], g['question']))
        lines.append('MUST (approve these; strike or add)')
        for p in g['must']:
            t = index.text(p['verse_ids'][0])
            tag = ' [deutero]' if p['canon'] == 'deutero' else ''
            lines.append('  %s — %s  [%s]%s'
                         % (p['ref'], first_words(t), ', '.join(p['origins']), tag))
        lines.append('SHOULD (Claude\'s draft, unreviewed, non-gating)')
        for p in g['should']:
            t = index.text(p['verse_ids'][0])
            tag = ' [deutero]' if p['canon'] == 'deutero' else ''
            lines.append('  %s — %s  [%s]%s'
                         % (p['ref'], first_words(t), ', '.join(p['origins']), tag))
        lines.append('')

    with open(OUT_REVIEW, 'w', encoding='utf-8', newline='\n') as fh:
        fh.write('\n'.join(lines))

    # ---- console summary
    print('%-5s %-4s %-6s %-8s %s' % ('id', 'MUST', 'SHOULD', 'deutero', "Nave's topics"))
    for g in graded:
        print('%-5s %-4d %-6d %-8s %s'
              % (g['id'], len(g['must']), len(g['should']),
                 '%d/%d' % (g['_deutero_in_must'], g['_deutero_candidates'])
                 if g['canon'] == 'both' else '-',
                 '; '.join(g['nave_topics'])[:70]))
    print()
    print('MUST lines total:   %d' % sum(len(g['must']) for g in graded))
    print('SHOULD lines total: %d' % sum(len(g['should']) for g in graded))
    con.close()


if __name__ == '__main__':
    main()
