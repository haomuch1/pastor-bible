# EVAL

Evaluation protocol for The Pastor Bible. Derived from PLAN.md sections 6.2 and
6.3.

This file is a protocol skeleton. It holds no results and no numbers. Thresholds
are set in P3, against measurement, never estimated. Retrieval-only figures
arrive in P2; full-pipeline figures in P3. Nothing is written here that was not
measured on this project's own data.

## Evaluation set

Location: data/eval/questions.json

Forty questions of the kind a pastor would actually ask, split in two.

Twenty graded questions, g01 to g20, carry gold lists and produce the numbers
that gate a model. Twelve are everyday-life questions and eight are doctrinally
neutral study questions. Two of them, g19 and g20, are chosen because their
subject matter is treated at length in the deuterocanonical books as well as
the protestant ones; they run under both canon modes and exist to measure what
the canon toggle actually changes.

Twenty smoke questions, s01 to s20, carry no gold lists and gate nothing. They
are run in P3 so Jared can read the answers and see whether they look sane
across a wider spread of topics than twenty questions can cover. They exist so
that topic coverage does not come at the price of a review burden nobody can
sustain.

### MUST and SHOULD

Each graded question has two lists.

MUST is five to eight passages a pastor would say the answer cannot omit.
Retrieval recall@25 is measured against MUST only. Every MUST list is approved
by Jared personally. Nothing else gates anything.

SHOULD is a further set of relevant passages. It is Claude's draft, it is
labelled unreviewed, and it does not gate. It exists so that a passage which is
clearly relevant but not indispensable is recorded rather than lost, and so
that Jared has somewhere to promote a passage from.

A gold passage is a verse range inside a single chapter. Never a whole chapter,
never a whole book: a chapter-wide gold entry cannot tell good retrieval apart
from lucky retrieval.

### How the candidate lists were drafted

Drafted in the P2-prep session by pipeline/draft_gold.py, entirely from
index.db. No passage was written down from memory. A passage reaches the draft
only because one of three sources put it there:

  nave  the passage is in a matching Nave's topic. A topic's weight falls as
        the topic grows, because membership of a forty-verse topic says far
        more about a verse than membership of an eighteen-hundred-verse one.
  fts   the passage matched one of the question's keywords in the FTS5 index,
        weighted by its rank in that keyword's results.
  tsk   the passage was reached by one Treasury of Scripture Knowledge hop from
        a high-scoring anchor. Weighted lightly: a cross-reference suggests a
        passage is related, not that it is central.

Adjacent hits are grouped into ranges within a chapter and ranked by score
density, so a long range cannot win merely by covering more verses. A MUST
candidate must carry at least two of the three origins; a single-origin passage
drops to SHOULD rather than being discarded.

The keyword list for each question is stored in the JSON alongside the gold
lists, so that P2 can compare it against the query rewrites the chat model
produces on its own.

This procedure proposes. It does not decide. Plan 6.2 is explicit that gold
lists are judgment, are not auto-generated and are not delegated.

### File schema

    {
      "schema": 1,
      "status": "draft" | "approved",
      "generated_from_index": "<build_checksum of the index.db used>",
      "index_version": "0.1.0",
      "graded": [ { "id": "g01",
                    "question": "...",
                    "category": "life" | "study",
                    "canon": "66" | "both",
                    "keywords": ["...", ...],
                    "nave_topics": ["ANGER (230)", ...] or ["none"],
                    "must":   [ passage, ... ],
                    "should": [ passage, ... ],
                    "status": "draft" | "approved" } ],
      "smoke": [ { "id": "s01",
                   "question": "...",
                   "category": "life" | "study" } ]
    }

    passage = { "ref": "Mat 6:24-34",
                "verse_ids": [6006024, ...],
                "origins": ["fts", "nave", "tsk"],
                "canon": "protestant" | "deutero" }

verse_ids are the index's own primary keys, book_id*1000000 + chapter*1000 +
verse. They are what recall is computed against; ref is for human eyes.

