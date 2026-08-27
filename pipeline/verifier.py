"""The mechanical citation guarantee. Specified in docs/VERIFIER.md.

This is the module that keeps the project's central promise: no reference
reaches a reader unless the passages behind it were actually retrieved. It does
not trust the prompt, because the prompt is an instruction and the model is not
obliged to follow it.

Errors here are asymmetric. Missing a fabricated reference puts a false
citation in front of a reader. Flagging a real phrase costs one retry. The
rules lean towards detection, and the non-detection list keeps that lean from
mangling ordinary prose.

P4 ports this to Rust. TEST_VECTORS is the shared contract.
"""

import os
import re
import sqlite3
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)

DB = os.environ.get('TPB_INDEX_DB') or os.path.join(
    ROOT, 'src-tauri', 'resources', 'index.db')

TOKEN_RE = re.compile(r'\[P(\d+)\]')

# Book names that are also ordinary English words. These are matched
# case-sensitively so "he acts 2 ways" is not read as a citation of Acts.
AMBIGUOUS = {
    'job', 'mark', 'acts', 'numbers', 'judges', 'kings', 'song', 'songs',
    'revelation', 'chronicles', 'romans', 'hebrews', 'lamentations', 'kings',
    'proverbs', 'psalm', 'psalms', 'james', 'philemon', 'ruth', 'wisdom',
}

# A number followed by one of these is a count, not a chapter.
UNIT_NOUNS = {
    'times', 'time', 'days', 'day', 'years', 'year', 'months', 'month',
    'weeks', 'week', 'hours', 'hour', 'people', 'men', 'women', 'sons',
    'daughters', 'tribes', 'thousand', 'hundred', 'million', 'percent',
    'degrees',
}

ORDINAL_PREFIX = {
    'first': '1', '1st': '1', 'i': '1', '1': '1',
    'second': '2', '2nd': '2', 'ii': '2', '2': '2',
    'third': '3', '3rd': '3', 'iii': '3', '3': '3',
}


def _norm(tok):
    return re.sub(r'[\s.]', '', tok).lower()


# Common English names of books that neither the WEB's own long titles nor the
# TSK abbreviation table spells out. Without these, "Song of Solomon 3:1" and
# every deuterocanonical reference are invisible to Rule B: measured on
# 2026-08-26, 14 of 83 realistic book names were undetected, and 11 of the 14
# were the Deuterocanon, which is exactly what a both-canon answer cites.
ALIASES = {
    'SNG': ['Song of Songs', 'Song of Solomon', 'Canticles'],
    'ACT': ['Acts of the Apostles'],
    'WIS': ['Wisdom of Solomon', 'Wisdom'],
    'SIR': ['Ecclesiasticus', 'Sirach', 'Ben Sira'],
    'MAN': ['Prayer of Manasseh', 'Prayer of Manasses'],
    'ESG': ['Greek Esther'],
    'DAG': ['Greek Daniel'],
    '1ES': ['1 Esdras'],
    '2ES': ['2 Esdras'],
    '1MA': ['1 Maccabees'],
    '2MA': ['2 Maccabees'],
    '3MA': ['3 Maccabees'],
    '4MA': ['4 Maccabees'],
}


