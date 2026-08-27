# VERIFIER

The mechanical citation guarantee, specified. PLAN 5.6 states the promise: the
user never sees an unverified reference. This document is how that promise is
kept, precisely enough that the Rust implementation in P4 and the Python
implementation in pipeline/verifier.py can be checked against each other.

The test vectors at the end are part of the contract. Both implementations run
the same ones.

## What the model is given

Retrieval produces a set of passages. Each is sent to the model as an opaque
token and its text:

    [P1] Matthew 6:25-34
    Therefore I tell you, don't be anxious for your life ...

    [P2] Philippians 4:6-7
    In nothing be anxious, but in everything, by prayer and petition ...

The token is `[P` followed by a positive integer and `]`. Tokens are numbered
from 1 and are unique within a request. A passage carries the verses it covers;
the union of those verses is the **sent set**.

The prompt permits citing only by these tokens and forbids naming any reference
that is not in the set. The verifier does not trust that instruction. It
assumes the model will sometimes disobey, because it will.

## What the verifier checks

**Rule A — every token exists.** Every `[P#]` occurring anywhere in the output
must be one of the tokens sent. `[P9]` in a response built from eight passages
is a violation.

**Rule B — every free-text reference resolves inside the sent set.** Any
scripture reference written out in prose is a violation unless every verse it
names is in the sent set. A reference is detected when a book name is followed
by a chapter number. The forms recognised are:

    Genesis 1:1              full name, chapter and verse
    Gen 1:1                  abbreviation
    Gen. 1:1                 abbreviation with a full stop
    Genesis 1:1-3            verse range
    Genesis 1:1–3            en dash
    Genesis 1                chapter only
    Ps 23                    abbreviation, chapter only
    1 Corinthians 13:4-7     numeric book prefix
    1Cor 13:4                no space after the prefix
    1 Cor. 13:4-7            prefix, abbreviation, full stop
    First Corinthians 13     ordinal word prefix
    I Corinthians 13         roman numeral prefix
    Song of Solomon 2:1      multi-word book name
    Wisdom of Solomon 3:1    multi-word deuterocanonical name
    1 Maccabees 2:15         numeric prefix, deuterocanonical

The written forms are taken from index.db itself: each book's USFM code, its
abbreviation, its full title, that title without a leading "The", and the
Treasury of Scripture Knowledge abbreviation table. A short alias list supplies
the common English names that none of those spells out, which are "Song of
Songs", "Canticles", "Acts of the Apostles", "Wisdom of Solomon", "Wisdom",
"Ecclesiasticus", "Sirach", "Ben Sira", "Prayer of Manasseh", "Greek Esther",
"Greek Daniel", and "1" through "4 Maccabees" and "1" and "2 Esdras".

*Corrected 2026-08-26, P4.* Until this date the implementation built its
pattern from the normalised form of each name, which has spaces stripped, so no
multi-word written form could ever match. Measured against 83 realistic book
names, 14 were undetected: "Song of Solomon", "Song of Songs", "Acts of the
Apostles" and eleven deuterocanonical names, which are exactly what a
both-canon answer cites. Both implementations now build the pattern word by
word from the written form. The correction changes no verdict on any output P3
produced: 41 first-pass verdicts, 14 violation records with identical kind,
text, reason and span, and 41 final answers, all unchanged.

A chapter-only reference such as `Ps 23` claims the whole chapter. It resolves
only if every verse of that chapter is in the sent set, which is rarely true of
a partial retrieval. That strictness is deliberate: the model was told to cite
by token, so a free-text reference is already outside its instructions, and a
claim about a whole chapter is not supported by a handful of its verses.

**Deliberate non-detections.** These are not references and must not be flagged.
A false positive costs a needless retry and mangles prose; the list below is
what the test vectors defend.

- A number with no book name: "the third day", "seven times", "forty years".
- The words "chapter" and "verse" as ordinary language: "chapter and verse".
- A book name used as an ordinary word, with no chapter number after it: Job as
  a person, Mark as a name, Acts, Numbers, Judges, Kings, Song, Revelation,
  Chronicles, Romans, Hebrews as a people.