Smoke entries carry id, question and category and nothing else. A gold field on
a smoke entry is a bug, and tests/test_eval_set.py fails on it.

## Metrics

Retrieval recall@25
  Fraction of MUST passages appearing in the top 25 retrieved passages, measured
  per question and averaged. SHOULD passages do not count towards it and do not
  count against it. Measures index and retrieval quality, independent of the
  chat model. Reported per configuration in P2.
  Threshold: set in P3.

Fabricated-reference count
  Total count, across every run, of references in generated prose that do not
  resolve to a passage in the set actually sent to the model. Counted by the same
  mechanical verifier the app ships (plan 5.6), not by reading the output.
  Threshold: 0. Hard gate. Not a target, not an average — any nonzero value fails.

Citation precision
  Fraction of cited passages judged genuinely relevant to the question. Judged,
  so the judging procedure is recorded alongside the number when it is taken.
  Threshold: set in P3.

Answer quality
  Jared rates a fixed subset on a 1 to 5 rubric. The subset is fixed before
  scoring and does not change between models, so scores compare. The rubric is
  written in P3.
  Threshold: quality floor set in P3.

Latency and peak RAM
  Measured on CPU, per model size, on a machine whose specification is recorded
  with the numbers. Feeds the hardware floor stated in README (plan 6.5) and the
  installer's automatic model choice (plan 6.4).
  Threshold: no pass/fail gate; these numbers define the stated requirements.

## Gates

A model configuration passes only if all of these hold at once:

  1. Recall gate      — recall@25 at or above the threshold set in P3.
  2. Fabrication gate — fabricated-reference count exactly 0.
  3. Quality gate     — answer quality at or above the floor set in P3.

## Selection rule

From plan 6.4. The smallest model size passing all three gates becomes the
fallback model. One size up from it becomes the default. Both ship; the installer
picks between them from detected RAM, and the user can override in settings.

If no size passes, nothing is selected and the gates are not relaxed to
manufacture a pass. That outcome is reported to Jared as a finding.

## Index build 0.1.0

Built 2026-08-26 in P1. Every number below was read back out of the finished
index.db by query, in a process separate from the build.

Source text: World English Bible Classic, eBible.org eng-web, USFM.

    books                       81      66 protestant, 15 deuterocanonical
    chapters                  1402      1189 protestant, 213 deuterocanonical
    verses                   38029      31098 protestant, 6931 deuterocanonical
      protestant OT          23145
      protestant NT           7953
    verse bridges                1      4 Maccabees 8:28-29
    omitted verse markers       29      verses the WEB omits, listed in meta
    pericopes                10052      10 from headings, 10042 from paragraphs
    verses with no pericope      0

The protestant total is 4 fewer than the King James count of 31,102. The
difference is entirely accounted for and no data was adjusted to close it:
Luke 17:36 and Acts 8:37, 15:34 and 24:7 are omitted by the WEB, each with a
footnote saying so. A fifth omission, Romans 16:25, does not change the total
because the WEB places the doxology at Romans 14:24-26, which the King James
numbers 16:25-27.

Cross-references, Treasury of Scripture Knowledge:

    edges                   593670
    distinct source verses   26202
    distinct target verses   31045
    unresolved                1380      quarantined in tsk_unresolved
      marginal note markers   1175      TSK's own "*marg:" notes, not references
      no matching verse        179      references to verses absent from the WEB
      unparseable               19      malformed reference strings
      source verse absent        7

Topics, Nave's Topical Bible:

    topics                   18837      5322 top level, 13515 subtopics
    topic-verse rows        399374
    distinct verses          30982
    topics carrying verses   18188
    unresolved                  31      quarantined in nave_unresolved

Keyword index: FTS5 over verse text, 38029 rows, equal to the verse count.

Determinism: the pipeline was run twice into separate files and the outputs
compared byte for byte. They were identical.

## Results

None yet. Populated in P2 (retrieval) and P3 (full pipeline).
