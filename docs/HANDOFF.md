# HANDOFF

Session: P2-prep, evaluation set and candidate gold lists
Date: 2026-08-26
Status: COMPLETE. P2 itself is BLOCKED, awaiting Jared's approval.

## State

The repository is at D:\Haomuch-Programs\The-Pastor-Bible, branch main, pushed
to https://github.com/haomuch1/pastor-bible, still private.

Nothing was built this session but documents and a draft. No embeddings, no
vector store, no retrieval run, no model was loaded. index.db is untouched.

New:

  data/eval/questions.json        the evaluation set, status "draft"
  docs/EVAL-GOLD-REVIEW.md        the review file Jared marks up
  pipeline/draft_gold.py          the script that drafted the candidates
  tests/test_eval_set.py          18 tests over the evaluation set

Changed: NOTICE.md now states outright that WEB verse text is reproduced
unmodified and that translator footnotes and cross-references are omitted.
docs/EVAL.md gained the file schema, the drafting procedure, the MUST and
SHOULD rule and the graded/smoke split, and its recall@25 definition now says
explicitly that recall is measured against MUST only. DECISIONS.md gained
eleven entries.

The three carryover fixes from P1 remain as they were. Nothing about the
application changed.

## Verified

index.db is the one P1 produced. Its sha256 is
128c3446857fa98c1ffb24fd6c3f69496b2d6c678d94f7ab436abcb356dc24db, which matches
the value recorded in the P1 handoff exactly. No rebuild was needed.

The evaluation set holds 20 graded questions and 20 smoke questions. Question
wording is exactly as the session brief specified; it was transcribed once and
not edited afterwards.

Every one of the 400 proposed passages, 160 MUST and 240 SHOULD, resolves to
real verse rows in index.db. This is asserted twice: once inside draft_gold.py,
which raises rather than writing a passage it cannot resolve, and again in
tests/test_eval_set.py, which re-checks every verse_id against the database
after the file is written.

Every passage lies inside a single chapter, none is a whole chapter, verse ids
are contiguous and sorted, and each passage's canon label matches the canon of
the book it sits in. The 18 questions marked canon "66" carry no
deuterocanonical passage at all.

Every MUST list has exactly 8 entries, within the 5 to 8 rule. Every MUST
passage carries at least two of the three origins.

Every graded question found at least one real Nave's topic. None rests on FTS
and TSK alone. Where Nave's routes a heading to another topic the pointer was
followed once and recorded, so ANXIETY shows as "ANXIETY -> CARE" and TRUST as
"TRUST -> FAITH".

Smoke entries carry id, question and category and nothing else, asserted.

The full test suite runs and passes: 46 tests, 46 passed. That is P1's 28 plus
this session's 18.

## Not verified

Nothing in this session is approved, and nothing in it has been judged correct
by anyone qualified to judge it. The gold lists are proposals. Plan 6.2 puts
that judgment with Jared and this session does not touch it.

Whether the drafting method finds the passages a pastor would name is unproven.
Two rounds of tuning were done and their effect was inspected by eye: weighting
Nave's topics down as they grow, ranking by score density, requiring two
origins for a MUST entry, and sharpening the keyword lists. The lists improved
markedly under that, but "improved by eye" is not a measurement, and no
measurement of the drafting method is possible before there are approved lists
to measure it against.

The keyword lists are Claude's choice. They are recorded in the JSON precisely
so that P2 can compare them against the query rewrites the chat model produces
and see whether the two agree; that comparison has not happened.

## Flags for Jared

Two lists need your attention more than the rest: g04 on grief, and g08 on
fear. Both are dominated by narrative examples rather than by passages that
speak to the person asking. g04 offers David mourning Absalom, Jacob mourning
Joseph and Joab rebuking David; g08 offers Gideon, Ezekiel and Joshua being
told not to be afraid. These are not errors: they are what the sources contain.
Nave's indexes grief and fear largely by instance, and the WEB's vocabulary of
fear is mostly narrative speech. The passages a grieving or frightened person
is usually given, and I am naming these as the kind of thing that is absent
rather than proposing them, live in places the keyword and topic path does not
reach well. Expect to do more striking and adding on these two than on any
others.

This is also a finding rather than only a problem. It predicts that keyword and
topic retrieval alone will do worst on exactly the emotional-support questions
the app most needs to handle well, which is the case embeddings exist to fix.
P2 will be able to measure that instead of assuming it.

The deuterocanon result is worth reading before you approve g19 and g20. The
candidates are there: 40 deuterocanonical ranges surfaced for g19 and 27 for
g20, and the obvious ones are among them. Tobit 12:8-9, "Good is prayer with
fasting, alms, and righteousness", ranks 37th of 540. Tobit 4:7-11, "Give alms
from your possessions", ranks 60th. Sirach 19:20, "All wisdom is the fear of
the Lord", ranks 24th of 462. Not one reached MUST or SHOULD. The reason is
structural and was predicted: neither Nave's nor TSK indexes those books, so a
deuterocanonical passage can only ever carry one origin and can never clear the
two-origin bar, nor outrank protestant passages carrying three. If you want the
canon toggle to mean anything on these two questions you will need to add those
lines by hand, and that is a deliberate decision for you rather than something
the draft should have done quietly.

One methodological point that will matter in P2. Turning the deuterocanon on
changes the protestant results too, not only adds to them. Deuterocanonical
verses occupy slots in each keyword's results, which displaces protestant
verses from the window and shifts their ranks. For g20 the MUST list is not the
same under the two canon modes even though no deuterocanonical passage is in
it. P2 must not assume the 66-book result is a subset of the both-canon result.

Every MUST list currently holds 8 entries, the top of the 5 to 8 range. That is
the draft being generous so you have material to cut from. Cutting to five is
expected and is not a loss.

The review file gives no deuterocanonical candidates to choose from, because
none ranked. The six best for each of g19 and g20 are listed above in this file
so you have them in front of you if you want to add any.

## Next session

P2 is BLOCKED until Jared writes APPROVED under every MUST list in
docs/EVAL-GOLD-REVIEW.md.

P2's first step copies the approved lists into questions.json and sets status
approved.

Only then does P2's own work begin: shortlist two or three embedding models
under permissive licences, build verse, pericope and topic embeddings, settle
the vector store, wire hybrid fusion over FTS5 and vectors with reciprocal rank
fusion, add TSK expansion, add a reranker, and report recall@25 by
configuration against the approved MUST lists.

P2 owns one VERIFY item from plan section 16: whether sqlite-vec is mature
enough and builds on Windows, with a flat binary vector file read from Rust as
the fallback. The embedding and reranker candidates and their licences are
shared with P3.

P2 also owes two reports that this session's findings ask for: the paragraph
length distribution across pericopes, since pericopes are WEB paragraphs and no
headings are being added, and the recall difference between the two canon modes
on g19 and g20.

Read PLAN.md, DECISIONS.md and this file before starting. Do not begin P3.
