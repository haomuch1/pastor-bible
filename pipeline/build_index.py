"""Build index.db for The Pastor Bible.

Runs on our machines only, never on a user's. Reads the vendored sources in
data/sources/ and writes a single SQLite file.

The build is deterministic: the same sources produce a byte-identical file.
Nothing time-varying is written into any row; the build date lives in meta and
is excluded from the checksum comparison by being written last, in a fixed
order, from a value the caller supplies.

Usage:
    python pipeline/build_index.py [--out PATH] [--date YYYY-MM-DD]
"""

import argparse
import hashlib
import os
import re
import sqlite3
import sys
import tempfile
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)

import books as bk           # noqa: E402
import refs as rf            # noqa: E402
import sword                 # noqa: E402
import usfm                  # noqa: E402

WEB_ZIP = os.path.join(ROOT, 'data', 'sources', 'web', 'eng-web_usfm.zip')
TSK_ZIP = os.path.join(ROOT, 'data', 'sources', 'tsk', 'TSK.zip')
NAVE_ZIP = os.path.join(ROOT, 'data', 'sources', 'naves', 'Nave.zip')

INDEX_VERSION = '0.1.0'
SCHEMA_VERSION = '1'


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, 'rb') as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b''):
            h.update(chunk)
    return h.hexdigest()


def verse_id(book_id, chapter, verse):
    return book_id * 1000000 + chapter * 1000 + verse


# --------------------------------------------------------------------------
# WEB
# --------------------------------------------------------------------------

def load_web():
    """Returns (parsed_books_in_order, omitted_markers)."""
    z = zipfile.ZipFile(WEB_ZIP)
    names = sorted(n for n in z.namelist() if n.lower().endswith('.usfm'))
    parsed = []
    omitted = []
    for n in names:
        src = z.read(n).decode('utf-8-sig')
        b = usfm.parse_book(src)
        if b is None:
            continue
        if b.usfm_code in bk.NON_SCRIPTURE:
            continue
        b.sort_key = n
        parsed.append(b)
        # Verse markers present in the source but carrying no text: the WEB
        # omits these verses and explains the omission in a footnote.
        got = set((c, v) for (c, v, ve, t, p) in b.verses)
        chapter = 0
        for line in src.replace('\r\n', '\n').split('\n'):
            cm = re.match(r'\\c\s+(\d+)', line)
            if cm:
                chapter = int(cm.group(1))
            vm = re.match(r'\\v\s+(\d+)', line)
            if vm and (chapter, int(vm.group(1))) not in got:
                omitted.append((b.usfm_code, chapter, int(vm.group(1))))
    return parsed, omitted


# --------------------------------------------------------------------------
# TSK
# --------------------------------------------------------------------------

def kjv_layout():
    from pysword.canons import canons
    k = canons['kjv']
    ot = [(b[2], b[3]) for b in k['ot']]
    nt = [(b[2], b[3]) for b in k['nt']]
    return ot, nt


# pysword's canon uses its own book keys; map them onto USFM codes by position,
# since the KJV canon order is fixed and shared.
def kjv_keys_to_usfm(ot, nt):
    order = bk.PROTESTANT_66
    mapping = {}
    for i, (key, _) in enumerate(ot):
        mapping[key] = order[i]
    for i, (key, _) in enumerate(nt):
        mapping[key] = order[39 + i]
    return mapping


def load_tsk():
    """Returns dict (usfm_code, chapter, verse) -> raw ThML entry."""
    tmp = tempfile.mkdtemp()
    zipfile.ZipFile(TSK_ZIP).extractall(tmp)
    base = os.path.join(tmp, 'modules', 'comments', 'zcom', 'tsk')

    def rd(name):
        with open(os.path.join(base, name), 'rb') as fh:
            return fh.read()

    ot, nt = kjv_layout()
    keymap = kjv_keys_to_usfm(ot, nt)
    out = {}
    for testament, layout in (('ot', ot), ('nt', nt)):
        got = sword.read_zcom(rd(testament + '.bzs'), rd(testament + '.bzz'),
                              rd(testament + '.bzv'), layout)
        for (key, ch, vs), text in got.items():
            out[(keymap[key], ch, vs)] = text
    return out


_SCRIPREF = re.compile(r'<scripRef(?:\s+passage="([^"]*)")?\s*>(.*?)</scripRef>',
                       re.S | re.I)
_TAG = re.compile(r'<[^>]+>')


