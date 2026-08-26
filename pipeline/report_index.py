"""Report the contents of a built index.db.

Opens the database file and derives every number from it by query. Nothing here
reads the sources or trusts a figure the build reported: if the build got it
wrong, this says so.

Usage:  python pipeline/report_index.py [PATH]
"""

import os
import sqlite3
import sys

sys.stdout.reconfigure(encoding='utf-8')

DEFAULT = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                       'src-tauri', 'resources', 'index.db')

SPOT_CHECKS = [
    ('GEN', 1, 1), ('PSA', 23, 1), ('JHN', 3, 16), ('REV', 22, 21), ('TOB', 1, 1),
]


def main():
    path = sys.argv[1] if len(sys.argv) > 1 else DEFAULT
    con = sqlite3.connect('file:%s?mode=ro' % path.replace('\\', '/'), uri=True)
    q = lambda s, *a: con.execute(s, a).fetchall()
    one = lambda s, *a: con.execute(s, a).fetchone()[0]

    print('file: %s' % path)
    print('size: %d bytes' % os.path.getsize(path))
    print()

    print('== meta ==')
    for k, v in q('SELECT key, value FROM meta ORDER BY key'):
        if k == 'omitted_verse_list':
            v = v[:100] + ('...' if len(v) > 100 else '')
        print('  %-24s %s' % (k, v))
    print()

    print('== books ==')
    for canon, n in q('SELECT canon, COUNT(*) FROM books GROUP BY canon ORDER BY canon'):
        print('  %-12s %d' % (canon, n))
    print('  %-12s %d' % ('total', one('SELECT COUNT(*) FROM books')))
    for t, c, n in q('SELECT testament, canon, COUNT(*) FROM books'
                     ' GROUP BY testament, canon ORDER BY testament, canon'):
        print('    %s / %-11s %d' % (t, c, n))
    print()

    print('== chapters ==')
    print('  total chapters: %d' % one(
        'SELECT COUNT(*) FROM (SELECT DISTINCT book_id, chapter FROM verses)'))
    for canon, n in q('SELECT b.canon, COUNT(*) FROM (SELECT DISTINCT book_id,'
                      ' chapter FROM verses) v JOIN books b USING (book_id)'
                      ' GROUP BY b.canon ORDER BY b.canon'):
        print('  %-12s %d' % (canon, n))
    print()

    print('== verses ==')
    for canon, n in q('SELECT b.canon, COUNT(*) FROM verses v JOIN books b'
                      ' USING (book_id) GROUP BY b.canon ORDER BY b.canon'):
        print('  %-12s %d' % (canon, n))
    for t, n in q('SELECT b.testament, COUNT(*) FROM verses v JOIN books b'
                  ' USING (book_id) WHERE b.canon=\'protestant\''
                  ' GROUP BY b.testament ORDER BY b.testament'):
        print('  protestant %-6s %d' % (t, n))
    total = one('SELECT COUNT(*) FROM verses')
    prot = one("SELECT COUNT(*) FROM verses v JOIN books b USING (book_id)"
               " WHERE b.canon='protestant'")
    print('  %-12s %d' % ('total', total))
    print()
    print('  protestant total vs KJV 31,102: %+d' % (prot - 31102))
    print('  verse bridges: %d' % one('SELECT COUNT(*) FROM verses WHERE verse_end IS NOT NULL'))
    for r in q('SELECT b.usfm_code, v.chapter, v.verse, v.verse_end FROM verses v'
               ' JOIN books b USING (book_id) WHERE v.verse_end IS NOT NULL'
               ' ORDER BY v.verse_id'):
        print('    %s %d:%d-%d' % r)
    print()

    print('== pericopes ==')
    print('  total: %d' % one('SELECT COUNT(*) FROM pericopes'))
    for src, n in q('SELECT source, COUNT(*) FROM pericopes GROUP BY source ORDER BY source'):
        print('  by %-10s %d' % (src, n))
    print('  with heading: %d' % one('SELECT COUNT(*) FROM pericopes WHERE heading IS NOT NULL'))
    print('  heading NULL: %d' % one('SELECT COUNT(*) FROM pericopes WHERE heading IS NULL'))
    print('  books with at least one heading: %d' % one(
        'SELECT COUNT(DISTINCT book_id) FROM pericopes WHERE heading IS NOT NULL'))
    for r in q('SELECT b.usfm_code, p.heading FROM pericopes p JOIN books b'
               ' USING (book_id) WHERE p.heading IS NOT NULL ORDER BY p.pericope_id'):
        print('    %-5s %s' % r)
    print('  verses with no pericope: %d' % one(
        'SELECT COUNT(*) FROM verses WHERE pericope_id IS NULL'))
    print()

    print('== TSK ==')
    print('  edges:                %d' % one('SELECT COUNT(*) FROM tsk_refs'))
    print('  distinct source verses: %d' % one('SELECT COUNT(DISTINCT from_verse_id) FROM tsk_refs'))
    print('  distinct target verses: %d' % one('SELECT COUNT(DISTINCT to_verse_id) FROM tsk_refs'))
    print('  unresolved:           %d' % one('SELECT COUNT(*) FROM tsk_unresolved'))
    for reason, n in q('SELECT reason, COUNT(*) FROM tsk_unresolved GROUP BY reason'
                       ' ORDER BY COUNT(*) DESC'):
        print('    %-34s %d' % (reason, n))
    print('  sample unresolved:')
    for raw, reason in q('SELECT raw, reason FROM tsk_unresolved LIMIT 6'):
        print('    %-24r %s' % (raw, reason))
    print()

    print("== Nave's ==")
    print('  topics total:      %d' % one('SELECT COUNT(*) FROM nave_topics'))
    print('  top-level topics:  %d' % one('SELECT COUNT(*) FROM nave_topics WHERE parent_topic_id IS NULL'))
    print('  subtopics:         %d' % one('SELECT COUNT(*) FROM nave_topics WHERE parent_topic_id IS NOT NULL'))
    print('  topic-verse rows:  %d' % one('SELECT COUNT(*) FROM nave_topic_verses'))
    print('  distinct verses:   %d' % one('SELECT COUNT(DISTINCT verse_id) FROM nave_topic_verses'))
    print('  topics with verses: %d' % one('SELECT COUNT(DISTINCT topic_id) FROM nave_topic_verses'))
    print('  unresolved:        %d' % one('SELECT COUNT(*) FROM nave_unresolved'))
    for reason, n in q('SELECT reason, COUNT(*) FROM nave_unresolved GROUP BY reason'
                       ' ORDER BY COUNT(*) DESC'):
        print('    %-38s %d' % (reason, n))
    print()

    print('== FTS5 ==')
    fts = one('SELECT COUNT(*) FROM verse_fts')
    print('  indexed rows: %d   verses: %d   equal: %s' % (fts, total, fts == total))
    hits = one("SELECT COUNT(*) FROM verse_fts WHERE verse_fts MATCH 'anxious'")
    print("  MATCH 'anxious': %d hits" % hits)
    for r in q("SELECT b.usfm_code, v.chapter, v.verse, substr(v.text,1,70)"
               " FROM verse_fts f JOIN verses v ON v.verse_id=f.rowid"
               " JOIN books b USING (book_id) WHERE verse_fts MATCH 'anxious'"
               " ORDER BY v.verse_id LIMIT 3"):
        print('    %s %d:%d  %s...' % r)
    print()

    print('== spot checks ==')
    for code, ch, vs in SPOT_CHECKS:
        row = one("SELECT COUNT(*) FROM verses v JOIN books b USING (book_id)"
                  " WHERE b.usfm_code=? AND v.chapter=? AND v.verse=?", code, ch, vs)
        if not row:
            print('  %-5s %d:%-3d MISSING' % (code, ch, vs))
            continue
        txt = one("SELECT v.text FROM verses v JOIN books b USING (book_id)"
                  " WHERE b.usfm_code=? AND v.chapter=? AND v.verse=?", code, ch, vs)
        print('  %-5s %d:%-3d %s' % (code, ch, vs, txt[:110]))
    print()

    print('== integrity ==')
    print('  verses with unknown book:   %d' % one(
        'SELECT COUNT(*) FROM verses v LEFT JOIN books b USING (book_id) WHERE b.book_id IS NULL'))
    print('  tsk edges with dead source: %d' % one(
        'SELECT COUNT(*) FROM tsk_refs t LEFT JOIN verses v ON v.verse_id=t.from_verse_id WHERE v.verse_id IS NULL'))
    print('  tsk edges with dead target: %d' % one(
        'SELECT COUNT(*) FROM tsk_refs t LEFT JOIN verses v ON v.verse_id=t.to_verse_id WHERE v.verse_id IS NULL'))
    print('  nave rows with dead verse:  %d' % one(
        'SELECT COUNT(*) FROM nave_topic_verses n LEFT JOIN verses v USING (verse_id) WHERE v.verse_id IS NULL'))
    print('  foreign_key_check rows:     %d' % len(q('PRAGMA foreign_key_check')))
    print('  integrity_check:            %s' % one('PRAGMA integrity_check'))
    con.close()


if __name__ == '__main__':
    main()
