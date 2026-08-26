# HANDOFF

Session: P1 Ingestion
Date: 2026-08-26
Status: P1 COMPLETE

## State

The repository is at D:\Haomuch-Programs\The-Pastor-Bible, branch main, pushed
to https://github.com/haomuch1/pastor-bible, still private.

Carried over from P0 and now closed:

  The Code of Conduct's "[INSERT CONTACT METHOD]" placeholder is filled in.
  README 9.4 and PLAN 9.4 both now say the signing application is still to
  come, so no committed text claims something untrue.
  data/crisis_terms.txt exists, holding comments only and saying so in its own
  text.

Sources vendored, unmodified, as the archives exactly as downloaded:

  data/sources/web/eng-web_usfm.zip     3,244,005 bytes
  data/sources/tsk/TSK.zip              2,643,739 bytes
  data/sources/naves/Nave.zip           1,300,879 bytes

That is 7.2 MB in total. No single file is anywhere near the 50 MB threshold,
so no decision about Git LFS is needed.

The pipeline is in pipeline/: books.py holds the canon tables and the reference
abbreviation maps; usfm.py parses the WEB; sword.py decodes the two CrossWire
binary formats; refs.py resolves references onto real verse rows;
schema.sql defines the database; build_index.py orchestrates; report_index.py
reads a finished database back and prints what is in it.

pipeline/requirements.txt pins pysword 0.2.8 and pytest 9.1.1. The virtual
environment is at pipeline/.venv and is gitignored.

tests/test_index.py holds 28 tests that open index.db and assert the P1
invariants.

index.db is written to src-tauri/resources/index.db. It is 76,197,888 bytes and
is gitignored, as planned: it is a build product, published later as a release
asset, never committed.

## Verified

Prerequisites: Python 3.13.14, pip 26.1.2, SQLite 3.50.4 through Python's
sqlite3 module. FTS5 was confirmed twice: it appears in PRAGMA compile_options
as ENABLE_FTS5, and a scratch FTS5 table was created, populated and matched
against successfully. There is no sqlite3 command line binary on this machine
and none is needed.

The World English Bible Classic was verified to be the edition the plan names,
by reading the downloaded text rather than trusting the catalogue. It contains
"Yahweh" 6,902 times, twelve of them in Genesis 2. US spellings dominate
decisively: savior 52 against saviour 0, labor 200 against labour 0, neighbor
207 against neighbour 0, color 17 against colour 0, favor 163 against favour 0,
honor 342 against honour 1. The archive's own copyright file describes it as
"the Classic World English Bible with the full ecumenical book set" and states
that the text is in the public domain.

The deuterocanonical set found in the files is exactly the set plan 4.2
expects, with no additions and nothing missing. Fifteen books: Tobit, Judith,
Greek Esther, Wisdom, Sirach, Baruch, 1 and 2 Maccabees, 1 and 2 Esdras, the
Prayer of Manasseh, Psalm 151, 3 and 4 Maccabees, and Greek Daniel. Plan 4.2
expects the Letter of Jeremiah to appear as Baruch chapter 6; Baruch was
checked and has six chapters, the sixth headed "The Letter of Jeremy
(Jeremiah)". Greek Daniel carries the Song of the Three, Susanna, and Bel and
the Dragon, as 4.2 expects.

Both study corpora declare their own licence inside the files themselves:
mods.d/tsk.conf and mods.d/nave.conf each say DistributionLicense=Public
Domain. That is recorded in NOTICE.md with the URL, retrieval date and
checksum of each archive.

Every count below was read back out of the finished index.db by query, in a
process separate from the one that built it. These are the reported numbers;
figures the build itself printed are not.

  books                    81     66 protestant, 15 deuterocanonical
  chapters               1402     1189 protestant, 213 deuterocanonical
  verses                38029     31098 protestant, 6931 deuterocanonical
    protestant OT       23145
    protestant NT        7953
  verse bridges             1     4 Maccabees 8:28-29
  pericopes             10052     10 from headings, 10042 from paragraphs
  verses with no pericope   0
  TSK edges            593670     from 26202 verses, onto 31045 verses
  TSK unresolved         1380     quarantined, not dropped
  Nave topics           18837     5322 top level, 13515 subtopics
  Nave topic-verse rows 399374     over 30982 distinct verses
  Nave unresolved          31     quarantined, not dropped
  FTS5 rows             38029     equal to the verse count

The 31,102 check resolves cleanly and nothing was adjusted to make it do so.
31,102 is the King James verse total. The WEB's own verse markers number
31,103, matching eBible's published figure. Of those markers, five carry no
text at all: Luke 17:36, Acts 8:37, Acts 15:34, Acts 24:7 and Romans 16:25,
each one a verse the WEB omits, each with a footnote in the source explaining
the omission. That leaves 31,098 verses that actually have text, which is what
the database holds. Against the King James figure the difference is four, not
five, because Romans is a special case: the WEB places the doxology at Romans
14:24-26, three verses the King James numbers 16:25-27, so Romans' own total is
unchanged. Twenty-nine such empty markers exist across the whole corpus, 24 of
them in Sirach; the count and the full list are stored in meta.

Spot checks were read out of the database and compared against the raw USFM.
Genesis 1:1, Psalm 23:1, John 3:16, Revelation 22:21 and Tobit 1:1 all match,
and Tobit 1:1 is correctly flagged canon=deutero.