class BookNames(object):
    """Maps written book names onto book ids, from the index itself."""

    def __init__(self, con):
        self.by_name = {}
        # The written form each normalised key came from, so the reference
        # pattern can be rebuilt with its spaces intact. Without it a
        # multi-word name is escaped as one run of letters and can never match.
        self.written_of = {}
        self.ambiguous_ids = set()
        rows = con.execute(
            'SELECT book_id, usfm_code, name, abbrev FROM books').fetchall()
        for book_id, code, name, abbrev in rows:
            for variant in self._variants(code, name, abbrev):
                key = _norm(variant)
                if key not in self.by_name:
                    self.by_name[key] = book_id
                    self.written_of[key] = variant
                if key in AMBIGUOUS:
                    self.ambiguous_ids.add(book_id)
        # Abbreviations and common forms the source does not spell out.
        try:
            from books import TSK_ABBREV
            codes = {c: b for b, c in con.execute(
                'SELECT book_id, usfm_code FROM books')}
            for abbr, code in TSK_ABBREV.items():
                if code in codes:
                    key = _norm(abbr)
                    if key not in self.by_name:
                        self.by_name[key] = codes[code]
                        self.written_of[key] = abbr
        except Exception:  # noqa: BLE001
            pass

    @staticmethod
    def _variants(code, name, abbrev):
        out = [code, abbrev]
        # The long name is "The First Book of Moses, Commonly Called Genesis";
        # the useful part is the last word or two.
        tail = re.sub(r'^.*\bCalled\b\s*', '', name).strip()
        out.append(tail)
        out.append(name)
        # "The Song of Solomon" is how the WEB titles the book; "Song of
        # Solomon" is how anyone writes it.
        for v in (tail, name):
            if v.lower().startswith('the '):
                out.append(v[4:])
        out.extend(ALIASES.get(code, []))
        # Numeric-prefixed books: 1SA -> "1 Samuel", "First Samuel",
        # "I Samuel", all normalising to the same key.
        m = re.match(r'^([123])(.*)$', code)
        if m and tail:
            base = re.sub(r'^[123]\s*', '', tail)
            n = m.group(1)
            words = {'1': ('first', 'i'), '2': ('second', 'ii'),
                     '3': ('third', 'iii')}[n]
            for pre in (n, n + ' ') + tuple(w + ' ' for w in words):
                out.append('%s%s' % (pre, base))
        return [v for v in out if v]

    def lookup(self, written):
        key = _norm(written)
        hit = self.by_name.get(key)
        if hit is not None:
            return hit
        # "First Corinthians", "I Corinthians", "1st Corinthians" all mean
        # 1 Corinthians. Rewrite the prefix to its digit and try again. The
        # rewrite is only accepted when the result is a book we know, so
        # "Isaiah" is not mangled into "1saiah".
        for word, digit in sorted(ORDINAL_PREFIX.items(),
                                  key=lambda kv: -len(kv[0])):
            if key.startswith(word) and len(key) > len(word):
                hit = self.by_name.get(digit + key[len(word):])
                if hit is not None:
                    return hit
        return None


ORDINAL_ALT = {'1': 'First|1st|I', '2': 'Second|2nd|II', '3': 'Third|3rd|III'}


def book_alternative(written):
    """A permissive pattern for one written book name.

    The name is matched word by word so that a multi-word title keeps its
    spaces. An earlier version built the pattern from the normalised key, which
    has the spaces stripped, so "Song of Solomon 3:1" and every
    deuterocanonical reference slipped past Rule B unflagged.
    """
    parts = []
    for i, tok in enumerate(written.split()):
        m = re.match(r'^([123])(.*)$', tok) if i == 0 else None
        if m:
            pre, rest = m.group(1), m.group(2)
            head = r'(?:%s|%s)' % (pre, ORDINAL_ALT[pre])
            parts.append(head + (r'\s*\.?\s*' + re.escape(rest) if rest else ''))
        else:
            parts.append(re.escape(tok))
    # A full stop may follow any word of an abbreviated name.
    return r'\.?\s+'.join(parts)


def build_reference_re(book_names):
    """One alternation over every known written form, longest first."""
    keys = sorted(book_names.by_name, key=len, reverse=True)
    alts = []
    seen = set()
    for k in keys:
        if k in seen:
            continue
        seen.add(k)
        alts.append(book_alternative(book_names.written_of.get(k, k)))
    body = '|'.join(alts)
    return re.compile(
        r'\b(?P<book>%s)\s*\.?\s+(?P<chapter>\d{1,3})'
        r'(?:\s*[:.]\s*(?P<verse>\d{1,3})'
        r'(?:\s*[-–—]\s*(?P<verse_end>\d{1,3}))?)?' % body,
        re.IGNORECASE)


