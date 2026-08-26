"""Resolving scripture references from TSK and Nave's onto WEB verse rows.

Nothing here invents a verse. Every reference is resolved against the verse
inventory actually parsed from the WEB; a reference that does not land on a
real row is returned as unresolved, with a reason, and the caller quarantines
it. That is the whole point: a cross-reference the app cannot show is a
cross-reference the app must not claim.
"""

import re

from books import OSIS_TO_USFM, TSK_ABBREV


class VerseIndex(object):
    """The verses that actually exist, for membership tests and expansion."""

    def __init__(self, rows):
        # rows: iterable of (usfm_code, chapter, verse)
        self.by_chapter = {}
        self.present = set()
        for code, ch, vs in rows:
            self.present.add((code, ch, vs))
            self.by_chapter.setdefault((code, ch), []).append(vs)
        for k in self.by_chapter:
            self.by_chapter[k].sort()

    def has(self, code, ch, vs):
        return (code, ch, vs) in self.present

    def chapter_verses(self, code, ch):
        return self.by_chapter.get((code, ch), [])

    def expand(self, code, ch1, v1, ch2, v2):
        """Every existing verse from (ch1,v1) to (ch2,v2) inclusive."""
        out = []
        for ch in range(ch1, ch2 + 1):
            for vs in self.chapter_verses(code, ch):
                if ch == ch1 and vs < v1:
                    continue
                if ch == ch2 and v2 is not None and vs > v2:
                    continue
                out.append((code, ch, vs))
        return out


# --------------------------------------------------------------------------
# Nave's: OSIS references, e.g. Exod.6.16-Exod.6.20, Josh.21.4, 1Chr.24
# --------------------------------------------------------------------------

_OSIS_PART = re.compile(r'^([0-9A-Za-z]+)(?:\.(\d+))?(?:\.(\d+))?$')


def _osis_point(part):
    m = _OSIS_PART.match(part.strip())
    if not m:
        return None
    code = OSIS_TO_USFM.get(m.group(1))
    if code is None:
        return None
    ch = int(m.group(2)) if m.group(2) else None
    vs = int(m.group(3)) if m.group(3) else None
    return code, ch, vs


def resolve_osis(osis_ref, index):
    """Resolve one osisRef attribute to a list of (code, chapter, verse).

    Returns (verses, reason). reason is None on success; on failure verses is
    empty and reason says why.
    """
    ref = osis_ref.strip()
    if not ref:
        return [], 'empty'
    start, _, end = ref.partition('-')
    a = _osis_point(start)
    if a is None:
        return [], 'unknown book or malformed osisRef'
    code, ch1, v1 = a

    if end:
        b = _osis_point(end)
        if b is None or b[0] != code:
            return [], 'malformed or cross-book range'
        _, ch2, v2 = b
    else:
        ch2, v2 = ch1, v1

    if ch1 is None:
        return [], 'book-level reference, too broad to attach'

    if v1 is None:
        # Whole chapter, or a chapter range.
        out = []
        for ch in range(ch1, (ch2 or ch1) + 1):
            out.extend((code, ch, vs) for vs in index.chapter_verses(code, ch))
        if not out:
            return [], 'chapter not present in this text'
        return out, None

    out = index.expand(code, ch1, v1, ch2 or ch1, v2)
    if not out:
        return [], 'verse not present in this text'
    return out, None


# --------------------------------------------------------------------------
# TSK: compact KJV-style reference text, e.g.
#   "Job 26:7; Isa 45:18; Jer 4:23"
#   "Ps 33:6,9; 148:5; Mt 8:3"          (book carries, then chapter carries)
#   "10,12,18,25,31; Ec 2:13"           (bare verses in the current chapter)
#   "Isa 40:12-14"                      (ranges)
# --------------------------------------------------------------------------

_BOOK_RE = re.compile(r'^\s*((?:[123]\s*)?[A-Za-z]+\.?)\s*')
_NUMS_RE = re.compile(r'^\s*(\d+)\s*(?::\s*(\d+))?')


def _norm_book(tok):
    return TSK_ABBREV.get(re.sub(r'[\s.]', '', tok).lower())