- A book name that is also a common English word, in lower case, followed by a
  number: "he acts 2 ways", "it numbers 3 among them". The ambiguous names are
  matched case-sensitively, so only a capitalised `Acts 2` is a candidate.
- A capitalised ambiguous name followed by a number that is plainly a count:
  "Kings 3 and 4 of the dynasty" is still detected, but "Judges 12 times" is
  not, because a unit noun follows the number.

The unit nouns that suppress detection after a number are: times, time, days,
day, years, year, months, month, weeks, week, hours, hour, people, men, women,
sons, daughters, tribes, thousand, hundred, million, percent, degrees.

**Asymmetry of errors.** Missing a fabricated reference puts a false citation in
front of a reader and breaks the project's central promise. Flagging a
non-reference costs one retry. The rules above therefore lean towards detection
wherever the two conflict, and the non-detection list exists to stop that lean
from damaging ordinary prose.

## What happens on a violation

1. **First failure.** Every offending token and reference is stripped from the
   output. Generation is retried once, with the specific failure named in the
   prompt: which token or reference was rejected and why.
2. **Second failure.** No third attempt. The app shows the retrieved passages
   grouped by book, with the one-line note from PLAN 5.6 that a synthesis could
   not be produced. This is the fallback, and it is a normal outcome, not an
   error state.
3. The passage panel always renders verse text from index.db, never from model
   output, at every stage including the fallback.

A stripped output is never shown to the user as-is. Stripping exists so the
retry prompt can quote what was wrong, and so a partially-good response can be
measured; the user sees either a clean generation or the fallback.

## Verdicts

    ok           no violations
    violation    at least one rule A or rule B failure; carries the details
    fallback     a violation on the retry as well

Each violation records its kind (`token` or `reference`), the exact text, and
the reason (`token not in sent set`, `book not in this text`, `verses not in
sent set`).

## Test vectors

35 vectors, in pipeline/verifier.py as `TEST_VECTORS`, run by
tests/test_verifier.py and by the Rust port's own test. The sent set for all of
them is:

    [P1] Matthew 6:25-34      [P2] Philippians 4:6-7
    [P3] Psalm 23:1-4         [P4] 1 Peter 5:6-7

Vectors 1 to 8 must pass. Vectors 9 to 17 must be flagged. Vectors 18 to 25 are
the false positives that must not be flagged. Vectors 26 to 31 are the
multi-word and deuterocanonical names the 2026-08-26 correction added, and 32
to 35 are the same names in ordinary prose with no chapter number, which must
still not be flagged. The file is the authority on their exact text; this list
is the summary.

    1   cites [P1] only                                        ok
    2   cites [P1] and [P2]                                    ok
    3   full-name reference wholly inside the set              ok
    4   abbreviated reference inside the set                   ok
    5   verse range inside the set                             ok
    6   en-dash range inside the set                           ok
    7   prose with no citation at all                          ok
    8   token adjacent to punctuation                          ok
    9   [P9], a token never sent                               violation
    10  [P0], not a valid token number                         violation
    11  reference to a book not in the sent set                violation
    12  reference to the right book, wrong chapter             violation
    13  range extending past the sent verses                   violation
    14  chapter-only reference, chapter only partly sent       violation
    15  numeric-prefix book not in the set                     violation
    16  ordinal-word book not in the set                       violation
    17  roman-numeral book not in the set                      violation
    18  "the third day"                                        ok
    19  "seven times"                                          ok
    20  "chapter and verse" as idiom                           ok
    21  Job as a person, no chapter                            ok
    22  Mark as a name, no chapter                             ok
    23  "he acts 2 ways", lower case                           ok
    24  "Judges 12 times", unit noun follows                   ok
    25  Numbers, Kings, Acts as words in a sentence            ok
    26  Song of Solomon 2:1                                    violation
    27  Song of Songs 2:1                                      violation
    28  The Acts of the Apostles 2:38                          violation
    29  Wisdom of Solomon 3:1                                  violation
    30  1 Maccabees 2:15                                       violation
    31  Sirach 3:1                                             violation
    32  "the song of songs" sung, no chapter                   ok
    33  "the acts of the apostles" as prose, no chapter        ok
    34  Wisdom personified, no chapter                         ok
    35  "the prayer of Manasseh" read aloud, no chapter        ok