class Verifier(object):
    def __init__(self, db_path=DB):
        self.con = sqlite3.connect('file:%s?mode=ro' % db_path.replace('\\', '/'),
                                   uri=True)
        self.books = BookNames(self.con)
        self.ref_re = build_reference_re(self.books)
        self.chapter_sizes = {}

    def chapter_verses(self, book_id, chapter):
        key = (book_id, chapter)
        if key not in self.chapter_sizes:
            self.chapter_sizes[key] = {
                r[0] for r in self.con.execute(
                    'SELECT verse FROM verses WHERE book_id=? AND chapter=?',
                    (book_id, chapter))}
        return self.chapter_sizes[key]

    # -- the two rules ----------------------------------------------------

    def check(self, text, passages):
        """passages: list of dicts with 'token' and 'verse_ids'.

        Returns (verdict, violations). verdict is 'ok' or 'violation'.
        """
        sent_tokens = {p['token'] for p in passages}
        sent_verses = set()
        for p in passages:
            sent_verses.update(p['verse_ids'])

        violations = []

        # Rule A
        for m in TOKEN_RE.finditer(text):
            tok = '[P%s]' % m.group(1)
            if tok not in sent_tokens:
                violations.append({'kind': 'token', 'text': tok,
                                   'reason': 'token not in sent set',
                                   'span': m.span()})

        # Rule B
        for m in self.ref_re.finditer(text):
            written = m.group(0)
            book_written = m.group('book')
            book_id = self.books.lookup(book_written)
            if book_id is None:
                continue
            # An ambiguous book name must be capitalised to count.
            if _norm(book_written) in AMBIGUOUS and not book_written[:1].isupper():
                continue
            # A number followed by a unit noun is a count, not a chapter.
            after = text[m.end():m.end() + 24]
            nxt = re.match(r'\s+([A-Za-z]+)', after)
            if (m.group('verse') is None and nxt
                    and nxt.group(1).lower() in UNIT_NOUNS):
                continue

            chapter = int(m.group('chapter'))
            if m.group('verse') is None:
                wanted = {book_id * 1000000 + chapter * 1000 + v
                          for v in self.chapter_verses(book_id, chapter)}
                reason = 'whole chapter not in sent set'
                if not wanted:
                    reason = 'chapter not in this text'
            else:
                v1 = int(m.group('verse'))
                v2 = int(m.group('verse_end') or v1)
                wanted = {book_id * 1000000 + chapter * 1000 + v
                          for v in range(v1, v2 + 1)}
                reason = 'verses not in sent set'
            if not wanted or not wanted <= sent_verses:
                violations.append({'kind': 'reference', 'text': written,
                                   'reason': reason, 'span': m.span()})

        return ('violation' if violations else 'ok'), violations

    @staticmethod
    def strip(text, violations):
        """Remove offending spans, right to left so offsets stay valid."""
        out = text
        for v in sorted(violations, key=lambda v: -v['span'][0]):
            a, b = v['span']
            out = out[:a] + out[b:]
        return re.sub(r'[ \t]{2,}', ' ', out)

    @staticmethod
    def failure_note(violations):
        """The text handed back to the model on the retry."""
        parts = []
        for v in violations:
            parts.append('%s "%s" (%s)' % (v['kind'], v['text'], v['reason']))
        return '; '.join(parts)

    def fallback(self, passages):
        """PLAN 5.6's fallback: passages grouped by book, with the note."""
        by_book = {}
        for p in passages:
            bid = p['verse_ids'][0] // 1000000
            by_book.setdefault(bid, []).append(p)
        names = dict(self.con.execute('SELECT book_id, abbrev FROM books'))
        order = sorted(by_book)
        lines = ['A synthesis could not be produced for this question. '
                 'These are the passages that were found.']
        for bid in order:
            lines.append('')
            lines.append(names.get(bid, '?'))
            for p in sorted(by_book[bid], key=lambda p: p['verse_ids'][0]):
                lines.append('  %s' % p.get('ref', p['token']))
        return '\n'.join(lines)


# ---------------------------------------------------------------------------
# Test vectors. The contract shared with the Rust port. Sent set:
#   [P1] Matthew 6:25-34   [P2] Philippians 4:6-7
#   [P3] Psalm 23:1-4      [P4] 1 Peter 5:6-7
# ---------------------------------------------------------------------------

SENT_SPEC = [
    ('P1', 'MAT', 6, 25, 34),
    ('P2', 'PHP', 4, 6, 7),
    ('P3', 'PSA', 23, 1, 4),
    ('P4', '1PE', 5, 6, 7),
]