def tsk_pairs(entry):
    """Yield (anchor, refs_text) from one TSK entry.

    A TSK entry is a run of "anchor phrase" followed by a <scripRef> holding
    the references for that phrase. Chapter-summary scripRefs carry a passage
    attribute and are skipped: they are navigation, not cross-references.
    """
    pos = 0
    anchor = None
    for m in _SCRIPREF.finditer(entry):
        before = entry[pos:m.start()]
        pos = m.end()
        txt = _TAG.sub(' ', before)
        txt = txt.replace('&nbsp;', ' ').strip()
        txt = re.sub(r'\s+', ' ', txt)
        if txt:
            anchor = txt.rstrip('.').strip() or None
        if m.group(1):
            continue  # chapter-summary link
        body = _TAG.sub(' ', m.group(2))
        body = re.sub(r'\s+', ' ', body).strip()
        if body:
            yield anchor, body


# --------------------------------------------------------------------------
# Nave's
# --------------------------------------------------------------------------

def load_nave():
    tmp = tempfile.mkdtemp()
    zipfile.ZipFile(NAVE_ZIP).extractall(tmp)
    base = os.path.join(tmp, 'modules', 'lexdict', 'zld', 'nave')

    def rd(name):
        with open(os.path.join(base, name), 'rb') as fh:
            return fh.read()

    return sword.read_zld(rd('dict.idx'), rd('dict.dat'),
                          rd('dict.zdx'), rd('dict.zdt'))


_LB_SPLIT = re.compile(r'<lb\s*/>', re.I)
_REF = re.compile(r'<ref\s+osisRef="([^"]*)"[^>]*>(.*?)</ref>', re.S | re.I)
_SEEALSO = re.compile(r'<ref\s+target="Nave:([^"]*)"[^>]*>', re.I)


def nave_subtopics(entry):
    """Split one Nave's entry into (subheading_or_None, osisrefs, see_also).

    Nave's entries are a list of lines introduced by <lb/>, each a subtopic
    with its own references. Sub-numbered lines ("1. A judge of Israel") keep
    their text as the subheading.
    """
    body = entry
    m = re.search(r'<def>(.*)</def>', body, re.S | re.I)
    if m:
        body = m.group(1)
    for part in _LB_SPLIT.split(body):
        osis = [g.group(1) for g in _REF.finditer(part)]
        see = [g.group(1) for g in _SEEALSO.finditer(part)]
        label = _REF.sub(' ', part)
        label = _SEEALSO.sub(' ', label)
        label = _TAG.sub(' ', label)
        label = label.replace('&nbsp;', ' ').replace('\u2192', ' ')
        label = re.sub(r'\s+', ' ', label).strip(' .;:,')
        if not osis and not see and not label:
            continue
        yield (label or None), osis, see


# --------------------------------------------------------------------------
# Build
# --------------------------------------------------------------------------

