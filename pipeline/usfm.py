"""USFM parsing for the World English Bible Classic.

Produces books, verses and pericopes. The text of a verse is the words the WEB
has, with USFM markup removed and nothing added. Footnotes and cross-references
are apparatus, not text, and are dropped; the words inside character-level
markup (words of Jesus, Selah, quoted book titles) are kept.
"""

import re

# Paragraph-level markers that begin a new pericope. Poetry line markers
# (\q1, \q2, \q3) deliberately do NOT appear here: they mark line breaks within
# a poem, and treating each line as its own pericope would shred the Psalms.
PARAGRAPH_MARKERS = {
    'p', 'm', 'pi', 'pi1', 'pi2', 'mi', 'nb', 'pc', 'pr', 'pm', 'pmo',
    'pmc', 'pmr', 'cls', 'li', 'li1', 'li2', 'li3', 'ili', 'ili1', 'ili2',
    'b', 'd', 'sp',
}

# Section-heading markers, which begin a new pericope AND give it a heading.
HEADING_MARKERS = {'s', 's1', 's2', 's3', 's4', 'ms', 'ms1', 'ms2', 'sr', 'r'}

# Markers whose entire content is apparatus and must be discarded, including
# everything up to the matching closing marker.
NOTE_MARKERS = ('f', 'fe', 'x', 'ef', 'ex')


def _strip_notes(s):
    """Remove \\f ... \\f* and \\x ... \\x* spans entirely."""
    for m in NOTE_MARKERS:
        s = re.sub(r'\\' + m + r'\b.*?\\' + m + r'\*', '', s, flags=re.S)
        # An unclosed note runs to end of line; drop it rather than keep junk.
        s = re.sub(r'\\' + m + r'\b.*$', '', s, flags=re.M)
    return s


def _strip_markup(s):
    """Remove remaining USFM markup, keeping the words."""
    s = _strip_notes(s)
    # \w word|strong="H1234"\w*  ->  word    (also \+w nested form)
    s = re.sub(r'\\\+?w\s+([^|\\]*?)(?:\|[^\\]*?)?\\\+?w\*', r'\1', s)
    # Character markers with closing tags: keep the enclosed words.
    s = re.sub(r'\\\+?([a-z]+[0-9]?)\*', '', s)
    # Any remaining marker token, with or without a numeric suffix.
    s = re.sub(r'\\\+?[a-z]+[0-9]?\b\s?', '', s)
    # Attribute leftovers of the form |strong="..."
    s = re.sub(r'\|[a-z\-]+="[^"]*"', '', s)
    s = s.replace('~', '\u00a0')          # USFM non-breaking space
    s = s.replace('//', '')               # optional line break marker
    s = re.sub(r'\s+', ' ', s)
    return s.strip()


class ParsedBook(object):
    def __init__(self, usfm_code):
        self.usfm_code = usfm_code
        self.name = None
        self.abbrev = None
        self.verses = []      # (chapter, verse, verse_end, text, pericope_index)
        self.pericopes = []   # (heading_or_None, source)


def parse_book(text):
    """Parse one USFM file. Returns a ParsedBook, or None if it has no verses."""
    text = text.replace('\r\n', '\n').replace('\r', '\n')
    m = re.search(r'\\id\s+(\S+)', text)
    if not m:
        return None
    book = ParsedBook(m.group(1).upper())

    def first(marker):
        mm = re.search(r'\\' + marker + r'\s+(.+)', text)
        return _strip_markup(mm.group(1)) if mm else None

    book.name = first('toc1') or first('h') or first('mt1') or book.usfm_code
    book.abbrev = first('toc3') or first('h') or book.usfm_code

    chapter = 0
    cur_verse = None          # [chapter, verse, verse_end, [text parts]]
    pending_pericope = None   # (heading, source) awaiting its first verse
    cur_pericope = None       # index into book.pericopes

    def flush_verse():
        if cur_verse is None:
            return
        ch, vs, ve, parts = cur_verse
        body = _strip_markup(' '.join(parts))
        if body:
            book.verses.append((ch, vs, ve, body, cur_pericope))

    # Work line by line; USFM is a line-oriented format.
    for line in text.split('\n'):
        line = line.strip()
        if not line:
            continue
        mk = re.match(r'\\([a-z]+[0-9]?)\b\s*(.*)$', line)
        marker = mk.group(1) if mk else None
        rest = mk.group(2) if mk else line

        if marker == 'c':
            flush_verse()
            cur_verse = None
            cm = re.match(r'(\d+)', rest)
            if cm:
                chapter = int(cm.group(1))
            continue

        if marker in HEADING_MARKERS:
            flush_verse()
            cur_verse = None
            heading = _strip_markup(rest)
            pending_pericope = (heading or None, 'heading')
            continue

        if marker in PARAGRAPH_MARKERS:
            flush_verse()
            cur_verse = None
            # A paragraph break only opens a new pericope if one is not already
            # pending; consecutive markers must not create empty pericopes.
            if pending_pericope is None:
                pending_pericope = (None, 'paragraph')
            if rest.strip():
                line = rest
                marker = None
                rest = line
            else:
                continue

        if marker == 'v' or (marker is None and rest.startswith('\\v ')):
            pass

        # Verse markers can appear mid-line after a paragraph marker.
        if marker == 'v':
            body = rest
        elif re.match(r'^\\v\s', rest):
            body = re.sub(r'^\\v\s', '', rest)
            marker = 'v'
        else:
            body = None

        if marker == 'v' and body is not None:
            flush_verse()
            vm = re.match(r'(\d+)(?:[-\u2013](\d+))?\s*(.*)$', body, re.S)
            if not vm:
                cur_verse = None
                continue
            if pending_pericope is None and cur_pericope is None:
                # A book whose first verse precedes any paragraph marker still
                # needs a pericope; no verse may be left unattached.
                pending_pericope = (None, 'paragraph')
            if pending_pericope is not None:
                book.pericopes.append(pending_pericope)
                cur_pericope = len(book.pericopes) - 1
                pending_pericope = None
            vs = int(vm.group(1))
            ve = int(vm.group(2)) if vm.group(2) else None
            cur_verse = [chapter, vs, ve, [vm.group(3)]]
            continue

        # Continuation of the current verse (poetry lines, wrapped prose).
        if cur_verse is not None and marker not in ('id', 'ide', 'h', 'toc1',
                                                    'toc2', 'toc3', 'mt1',
                                                    'mt2', 'mt3', 'ip', 'is1',
                                                    'cl', 'cp', 'is', 'imt'):
            cur_verse[3].append(rest if marker else line)

    flush_verse()
    return book if book.verses else None