TEST_VECTORS = [
    # 1-8 must pass
    ('Jesus tells us not to worry [P1].', 'ok'),
    ('Anxiety is answered by prayer [P1] [P2].', 'ok'),
    ('As Matthew 6:25 says, do not be anxious [P1].', 'ok'),
    ('See Mat 6:26 for the birds [P1].', 'ok'),
    ('The passage Matthew 6:25-34 covers this [P1].', 'ok'),
    ('The passage Matthew 6:25–34 covers this [P1].', 'ok'),
    ('The text speaks plainly about worry and trust.', 'ok'),
    ('Trust God ([P4]), and be humble.', 'ok'),
    # 9-17 must be flagged
    ('Consider also [P9], which speaks to this.', 'violation'),
    ('See [P0] for more.', 'violation'),
    ('As John 3:16 tells us, God loved the world.', 'violation'),
    ('Matthew 5:3 speaks of the poor in spirit.', 'violation'),
    ('Read Matthew 6:25-40 for the whole argument.', 'violation'),
    ('Psalm 23 is the shepherd psalm.', 'violation'),
    ('1 Corinthians 13:4 defines love.', 'violation'),
    ('First Corinthians 13 defines love.', 'violation'),
    ('I Corinthians 13 defines love.', 'violation'),
    # 18-25 must not be flagged
    ('He rose on the third day, as the passages say [P1].', 'ok'),
    ('Forgive seven times, and then seventy [P1].', 'ok'),
    ('He could quote chapter and verse on it [P2].', 'ok'),
    ('Job suffered greatly, yet he trusted [P3].', 'ok'),
    ('Mark was a companion on the journey [P1].', 'ok'),
    ('He acts 2 ways depending on who is watching [P2].', 'ok'),
    ('Judges 12 times refused to listen to the warning [P1].', 'ok'),
    ('Numbers and Kings and Acts all tell parts of the story [P3].', 'ok'),
    # 26-31 multi-word and deuterocanonical names must be flagged. Before
    # 2026-08-26 every one of these passed as clean prose.
    ('Song of Solomon 2:1 speaks of love.', 'violation'),
    ('Song of Songs 2:1 speaks of love.', 'violation'),
    ('The Acts of the Apostles 2:38 records the sermon.', 'violation'),
    ('Wisdom of Solomon 3:1 says the souls of the righteous are in God hands.',
     'violation'),
    ('1 Maccabees 2:15 tells of the revolt.', 'violation'),
    ('Sirach 3:1 counsels children to honour their parents.', 'violation'),
    # 32-35 the same names in ordinary prose, with no chapter number, must not
    # be flagged. Detection must not cost the reader a retry over a sentence.
    ('The song of songs was sung by the whole congregation [P3].', 'ok'),
    ('The acts of the apostles were many and are still told [P1].', 'ok'),
    ('Wisdom builds her house and calls out to the simple [P2].', 'ok'),
    ('He read the prayer of Manasseh aloud at the vigil [P4].', 'ok'),
]


def sent_set(con):
    """Build the test sent set from the real index."""
    out = []
    for token, code, ch, v1, v2 in SENT_SPEC:
        bid = con.execute('SELECT book_id FROM books WHERE usfm_code=?',
                          (code,)).fetchone()[0]
        ids = [bid * 1000000 + ch * 1000 + v for v in range(v1, v2 + 1)]
        ids = [i for i in ids if con.execute(
            'SELECT 1 FROM verses WHERE verse_id=?', (i,)).fetchone()]
        out.append({'token': '[%s]' % token, 'verse_ids': ids,
                    'ref': '%s %d:%d-%d' % (code, ch, v1, v2)})
    return out


def run_vectors(db_path=DB):
    v = Verifier(db_path)
    passages = sent_set(v.con)
    results = []
    for i, (text, expected) in enumerate(TEST_VECTORS, start=1):
        got, violations = v.check(text, passages)
        results.append((i, text, expected, got, violations))
    return results


if __name__ == '__main__':
    sys.stdout.reconfigure(encoding='utf-8')
    bad = 0
    for i, text, expected, got, violations in run_vectors():
        mark = 'ok  ' if got == expected else 'FAIL'
        if got != expected:
            bad += 1
        print('%s %2d  expected %-9s got %-9s  %s' % (mark, i, expected, got,
                                                      text[:58]))
        if got != expected and violations:
            for vi in violations:
                print('         -> %s %r (%s)' % (vi['kind'], vi['text'],
                                                  vi['reason']))
    print()
    print('%d/%d vectors pass' % (len(TEST_VECTORS) - bad, len(TEST_VECTORS)))