def parse_tsk_field(text, ctx_code, ctx_chapter, index):
    """Parse one <scripRef> body.

    ctx_code/ctx_chapter are the book and chapter of the verse the reference
    hangs off, used when the reference gives only bare verse numbers.
    Returns (resolved, unresolved) where resolved is a list of
    (code, chapter, verse) and unresolved is a list of (raw, reason).
    """
    resolved = []
    unresolved = []
    cur_code = ctx_code
    cur_chapter = ctx_chapter
    book_seen = False

    for chunk in re.split(r'[;]', text):
        raw = chunk.strip()
        if not raw:
            continue
        s = raw
        if s.startswith('*'):
            # TSK's own marginal-note markers ("*marg:", "*Gr:", "*title").
            # They introduce a note, not a reference.
            unresolved.append((raw, 'marginal note marker, not a reference'))
            continue
        # _BOOK_RE requires at least one letter, so a bare verse number such as
        # "10" cannot match it, while a numbered book such as "1Pe" can.
        mb = _BOOK_RE.match(s)
        if mb:
            code = _norm_book(mb.group(1))
            if code is None:
                unresolved.append((raw, 'unknown book abbreviation'))
                continue
            cur_code = code
            book_seen = True
            cur_chapter = None
            s = s[mb.end():]

        # The remainder is a comma-separated list of verse or chapter:verse
        # items, possibly with ranges.
        any_ok = False
        for item in s.split(','):
            item = item.strip()
            if not item:
                continue
            # A book name may also open a comma-separated item, as in
            # "Ps 1:1, De 2:36". Re-detect it here, not only after a semicolon.
            imb = _BOOK_RE.match(item)
            if imb:
                icode = _norm_book(imb.group(1))
                if icode is None:
                    unresolved.append((item, 'unknown book abbreviation'))
                    continue
                cur_code = icode
                book_seen = True
                cur_chapter = None
                item = item[imb.end():].strip()
                if not item:
                    continue
            m = re.match(r'^(\d+)\s*:\s*(\d+)\s*-\s*(?:(\d+)\s*:\s*)?(\d+)$', item)
            if m:
                c1, v1 = int(m.group(1)), int(m.group(2))
                c2 = int(m.group(3)) if m.group(3) else c1
                v2 = int(m.group(4))
                cur_chapter = c2
                got = index.expand(cur_code, c1, v1, c2, v2)
            else:
                m = re.match(r'^(\d+)\s*:\s*(\d+)$', item)
                if m:
                    cur_chapter = int(m.group(1))
                    got = ([(cur_code, cur_chapter, int(m.group(2)))]
                           if index.has(cur_code, cur_chapter, int(m.group(2)))
                           else [])
                else:
                    m = re.match(r'^(\d+)\s*-\s*(\d+)$', item)
                    if m and cur_chapter is not None:
                        got = index.expand(cur_code, cur_chapter, int(m.group(1)),
                                           cur_chapter, int(m.group(2)))
                    elif m:
                        # A bare range with no chapter context is a chapter
                        # range only when the book was just named.
                        got = []
                        for ch in range(int(m.group(1)), int(m.group(2)) + 1):
                            got.extend((cur_code, ch, v)
                                       for v in index.chapter_verses(cur_code, ch))
                    else:
                        m = re.match(r'^(\d+)$', item)
                        if not m:
                            unresolved.append((item, 'unparseable reference'))
                            continue
                        n = int(m.group(1))
                        if cur_chapter is None:
                            if book_seen:
                                # "Jude 3" style, or a whole chapter.
                                got = [(cur_code, ch, v)
                                       for ch, v in [(n, x) for x in
                                                     index.chapter_verses(cur_code, n)]]
                                if not got and index.has(cur_code, 1, n):
                                    got = [(cur_code, 1, n)]
                            else:
                                got = []
                        else:
                            got = ([(cur_code, cur_chapter, n)]
                                   if index.has(cur_code, cur_chapter, n) else [])

            if got:
                resolved.extend(got)
                any_ok = True
            else:
                unresolved.append((item, 'no matching verse in this text'))
        if not any_ok and not unresolved:
            unresolved.append((raw, 'nothing resolved'))

    return resolved, unresolved
