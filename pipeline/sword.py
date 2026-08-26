"""Readers for the two CrossWire SWORD modules used by this project.

Both modules are public domain. Neither format is documented by a spec we can
cite, so the layouts below were derived by inspection of the files themselves
and are asserted at read time: if a file does not match, the reader raises
rather than returning plausible-looking wrong data.

zCom (Treasury of Scripture Knowledge)
  <t>.bzs  block index, 12 bytes per block: uint32 offset into .bzz,
           uint32 compressed size, uint32 uncompressed size.
           BlockType=BOOK, so there is one block per book.
  <t>.bzz  the zlib-compressed blocks.
  <t>.bzv  entry index, 10 bytes per slot: uint32 block, uint32 offset within
           the decompressed block, uint16 length.
           Slot order within a testament is:
             0        module heading
             1        testament heading
             then, for each book:  book heading, then for each chapter:
             chapter heading, then one slot per verse.
           A slot of length 0 means the module has nothing for it.

zLD (Nave's Topical Bible)
  dict.idx  8 bytes per key: uint32 offset into .dat, uint32 length.
  dict.dat  key + CRLF + uint32 block number + uint32 index within block.
  dict.zdx  8 bytes per block: uint32 offset into .zdt, uint32 compressed size.
  dict.zdt  zlib-compressed blocks. Each decompressed block begins with a
            uint32 entry count, then that many (uint32 offset, uint32 size)
            pairs, then the entry bodies.
"""

import struct
import zlib


def _u32(b, o):
    return struct.unpack_from('<I', b, o)[0]


def read_zcom(bzs, bzz, bzv, book_chapter_counts):
    """Decode one testament of a zCom module.

    book_chapter_counts is an ordered list of (book_key, [verses_in_ch1, ...])
    describing the versification this module was built against. Returns a dict
    mapping (book_key, chapter, verse) -> entry text, skipping empty slots.
    """
    if len(bzs) % 12:
        raise ValueError('bzs length %d is not a multiple of 12' % len(bzs))
    if len(bzv) % 10:
        raise ValueError('bzv length %d is not a multiple of 10' % len(bzv))

    expected = 2 + sum(1 + len(ch) + sum(ch) for _, ch in book_chapter_counts)
    actual = len(bzv) // 10
    if expected != actual:
        raise ValueError('bzv holds %d slots, versification implies %d'
                         % (actual, expected))

    blocks = {}

    def block(n):
        if n not in blocks:
            off, csize, usize = struct.unpack_from('<III', bzs, n * 12)
            raw = zlib.decompress(bzz[off:off + csize])
            if len(raw) != usize:
                raise ValueError('block %d decompressed to %d, header says %d'
                                 % (n, len(raw), usize))
            blocks[n] = raw
        return blocks[n]

    def slot(i):
        bnum, start, size = struct.unpack_from('<IIH', bzv, i * 10)
        if size == 0:
            return None
        return block(bnum)[start:start + size].decode('utf-8', 'replace')

    out = {}
    i = 2  # skip module heading and testament heading
    for book_key, chapters in book_chapter_counts:
        i += 1  # book heading
        for cnum, vcount in enumerate(chapters, start=1):
            i += 1  # chapter heading
            for vnum in range(1, vcount + 1):
                text = slot(i)
                i += 1
                if text:
                    out[(book_key, cnum, vnum)] = text
    return out


def read_zld(idx, dat, zdx, zdt):
    """Decode a zLD module. Returns an ordered list of (key, entry_text)."""
    if len(idx) % 8:
        raise ValueError('dict.idx length %d is not a multiple of 8' % len(idx))
    if len(zdx) % 8:
        raise ValueError('dict.zdx length %d is not a multiple of 8' % len(zdx))

    blocks = []
    for i in range(len(zdx) // 8):
        off, csize = struct.unpack_from('<II', zdx, i * 8)
        raw = zlib.decompress(zdt[off:off + csize])
        count = _u32(raw, 0)
        ents = []
        for j in range(count):
            eo = _u32(raw, 4 + j * 8)
            es = _u32(raw, 8 + j * 8)
            ents.append(raw[eo:eo + es])
        blocks.append(ents)

    out = []
    for i in range(len(idx) // 8):
        off, size = struct.unpack_from('<II', idx, i * 8)
        rec = dat[off:off + size]
        key, _, rest = rec.partition(b'\r\n')
        if len(rest) < 8:
            continue
        bnum, enum = struct.unpack_from('<II', rest, 0)
        if bnum >= len(blocks) or enum >= len(blocks[bnum]):
            continue
        out.append((key.decode('utf-8', 'replace'),
                    blocks[bnum][enum].decode('utf-8', 'replace')))
    return out