def build(out_path, build_date):
    parsed, omitted = load_web()

    if os.path.exists(out_path):
        os.remove(out_path)
    con = sqlite3.connect(out_path)
    con.executescript(open(os.path.join(HERE, 'schema.sql'), encoding='utf-8').read())

    # ---- books, pericopes, verses
    book_ids = {}
    pericope_rows = []
    verse_rows = []
    next_pericope = 1
    for order, b in enumerate(parsed, start=1):
        book_ids[b.usfm_code] = order
        con.execute(
            'INSERT INTO books (book_id, usfm_code, name, abbrev, testament,'
            ' canon, book_order) VALUES (?,?,?,?,?,?,?)',
            (order, b.usfm_code, b.name, b.abbrev,
             bk.testament_of(b.usfm_code), bk.canon_of(b.usfm_code), order))

        local_to_global = {}
        used = sorted(set(p for (_, _, _, _, p) in b.verses if p is not None))
        for p in used:
            local_to_global[p] = next_pericope
            next_pericope += 1

        bounds = {}
        for (ch, vs, ve, text, p) in b.verses:
            vid = verse_id(order, ch, vs)
            gp = local_to_global.get(p)
            verse_rows.append((vid, order, ch, vs, ve, text, gp))
            if gp is not None:
                lo, hi = bounds.get(gp, (vid, vid))
                bounds[gp] = (min(lo, vid), max(hi, vid))

        for p in used:
            gp = local_to_global[p]
            heading, source = b.pericopes[p]
            lo, hi = bounds[gp]
            pericope_rows.append((gp, order, lo, hi, heading, source))

    con.executemany(
        'INSERT INTO pericopes (pericope_id, book_id, start_verse_id,'
        ' end_verse_id, heading, source) VALUES (?,?,?,?,?,?)',
        sorted(pericope_rows))
    con.executemany(
        'INSERT INTO verses (verse_id, book_id, chapter, verse, verse_end,'
        ' text, pericope_id) VALUES (?,?,?,?,?,?,?)',
        sorted(verse_rows))
    con.execute("INSERT INTO verse_fts (verse_fts) VALUES ('rebuild')")

    index = rf.VerseIndex((b.usfm_code, ch, vs)
                          for b in parsed for (ch, vs, _, _, _) in b.verses)

    # ---- TSK
    tsk = load_tsk()
    edges = set()
    tsk_unres = []
    for (code, ch, vs), entry in sorted(tsk.items()):
        if code not in book_ids:
            tsk_unres.append((None, '%s %d:%d' % (code, ch, vs),
                              'source book not in this text'))
            continue
        if not index.has(code, ch, vs):
            tsk_unres.append((None, '%s %d:%d' % (code, ch, vs),
                              'source verse not in this text'))
            continue
        from_id = verse_id(book_ids[code], ch, vs)
        for anchor, body in tsk_pairs(entry):
            got, bad = rf.parse_tsk_field(body, code, ch, index)
            for (c2, ch2, v2) in got:
                edges.add((from_id, verse_id(book_ids[c2], ch2, v2), anchor))
            for raw, reason in bad:
                tsk_unres.append((from_id, raw, reason))

    con.executemany('INSERT OR IGNORE INTO tsk_refs (from_verse_id,'
                    ' to_verse_id, anchor) VALUES (?,?,?)',
                    sorted(edges, key=lambda e: (e[0], e[1], e[2] or '')))
    con.executemany('INSERT INTO tsk_unresolved (from_verse_id, raw, reason)'
                    ' VALUES (?,?,?)', sorted(tsk_unres, key=lambda r: (r[0] or 0, r[1], r[2])))

    # ---- Nave's
    nave = load_nave()
    topic_rows = []
    tv_rows = set()
    nave_unres = []
    next_topic = 1
    for key, entry in nave:
        parent_id = next_topic
        topic_rows.append((parent_id, key, None, None))
        next_topic += 1
        for label, osis, see in nave_subtopics(entry):
            if osis:
                tid = next_topic
                topic_rows.append((tid, label or key, parent_id,
                                   ', '.join(see) if see else None))
                next_topic += 1
            else:
                tid = parent_id
                if see:
                    for i, row in enumerate(topic_rows):
                        if row[0] == parent_id and row[3] is None:
                            topic_rows[i] = (row[0], row[1], row[2], ', '.join(see))
                            break
            for o in osis:
                got, reason = rf.resolve_osis(o, index)
                if reason:
                    nave_unres.append((tid, o, reason))
                    continue
                for (c2, ch2, v2) in got:
                    tv_rows.add((tid, verse_id(book_ids[c2], ch2, v2)))
                    tv_rows.add((parent_id, verse_id(book_ids[c2], ch2, v2)))

    con.executemany('INSERT INTO nave_topics (topic_id, heading,'
                    ' parent_topic_id, see_also) VALUES (?,?,?,?)',
                    sorted(topic_rows))
    con.executemany('INSERT OR IGNORE INTO nave_topic_verses (topic_id,'
                    ' verse_id) VALUES (?,?)', sorted(tv_rows))
    con.executemany('INSERT INTO nave_unresolved (topic_id, raw, reason)'
                    ' VALUES (?,?,?)', sorted(nave_unres, key=lambda r: (r[0] or 0, r[1], r[2])))

    # ---- meta
    meta = [
        ('schema_version', SCHEMA_VERSION),
        ('index_version', INDEX_VERSION),
        ('build_date', build_date),
        ('text_name', 'World English Bible Classic'),
        ('text_source', 'https://ebible.org/Scriptures/eng-web_usfm.zip'),
        ('source_sha256_web', sha256_file(WEB_ZIP)),
        ('source_sha256_tsk', sha256_file(TSK_ZIP)),
        ('source_sha256_nave', sha256_file(NAVE_ZIP)),
        ('omitted_verse_markers', str(len(omitted))),
        ('omitted_verse_list', '; '.join('%s %d:%d' % o for o in omitted)),
    ]
    con.executemany('INSERT INTO meta (key, value) VALUES (?,?)', meta)

    con.commit()
    con.execute('VACUUM')
    con.commit()
    con.close()
    return omitted


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--out', default=os.environ.get('TPB_INDEX_DB')
                    or os.path.join(ROOT, 'src-tauri', 'resources',
                                    'index.db'))
    ap.add_argument('--date', default='1970-01-01',
                    help='build date recorded in meta; fixed by default so '
                         'that repeated builds are byte-identical')
    args = ap.parse_args()

    omitted = build(args.out, args.date)
    digest = sha256_file(args.out)

    con = sqlite3.connect(args.out)
    con.execute('UPDATE meta SET value=? WHERE key=?',
                (digest, 'build_checksum'))
    if con.total_changes == 0:
        con.execute('INSERT INTO meta (key, value) VALUES (?,?)',
                    ('build_checksum', digest))
    con.commit()
    con.close()

    print('wrote %s' % args.out)
    print('sha256 before build_checksum row: %s' % digest)
    print('omitted verse markers: %d' % len(omitted))


if __name__ == '__main__':
    main()