Structural integrity, all checked by query against the file: PRAGMA
integrity_check returns ok; PRAGMA foreign_key_check returns no rows; every
verse resolves to a book; every verse has a pericope; every verse has
non-empty text; no verse text contains a leftover backslash or a Strong's
number; every verse_id equals book_id*1000000 + chapter*1000 + verse; every one
of the 593,670 TSK edges resolves to real verses at both ends; every Nave
topic-verse row resolves to a real verse.

Determinism was verified against the artifact, not asserted. The pipeline was
run twice into two separate files and the two files compared byte for byte with
cmp. They are identical, 76,197,888 bytes each, sha256
128c3446857fa98c1ffb24fd6c3f69496b2d6c678d94f7ab436abcb356dc24db.

The test suite runs and passes: 28 tests, 28 passed.

## Not verified

The two SWORD binary formats have no specification this project can cite. The
layouts in pipeline/sword.py were derived by inspecting the files. They are
supported by strong evidence rather than by a document: the slot arithmetic for
the Treasury of Scripture Knowledge predicts its index size exactly, 1 + 1 + 39
books + 929 chapters + 23,145 verses giving the 24,115 slots the file actually
contains, and the same arithmetic holds for the New Testament; every block
decompresses to precisely the length its own header declares; and for Nave's,
all 5,322 keys land on an entry whose embedded name matches the key, with five
exceptions noted below. The readers assert these properties and raise rather
than return doubtful data. Even so, this is inference from the files, not
conformance to a published spec.

Five of Nave's 5,322 keys point at an entry whose internal name differs from
the key. They were not investigated individually. They are a rounding error in
a corpus of 18,837 topics, but they are unexplained.

The quality of the cross-references themselves is unassessed. This session
verified that every edge resolves to a real verse; it did not judge whether the
Treasury of Scripture Knowledge's connections are apt, and that is not a P1
question. Nineteen reference strings in TSK are malformed enough that no
reading of them was attempted; they sit in tsk_unresolved with the reason
"unparseable reference".

Nave's chapter-level references are expanded to every verse in the chapter,
which is what such a reference means but does inflate the topic-verse row
count. Whether that expansion helps or hurts retrieval is a P2 question,
answerable against the gold lists and not before.

No retrieval has been attempted. There are no embeddings, no vector store, and
no evidence yet about whether this index answers questions well. P1 built the
index; P2 measures it.

## Flags for Jared

The Code of Conduct sentence. The replacement text you gave, dropped in
verbatim, produced "reported to the community leaders responsible for
enforcement at by opening an issue on the pastor-bible GitHub repository". The
stray "at" belonged to the template's own sentence. I removed that one word so
the sentence reads correctly, changed nothing else, and logged it. Say the word
if you want the literal version back.

The plan is no longer byte-identical to the document you approved. Rewording
9.4 in both README and PLAN was your instruction and it was the right call, but
it means docs/PLAN.md now hashes to
c2431f31134cd192d254ec137801d3d5c650e52aacf059fadc375f75b22cde48 rather than
the f07a7354 that P0 verified three times over. The plan is a living document
from here. That is worth knowing before some later session treats its old hash
as a fixed point.

Editing the Code of Conduct obliged NOTICE.md to change too. CC BY 4.0 requires
that modifications be indicated, so the entry now records the file as modified,
states exactly what changed, and carries both the new checksum and the
upstream one.

The World English Bible trademark condition and what we do to the text. The
licence says that anyone who changes the actual text must not call the result
the World English Bible. We do not change the words. We do strip USFM
formatting markers, and we drop footnotes and cross-references, which are the
translators' apparatus rather than the text. That is what every Bible
application does and I am confident it is within the condition, but the
condition is the one legal obligation attached to our main source, so you
should know precisely what we do rather than hear that it is fine.

Cross-references reach into the deuterocanon. TSK and Nave's were compiled
against a protestant canon, so their references land on protestant verses; but
the canon filter in plan 5.1 still has to gate them at query time. Nothing to
decide now. It is a P2 correctness point and it is written here so it is not
discovered late.

Sirach is missing 24 verses relative to its own numbering, more than the rest
of the corpus put together. Every one is a verse the WEB omits deliberately,
with a footnote. This is normal for Sirach, whose textual tradition is
genuinely uncertain, and nothing is wrong. Flagged because a reader who queries
Sirach and finds gaps deserves to know they are the translation's, not ours.

Nothing about appearance changed this session, so the P0 flags on the
placeholder interface and the stock Tauri icon all still stand.

## Next session

P2 Index and retrieval harness, per plan section 13.

P2 IS BLOCKED until you approve the evaluation gold lists. Plan 6.2 is explicit
that gold lists are judgment, are not auto-generated and are not delegated, and
plan section 13 says P2 does not start until they are approved. The eval set at
data/eval/questions.json does not exist yet: roughly 40 questions, each with the
passages a correct answer should surface. Claude drafts the candidate lists from
the index now that there is an index to draft them from; you review and approve
every one. That drafting can happen at the start of the P2 session, but the
approval has to be yours before any retrieval number means anything.

P2's own scope, once unblocked: choose an embedding model and a reranker from
permissively licensed candidates, build verse, pericope and topic embeddings,
settle the vector store, wire hybrid fusion over FTS5 and vectors, add TSK
expansion, and report recall@25 by configuration.

P2 owns one VERIFY item from plan section 16: sqlite-vec's maturity and whether
it builds on Windows, with a flat binary vector file read from Rust as the
fallback. Two more, the embedding and reranker model candidates and their
licences, are shared with P3.

Read PLAN.md, DECISIONS.md and this file before starting. Do not begin P3.
