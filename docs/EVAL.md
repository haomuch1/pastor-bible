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

## Embedding and reranker shortlist (P2)

Every model here meets all of the conditions set for P2: an Apache-2.0 or MIT
licence, English, small enough to run on a CPU, available as GGUF, and
supported by llama-server's embedding or rerank endpoint. The licence quoted is
the `license` field of the model's own card on Hugging Face, read from the API
rather than from a README.

Index-time vectors and query-time vectors are produced by the same binary
through the same endpoint, so there is no train/serve mismatch to chase later.

    tool          llama.cpp, release b10639, win-cpu-x64 prebuilt binary
                  reported by the binary as: version 0.3.0-dev (build 10639,
                  commit 5e6a37cb1), built with Clang 20.1.8
                  kept in tools/, gitignored, never committed

Embedding candidates:

    bge-small-en-v1.5
      licence     mit          (BAAI/bge-small-en-v1.5)
      params      33.4M        dim 384      model context 512
      GGUF        CompendiumLabs/bge-small-en-v1.5-gguf, bge-small-en-v1.5-f16.gguf
      sha256      f0b2fef971e8366438bfd2d9aefea1b0115919389448806d290237f638bae999
      size        64.2 MB
      prefixes    documents: none
                  queries:   "Represent this sentence for searching relevant passages: "

    nomic-embed-text-v1.5
      licence     apache-2.0   (nomic-ai/nomic-embed-text-v1.5)
      params      136.7M       dim 768      model context 2048
      GGUF        nomic-ai/nomic-embed-text-v1.5-GGUF, nomic-embed-text-v1.5.f16.gguf
      sha256      f7af6f66802f4df86eda10fe9bbcfc75c39562bed48ef6ace719a251cf1c2fdb
      size        261.6 MB
      prefixes    documents: "search_document: "
                  queries:   "search_query: "

    qwen3-embedding-0.6b
      licence     apache-2.0   (Qwen/Qwen3-Embedding-0.6B)
      params      595.8M       dim 1024     model context 32768, used at 2048
      GGUF        Qwen/Qwen3-Embedding-0.6B-GGUF, Qwen3-Embedding-0.6B-Q8_0.gguf
      sha256      06507c7b42688469c4e7298b0a1e16deff06caf291cf0a5b278c308249c3e439
      size        609.5 MB
      prefixes    documents: none
                  queries:   "Instruct: Given a question about the Bible, retrieve
                              passages of scripture that answer it
Query: "

Reranker candidate:

    bge-reranker-v2-m3
      licence     apache-2.0   (BAAI/bge-reranker-v2-m3, and the GGUF repo
                  gpustack/bge-reranker-v2-m3-GGUF is apache-2.0 as well)
      params      567.8M       model context 8194, used at 2048
      GGUF        gpustack/bge-reranker-v2-m3-GGUF, bge-reranker-v2-m3-Q8_0.gguf
      sha256      a43c7c9b11a4c1517e5bf95151960e1621d1b72f7a493364b01e386cf1aaa1d3
      size        606.2 MB

Only one reranker made the list. The obvious smaller alternative,
bge-reranker-base, is MIT but its only GGUF builds are third-party repositories
with a few dozen downloads, which is not a checksum this project should stake a
release on. Qwen3-Reranker-0.6B is Apache-2.0 but has no first-party GGUF, and
its yes/no-logit scoring is a different mechanism from the classification head
that llama-server's rerank endpoint expects. Rerank is not gating, so one
candidate is enough to answer whether it earns its place.

Prefixes matter. Each of these models states a convention on its card, and
nomic's is mandatory rather than advisory. Using the wrong prefix, or none,
costs real recall, so the exact strings are stored in the index alongside the
vectors, in the embedding_models table, and the harness reads them from there
rather than hardcoding them.

## Embedding text templates

Recorded because changing one of these invalidates every vector in the database.

    verse     "{abbrev} {chapter}:{verse}[ — {heading}]
{verse text}"
    pericope  "{abbrev} {chapter}:{first}-{last}[ — {heading}]
{verse texts}"
    topic     "{heading}", or "{parent heading} — {heading}" for a subtopic

The heading is appended only where the source actually has one, which is 10
pericopes out of 10,052. The document prefix for the model in use is prepended
to all three.

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

## Retrieval evaluation method (P2)

Run by pipeline/evaluate.py against the built index.db, over the 20 approved
graded questions.

What recall@k means here. Candidates come back as ranked verses, are grouped
into contiguous ranges within a chapter, and the best k ranges are the
retrieved passages. That matches PLAN 5.5, which sends "top ~25 passages" to
generation, rather than counting 25 individual verses. A MUST passage counts as
recalled if it shares at least one verse with a retrieved passage. Recall is
the fraction of MUST passages recalled, averaged across questions.

The query. No chat model runs in P2, so PLAN 5.2's query rewriting does not
happen. The stored keyword list for each question stands in for it: the vector
paths receive the plain question text followed by its keywords, and the FTS
path receives the keywords as separate ranked searches. P3 replaces this with
the model's own rewrites and can compare them against the stored lists.

Configurations:

    A   vector only, verses
    B   vector only, pericopes
    C   vector only, verses + pericopes fused
    D   FTS only
    E   C + D fused, hybrid with no expansion
    F   E + Nave's topic expansion + TSK one-hop expansion
    G   F + reranker

A, B and C are the evidence. D through G all use the topic, cross-reference or
keyword paths that the gold lists were themselves drawn from, so measuring them
against those lists is close to marking their own homework: a passage is in the
gold list partly because Nave's or TSK or a keyword search proposed it, and
those same paths then retrieve it. Their numbers are reported for completeness
and to see the shape of the pipeline, and they are not evidence of quality. The
vector paths had no hand in drafting the gold lists, so what they recall is the
one thing these questions can honestly measure.

Fusion is reciprocal rank fusion with k=60 over each contributing ranked list.
Topic expansion contributes the verse list of each of the top 5 matching Nave's
topics, capped at 60 verses per topic. TSK expansion follows cross-references
one hop from the 25 highest-scoring candidates, capped at 200 verses. Both are
tagged by origin so a passage's provenance is visible in the output.

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

## Retrieval results 0.2.0 (P2)

Measured 2026-08-26 against index.db 0.2.0, on the 20 approved graded
questions. Every figure was read back out of the built database by the harness;
none is carried over from a build log.

Read the A, B and C rows first. They are the only ones that answer an open
question. Everything below them shares sources with the gold lists and is
reported so the shape of the pipeline is visible, not as evidence.

### Recall@25 against MUST, by configuration and embedding model

    cfg    what it is                             bge-small-en-v1.5       nomic-embed-text-v1.5   qwen3-embedding-0.6b  
    A      vector only, verses                    0.279                   0.331                   0.442                 
    B      vector only, pericopes                 0.331                   0.387                   0.377                 
    C      vector only, verses + pericopes        0.346                   0.428                   0.462                 
  ~ D      FTS only                               0.559                   0.559                   0.559                 
  ~ E      C + D fused (hybrid, no expansion)     0.548                   0.604                   0.566                 
  ~ F      E + topic expansion + TSK expansion    0.588                   0.629                   0.571                 
  ~ G      F + reranker                           0.531                   0.529                   0.534                 

  Rows marked ~ use the topic, cross-reference or keyword paths that
  the gold lists were themselves drawn from. They are reported for
  completeness and are near-circular; A, B and C are the evidence.

### Where the curve bends: recall@10, @25, @50

    model                  cfg       @10      @25      @50
    bge-small-en-v1.5      C       0.216    0.346    0.415
    bge-small-en-v1.5      G       0.326    0.531    0.699
    nomic-embed-text-v1.5  C       0.247    0.428    0.560
    nomic-embed-text-v1.5  G       0.295    0.529    0.718
    qwen3-embedding-0.6b   C       0.294    0.462    0.576
    qwen3-embedding-0.6b   G       0.291    0.534    0.708

### Per-question recall@25, configuration C and G

    q     bge           nomic         qwen3           G
    g01   0.500         0.500         0.750           0.750
    g02   0.125         0.375         0.250           0.500
    g03   0.250         0.500         0.625           0.500
    g04   0.125         0.125         0.125           0.125
    g05   0.500         0.375         0.625           1.000
    g06   0.625         0.625         0.625           0.625
    g07   0.500         0.250         0.625           0.625
    g08   0.250         0.125         0.375           0.125
    g09   0.250         1.000         0.875           1.000
    g10   0.375         0.375         0.375           0.625
    g11   0.125         0.125         0.250           0.250
    g12   0.000         0.000         0.250           0.375
    g13   0.875         1.000         1.000           0.750
    g14   0.000         0.250         0.250           0.125
    g15   0.625         0.500         0.500           0.625
    g16   0.000         0.375         0.000           0.250
    g17   0.375         0.500         0.500           0.625
    g18   0.625         0.625         0.375           0.750
    g19   0.214         0.286         0.286           0.357
    g20   0.571         0.643         0.571           0.643

### Full retrieved set under configuration F

                      min   median      max
    passages          241      368      547
    verses            636      828      974
    tokens          16722    23003    27873

### Deuterocanon, g19 and g20 under both-canon mode

    bge-small-en-v1.5      A g19  in top 25: 3 of 6   Tob 12:8-9@1, Tob 4:7-11@6, Bar 3:2@11
    bge-small-en-v1.5      A g20  in top 25: 2 of 6   Sir 19:20-22@11, Wis 6:20@20
    bge-small-en-v1.5      B g19  in top 25: 2 of 6   Tob 4:7-11@10, Tob 12:8-9@12
    bge-small-en-v1.5      B g20  in top 25: 4 of 6   Wis 6:20@4, Bar 3:12@9, Sir 1:4@10, Sir 19:20-22@22
    bge-small-en-v1.5      C g19  in top 25: 3 of 6   Tob 12:8-9@4, Tob 4:7-11@5, Bar 3:2@13
    bge-small-en-v1.5      C g20  in top 25: 3 of 6   Wis 6:20@5, Sir 19:20-22@10, Bar 3:12@22
    bge-small-en-v1.5      D g19  in top 25: 3 of 6   Sir 31:4@6, 2Es 2:20@14, Tob 12:8-9@25
    bge-small-en-v1.5      D g20  in top 25: 2 of 6   Sir 1:4@4, Sir 19:20-22@16
    bge-small-en-v1.5      E g19  in top 25: 5 of 6   Tob 12:8-9@3, Tob 4:7-11@5, Bar 3:2@10, Sir 31:4@17, 2Es 2:20@23
    bge-small-en-v1.5      E g20  in top 25: 4 of 6   Wis 6:20@7, Sir 1:4@8, Sir 19:20-22@9, Bar 3:12@25
    bge-small-en-v1.5      F g19  in top 25: 3 of 6   Tob 12:8-9@8, Tob 4:7-11@10, Bar 3:2@23
    bge-small-en-v1.5      F g20  in top 25: 3 of 6   Wis 6:20@11, Sir 1:4@12, Sir 19:20-22@14
    bge-small-en-v1.5      G g19  in top 25: 4 of 6   Tob 4:7-11@1, 2Es 2:20@4, Tob 12:8-9@7, Sir 31:4@24
    bge-small-en-v1.5      G g20  in top 25: 3 of 6   Sir 1:4@1, Wis 6:20@3, Bar 3:12@6
    nomic-embed-text-v1.5  A g19  in top 25: 3 of 6   Tob 4:7-11@1, Tob 12:8-9@11, Tob 14:10-11@23
    nomic-embed-text-v1.5  A g20  in top 25: 2 of 6   Sir 19:20-22@16, Sir 1:4@17
    nomic-embed-text-v1.5  B g19  in top 25: 2 of 6   Tob 4:7-11@2, Tob 12:8-9@4
    nomic-embed-text-v1.5  B g20  in top 25: 4 of 6   Sir 1:4@3, Sir 19:20-22@6, Wis 6:20@12, Bar 3:12@15
    nomic-embed-text-v1.5  C g19  in top 25: 3 of 6   Tob 4:7-11@2, Tob 12:8-9@4, Bar 3:2@25
    nomic-embed-text-v1.5  C g20  in top 25: 3 of 6   Sir 1:4@4, Sir 19:20-22@5, Wis 6:20@22
    nomic-embed-text-v1.5  D g19  in top 25: 3 of 6   Sir 31:4@6, 2Es 2:20@14, Tob 12:8-9@25
    nomic-embed-text-v1.5  D g20  in top 25: 2 of 6   Sir 1:4@4, Sir 19:20-22@16
    nomic-embed-text-v1.5  E g19  in top 25: 5 of 6   Tob 4:7-11@2, Tob 12:8-9@4, Bar 3:2@17, Tob 14:10-11@18, Sir 31:4@23
    nomic-embed-text-v1.5  E g20  in top 25: 3 of 6   Sir 1:4@1, Sir 19:20-22@4, Wis 6:20@23
    nomic-embed-text-v1.5  F g19  in top 25: 5 of 6   Tob 4:7-11@3, Tob 12:8-9@5, Bar 3:2@19, Tob 14:10-11@21, Sir 31:4@25
    nomic-embed-text-v1.5  F g20  in top 25: 2 of 6   Sir 1:4@3, Sir 19:20-22@7
    nomic-embed-text-v1.5  G g19  in top 25: 3 of 6   Tob 4:7-11@2, Tob 14:10-11@4, Tob 12:8-9@7
    nomic-embed-text-v1.5  G g20  in top 25: 4 of 6   Sir 1:4@1, Wis 6:20@3, Bar 3:12@6, Sir 19:20-22@25
    qwen3-embedding-0.6b   A g19  in top 25: 2 of 6   Tob 4:7-11@3, Bar 3:2@7
    qwen3-embedding-0.6b   A g20  in top 25: 2 of 6   Sir 19:20-22@3, Sir 1:4@18
    qwen3-embedding-0.6b   B g19  in top 25: 2 of 6   Tob 12:8-9@9, Tob 4:7-11@13
    qwen3-embedding-0.6b   B g20  in top 25: 2 of 6   Sir 19:20-22@2, Sir 1:4@8
    qwen3-embedding-0.6b   C g19  in top 25: 3 of 6   Tob 4:7-11@4, Tob 12:8-9@12, Bar 3:2@19
    qwen3-embedding-0.6b   C g20  in top 25: 2 of 6   Sir 19:20-22@2, Sir 1:4@8
    qwen3-embedding-0.6b   D g19  in top 25: 3 of 6   Sir 31:4@6, 2Es 2:20@14, Tob 12:8-9@25
    qwen3-embedding-0.6b   D g20  in top 25: 2 of 6   Sir 1:4@4, Sir 19:20-22@16
    qwen3-embedding-0.6b   E g19  in top 25: 4 of 6   Tob 4:7-11@10, Tob 12:8-9@16, Bar 3:2@21, 2Es 2:20@22
    qwen3-embedding-0.6b   E g20  in top 25: 2 of 6   Sir 1:4@2, Sir 19:20-22@3
    qwen3-embedding-0.6b   F g19  in top 25: 2 of 6   Tob 4:7-11@17, Tob 12:8-9@25
    qwen3-embedding-0.6b   F g20  in top 25: 2 of 6   Sir 1:4@6, Sir 19:20-22@8
    qwen3-embedding-0.6b   G g19  in top 25: 4 of 6   Tob 4:7-11@2, Tob 14:10-11@3, Tob 12:8-9@6, 2Es 2:20@24
    qwen3-embedding-0.6b   G g20  in top 25: 3 of 6   Sir 1:4@2, Wis 6:20@3, Bar 3:12@6

### Latency, mean seconds per query

    bge-small-en-v1.5|A            0.012
    bge-small-en-v1.5|B            0.008
    bge-small-en-v1.5|C            0.006
    bge-small-en-v1.5|D            0.003
    bge-small-en-v1.5|E            0.010
    bge-small-en-v1.5|F            0.017
    bge-small-en-v1.5|G            11.200
    nomic-embed-text-v1.5|A        0.015
    nomic-embed-text-v1.5|B        0.009
    nomic-embed-text-v1.5|C        0.008
    nomic-embed-text-v1.5|D        0.003
    nomic-embed-text-v1.5|E        0.011
    nomic-embed-text-v1.5|F        0.022
    nomic-embed-text-v1.5|G        10.283
    qwen3-embedding-0.6b|A         0.018
    qwen3-embedding-0.6b|B         0.012
    qwen3-embedding-0.6b|C         0.010
    qwen3-embedding-0.6b|D         0.003
    qwen3-embedding-0.6b|E         0.013
    qwen3-embedding-0.6b|F         0.024
    qwen3-embedding-0.6b|G         11.095

### Peak resident memory, MB

    embed:bge-small-en-v1.5        74.9
    embed:nomic-embed-text-v1.5    251.4
    embed:qwen3-embedding-0.6b     945.9
    rerank                         2009.3

### What the numbers say

Vector search is the weakest single path against these gold lists, and that is
exactly what should have been expected: the lists were drafted from Nave's, the
cross-references and the keyword index, so a keyword-only run scores 0.559
without the embeddings contributing anything at all. The vector paths had no
hand in the drafting. That they reach 0.35 to 0.46 unaided, against lists built
by other means, is the real result.

Pericope vectors beat verse vectors for the two smaller models and lose for the
largest. Fusing both beats either alone in every case, which is the one clean,
model-independent finding here.

The differences between the three embedding models are not statistically
separable on 20 questions. A paired bootstrap over questions, 20,000 resamples,
puts qwen3 minus nomic on configuration C at +0.034 with a 95 per cent interval
of [-0.045, +0.112], and nomic minus bge-small at +0.082 with [-0.005, +0.182].
Both intervals include zero. The point estimates order the models consistently,
and nomic leads bge-small on every configuration measured, but this eval set
cannot prove a difference of that size.

The reranker makes things worse where it matters. It lowers recall@25 for all
three models, from 0.588 to 0.531 for bge-small and from 0.629 to 0.529 for
nomic, while raising recall@50. It is reordering the head of the list badly,
pushing relevant passages out of the top 25 and into the 25-to-50 band. It also
costs about 10 seconds per query and 2 GB of resident memory, against 3 to 24
milliseconds for retrieval itself.

Retrieval latency confirms the vector-store decision. Brute-force cosine over
38,029 verse vectors, about 10,000 pericope vectors and 18,837 topic vectors
runs in 8 to 24 milliseconds per query in Python. sqlite-vec would have bought
nothing and cost a bundled native extension.

### The Deuterocanon, and what embeddings actually add

This is the clearest result of the session, and it reverses what P2-prep found.

In P2-prep, drafting candidates from Nave's, TSK and keyword search, not one
deuterocanonical passage reached a gold list for g19 or g20. The best of them,
Tobit 12:8-9, ranked 37th of 540. The reason was structural: neither study
corpus indexes those books, so those passages could never gather the
corroboration the drafting method required.

With embeddings, vector search finds them. Under configuration A, verse vectors
alone, bge-small returns Tobit 12:8-9 at rank 1 for g19. Across the models and
configurations, 2 to 5 of the 6 deuterocanonical MUST passages for each of g19
and g20 land in the top 25. The passages were always in the text. Nothing but a
semantic index could reach them.

### Canon mode is not a filter over a fixed ranking

Confirmed, and larger than P2-prep suspected. Turning the Deuterocanon on does
not add passages to a stable protestant list. It displaces them. Of the 25
protestant passages in the top 25 under canon 66, between 5 and 15 are gone
once the Deuterocanon is enabled, for every model and both configurations
tested.

    model                  cfg  q    protestant in top 25, 66 -> both   dropped
    bge-small-en-v1.5      C    g19  25 -> 14                           12
    bge-small-en-v1.5      F    g19  25 -> 19                           10
    bge-small-en-v1.5      C    g20  25 -> 20                            5
    bge-small-en-v1.5      F    g20  25 -> 22                            5
    nomic-embed-text-v1.5  C    g19  25 -> 11                           15
    nomic-embed-text-v1.5  F    g19  25 -> 15                           12
    nomic-embed-text-v1.5  C    g20  25 -> 19                            7
    nomic-embed-text-v1.5  F    g20  25 -> 21                            9
    qwen3-embedding-0.6b   C    g19  25 -> 18                            7
    qwen3-embedding-0.6b   F    g19  25 -> 21                            5
    qwen3-embedding-0.6b   C    g20  25 -> 16                           11
    qwen3-embedding-0.6b   F    g20  25 -> 20                            8

No code may assume the 66-book result is a subset of the both-canon result. A
reader who turns the setting on loses protestant passages they would otherwise
have seen. Whether that is acceptable is a product question for P5, not a bug.

### Weak questions

Per-question recall shows the same questions failing for every model. g04 on
grief sits at 0.125 throughout. g08 on fear, g11 on why bad things happen to
good people, g12 on burnout, g14 on prayer and g16 on repentance are all at or
below 0.375 for at least one model.

P2-prep predicted this, for a reason that still holds: Nave's indexes grief and
fear largely by narrative instance, so the gold lists for those questions are
full of narrative passages, and no retrieval method finds narrative examples
from an abstract emotional question. The gold lists are as much on trial here
as the retrieval is. Both should be re-examined in P3.

### Index size

Carrying all three models, index.db is 1,001,975,808 bytes. That is a
measurement rig, not a shippable artifact. Only one model's vectors ship, and
the choice materially changes the installer:

    base index, no vectors                        76 MB
    + bge-small-en-v1.5, 384 dim                 +102 MB
    + nomic-embed-text-v1.5, 768 dim             +206 MB
    + qwen3-embedding-0.6b, 1024 dim             +275 MB

### Full-set size, for the summarize-all mode

Configuration F returns 241 to 547 passages per question, median 368, covering
636 to 974 verses, median 828, which is 16,722 to 27,873 tokens of verse text,
median 23,003. That is the size of the reading PLAN 5.6's "Summarize all N
passages" button has to digest. It does not fit one context window on a small
CPU model, which is why 5.6 specifies batching and a merge. P3 measures whether
that is fast enough to offer.

## Chat model evaluation (P3)

Measured 2026-08-26 on an AMD Ryzen 7 5800X, 8 physical cores, 16 logical,
31.9 GB RAM, with 19 to 21 GB free before each model load. CPU only: the
llama.cpp build is win-cpu-x64 and the machine's RTX 3080 is not used, so every
figure here is a CPU figure and the hardware floor is a CPU floor.

Execution was strictly sequential, and that is enforced rather than merely
intended. llama-server runs with -np 1, a single slot, so it cannot serve two
requests at once. The harness loops one question at a time and writes each
metrics row before the next question starts. The embedding server and the chat
server never run together: query vectors for the whole set are embedded first,
the embedding server is stopped, and only then is the chat server started.
Every run records embed_phase_separate true. Before each load the harness reads
free RAM and refuses any model whose file size plus 2 GB of headroom exceeds
it; every load this session passed with at least 14 GB to spare. Servers run at
below-normal priority.

### Candidates

All three are Apache-2.0 with first-party GGUF builds from the Qwen
organisation, instruction-tuned, and current generation. Holding the family
constant makes size the variable that PLAN 6.4 selects on.

    Qwen3-1.7B      apache-2.0   Q8_0      1749.4 MB   context used 8192
      sha256 061b54daade076b5d3362dac252678d17da8c68f07560be70818cace6590cb1a
      Q4_K_M is not published for this size by Qwen; Q8_0 is their smallest.
    Qwen3-4B        apache-2.0   Q4_K_M    2381.6 MB   context used 8192
      sha256 7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5
    Qwen3-8B        apache-2.0   Q4_K_M    4794.9 MB   context used 8192
      sha256 d98cdcbd03e17ce47681435b5150e34c1417f50b5c0019dd560e4882c5745785

No candidate was refused for memory. Qwen2.5-3B-Instruct was excluded before
download: its card's licence field reads "other", not Apache-2.0 or MIT.

Reasoning is disabled with Qwen3's own /no_think switch, and any think block is
stripped before verification, so what is verified is what the reader would see.

### Results, canon 66, ten graded questions

P3 graded runs, canon 66, 10 questions each

model          fab1st  retry fallbk cit-prec cit-cov  struct  rewrite   med s   max s  peak MB
----------------------------------------------------------------------------------------------
qwen3-1.7b       0.00   0.00   0.00   0.212   0.820    1.00   -0.162    63.5    96.0     7095
qwen3-4b         0.30   0.30   0.00   0.219   0.333    0.70   -0.262   119.7   377.9    13128
qwen3-8b         0.00   0.00   0.00   0.269   0.792    1.00   -0.250   156.3   234.1    15068

  fab1st   first-pass verifier violation rate
  retry    fraction of questions that needed a second generation
  fallbk   fraction that fell back to the passage list
  cit-prec fraction of cited passages that are in MUST or SHOULD
  cit-cov  fraction of sent MUST passages the synopsis cited
  struct   headings present, every theme cites a sent token
  rewrite  recall@25 with model rewrites minus with hand keywords

Themes per answer, and generation speed

model          themes per question                    tok/s out chars
qwen3-1.7b     [1, 1, 1, 1, 1, 1, 1, 1, 4, 1]          12.8       958
qwen3-4b       [1, 0, 4, 5, 4, 4, 3, 0, 5, 0]           6.5      1613
qwen3-8b       [5, 4, 4, 5, 5, 5, 5, 5, 5, 4]           5.8      2051

Per-question recall@25, model rewrites vs hand keywords

q      qwen3-1.7b         qwen3-4b           qwen3-8b          
g01    0.50 / 0.75        0.50 / 0.75        0.50 / 0.75       
g03    0.62 / 0.62        0.50 / 0.62        0.62 / 0.62       
g04    0.50 / 0.25        0.12 / 0.25        0.25 / 0.25       
g05    0.50 / 1.00        0.12 / 1.00        0.25 / 1.00       
g08    0.12 / 0.50        0.12 / 0.50        0.00 / 0.50       
g11    0.38 / 0.62        0.12 / 0.62        0.38 / 0.62       
g12    0.00 / 0.12        0.00 / 0.12        0.00 / 0.12       
g13    0.75 / 1.00        0.75 / 1.00        0.75 / 1.00       
g14    0.38 / 0.50        0.50 / 0.50        0.25 / 0.50       
g18    0.62 / 0.62        0.62 / 0.62        0.50 / 0.62

### Gates and selection

Thresholds were fixed before the 4B and 8B results were seen and are recorded
in DECISIONS.md: first-pass violation rate at or below 0.30; fallback rate
below 0.10; structure compliance 100 per cent; median end-to-end latency at or
below 180 seconds with no question above 300.

    model         fab<=0.30  fallback<0.10  structure=1.00  median<=180  max<=300
    qwen3-1.7b    pass 0.00  pass 0.00      pass 1.00       pass 63.5    pass  96
    qwen3-4b      pass 0.30  pass 0.00      FAIL 0.70       pass 119.7   FAIL 378
    qwen3-8b      pass 0.00  pass 0.00      pass 1.00       pass 156.3   pass 234

Applying PLAN 6.4: the smallest model passing every gate is Qwen3-1.7B, so it
is the fallback. The next size that passes is Qwen3-8B, so it is the default.
Qwen3-4B is rejected. Size did not order quality here: the middle model was the
worst of the three on structure, on citation coverage, and on worst-case
latency.

No fabricated reference reached a simulated reader in any run. That is true by
construction rather than by the models behaving well: the verifier strips and
retries, and falls back if the retry also fails. Across 31 generations this
session, graded, smoke and Deuterocanon together, the fallback was never
reached, so every answer shown was a clean generation.

### What the gate numbers do not capture

Qwen3-1.7B passes structure compliance as P3 defines it, but nine of its ten
answers contain exactly one theme heading followed by a passage-by-passage
paraphrase. Qwen3-8B produced four or five themes on every question. The metric
asks whether headings exist and cite properly; it does not ask whether the
result reads as a synopsis rather than a list. Theme counts are reported above
for that reason, and the difference is plain to a reader in a way the gate is
not.

Citation precision is an index-derived proxy and should be read as one. A cited
passage counts as precise if it overlaps a MUST or SHOULD passage, and those
lists were themselves drafted from the same corpora and never reviewed by a
pastor. The figures, 0.21 to 0.27, mostly reflect that 25 passages are sent and
the gold lists cover only a fraction of them.

### Query rewriting makes retrieval worse

Every candidate's rewrites lowered recall@25 against the hand-written keyword
lists: by 0.162 for the 1.7B, 0.262 for the 4B, 0.250 for the 8B. The effect is
consistent across models and across questions rather than one bad case. On g05,
money and debt, the hand keywords reach 1.00 and every model's rewrites reach
0.50 or less.

PLAN 5.2 assumes the chat model's rewrites help. On this evidence they do not.
Recorded as a finding rather than acted on: 5.2 is an approved decision and ten
questions is a small sample. P4 should measure it again and decide whether the
rewrite step is kept, dropped, or made additive to the raw question rather than
a replacement for it.

### The summarize-all path is not usable as specified

Run once on g12, the graded question with the largest retrieved set: 548
passages, grouped by book into 6 batches of 44 to 115 passages.

    total wall time            1993 seconds, 33 minutes
    batches                    6, from 108 to 278 seconds each
    merge                      273 seconds
    fabrication events         1, in batch 5, recovered on its retry
    merge verdict              ok, no retry needed
    peak resident memory       16932 MB at 16384 context
    final output               2960 characters
    tokens cited by batches    253
    carried through the merge  123
    dropped at the merge       130

Thirty-three minutes is not an interactive feature, and the merge silently drops
just over half the passages the batches cited, which defeats the point of
summarizing the whole set. The citation guarantee held throughout: the verifier
ran on all six batches and on the merge, one violation was caught and fixed on
its retry, and nothing unverified survived.

This is a finding for P4 and P5, not a failure of the plan's intent. The mode
needs a much smaller model for the batch stage, or a hierarchical merge that
cannot drop citations, or an honest re-scoping.

### Hardware floor, measured

Disk, from the files themselves:

    index.db, bundled, nomic vectors only        366.8 MB
    embedding model, bundled                     261.6 MB
    default chat model, downloaded on first run 4794.9 MB
    fallback chat model                         1749.4 MB
    application binary and sidecar               not yet measured; P6

    default configuration total                 5423.3 MB, about 5.3 GB
    fallback configuration total                2377.8 MB, about 2.3 GB

Memory, measured two ways because they answer different questions. A controlled
single answer, one model loaded, one query:

    embedding server        peak resident   246 MB    private   139 MB
    Qwen3-8B Q4_K_M         peak resident  8998 MB    private  4615 MB
    Qwen3-1.7B Q8_0         peak resident  2768 MB    private  1076 MB

Observed worst case across a full ten-question run, which is what a reader
could actually hit:

    Qwen3-8B                15068 MB
    Qwen3-1.7B               7095 MB
    Qwen3-8B, summarize-all 16932 MB at 16384 context

The two sets differ because Windows lets a working set grow while RAM is free,
and llama.cpp memory-maps the model file. Private bytes is what the machine
must find; resident peak is what it will use when it can. On this evidence the
default model wants 16 GB of RAM to be comfortable and the fallback wants 8 GB.
Those are the figures README states, and they are a judgement drawn from the
measurements above rather than a measurement themselves. P7 tests them on a
clean machine.

### Deuterocanon, secondary and non-gating

Additive canon retrieval was built and is on by default in both-canon mode. The
canon-66 result is now a prefix of the both-canon result by construction, which
tests/test_embeddings.py asserts for g19 and g20. Turning the setting on can
only add passages, never take them away. That closes the displacement P2
measured, where enabling the Deuterocanon removed up to 15 of 25 protestant
passages.

Retrieval, configuration F: recall@25 against MUST is unchanged between modes
for both questions, 0.286 for g19 and 0.571 for g20, because the slice is
appended rather than substituted. With the slice included in what is sent to
generation, 2 of the 6 deuterocanonical MUST passages for g19 and 3 of 6 for
g20 reach the model.

One generation run, g19, both-canon, default model: 32 passages sent of which 7
were deuterocanonical, all 7 cited, the "(Deuterocanon)" marker preserved in
the output, citation precision 0.474. The first pass was rejected for writing
"Psalm 41" and "Job 31" as chapter-only references, and the retry passed. That
is the verifier doing exactly what it exists for.

## Rust backend (P4)

P4 reimplemented retrieval, the citation verifier and generation in Rust and
ran the same ten graded questions through a CLI harness that calls the same
code the app will call. Everything below is recomputed from the stored run
artefacts by pipeline/report_p4.py, not copied from a summary file.

### Parity with the Python harness

Retrieval was checked against committed fixtures rather than a live Python
process: pipeline/make_fixtures.py stores the query vector, the whole candidate
set with scores and origin tags, the passages those candidates group into, the
cut sent to generation and the matched topics, for fourteen cases, and the Rust
test reproduces them.

    cases compared            14   10 graded questions in canon 66, and
                                   g19 and g20 in canon 66 and both
    candidates compared   12,685   in rank order, with scores and origin tags
    mismatches after fix       0
    score tolerance         1e-5   not loosened

One mismatch was found and fixed rather than tolerated. On g08 the Rust
retriever put Pro 2:5 above Psa 19:9 where the harness has them the other way.
Their true cosine similarities differ by 1.8e-7: numpy's BLAS sums in blocks
and lands within 1e-9 of the exact value, while a sequential float32 sum drifts
2.4e-7 and crosses the gap. Accumulating in f64 is both the more accurate
answer and the one that reproduces the harness, and that is what the Rust
retriever now does.

The verifier was checked twice. All 35 contract vectors hold in both
implementations, with identical violation records and not merely identical
verdicts. Then every output P3 stored was re-verified: 82 rows, being the first
pass and the final answer of all 41 P3 generations, compared on verdict, on
each violation's kind, text, reason and character span, on the stripped text,
on the retry failure note and on the fallback rendering. Zero differences.

    contract vectors            35 of 35
    P3 outputs compared         82 rows, from 41 generations
    violation records compared  14, field by field including spans
    differences                  0

### Retrieval latency in Rust

Release build, warm index, on the same machine.

    index load, once per process     0.50 s   768-dimensional vectors for
                                              31,102 verses and their
                                              pericopes and topics
    per query, canon 66              0.039 s median, 0.048 s max
    per query, both-canon            0.078 s to 0.092 s
    the Python harness, for scale    0.015 s median

Rust is about 2.6 times slower per query than numpy's BLAS, and both are
irrelevant beside a generation that takes two and a half minutes.

### End to end, ten graded questions, canon 66, Qwen3-8B

    id    first-pass  retry  fabrications  cited  precision  coverage  themes  seconds
    g01   ok          -      0             11     0.27       0.75      5       134.7
    g03   ok          -      0             15     0.47       1.00      4       159.7
    g04   ok          -      0              6     0.00       -         5       154.7
    g05   ok          -      0              9     0.22       0.50      4       210.1
    g08   ok          -      0             14     0.00       -         4       112.8
    g11   ok          -      0             16     0.00       0.00      5       110.8
    g12   ok          -      0              9     0.00       -         5       180.7
    g13   ok          -      0             13     0.54       0.83      5       143.3
    g14   ok          -      0             14     0.21       1.00      5       190.7
    g18   ok          -      0             19     0.47       1.00      5       173.1

    fabricated references reaching output    0     the hard gate
    first-pass violation rate             0.00
    retry rate                            0.00
    fallback rate                         0.00
    structure compliance                  1.00     4 or 5 themes every time
    median end to end                    157.2 s
    max end to end                       210.1 s
    peak sidecar RAM                      9001 MB

The fabrication count is checked against the text a reader would actually see
and not against an intermediate: every [P#] in it must be one that was sent,
and no written reference of any kind may appear. Zero of both, on all ten.

### Against P3's Python numbers

                                  P4 Rust   P3 Python    delta
    first-pass violation rate        0.00        0.00    +0.00
    retry rate                       0.00        0.00    +0.00
    fallback rate                    0.00        0.00    +0.00
    citation precision              0.219       0.269   -0.050
    citation coverage               0.726       0.792   -0.065
    recall@25 of the sent set       0.362       0.350   +0.013
    median end to end             157.2 s     156.3 s    +0.9
    max end to end                210.1 s     234.1 s   -24.0
    median generation             151.7 s     156.3 s
    peak sidecar RAM              9001 MB     8998 MB controlled, and
                                              15068 MB observed over a run

The two runs did not retrieve from the same input. P3 retrieved with the
model's rewritten queries; P4 retrieves from the raw question, which is the
decision recorded below. Citation precision and coverage therefore compare two
different retrieved sets and are not a like-for-like measure of the port. The
verifier figures, the fabrication count and the latency are like for like, and
they agree.

The one figure that moved for a reason worth naming is peak memory. P3 observed
15,068 MB over a ten-question run and 8,998 MB in a single controlled answer;
P4 measures 9,001 MB on every question. The difference is not the context size:
P3 kept one llama-server alive for all ten questions and P4 starts a fresh one
per question, so nothing accumulates across questions. P5 will keep a server
alive between questions for speed and must measure this again before README's
floor is trusted.

### Context window

PLAN's assumption was that P3's 15 GB peak came from an oversized context. It
did not, and the measurement says so plainly.

    prompt tokens over the ten questions   min 2,709  median 3,967  max 5,819
    output budget                          900
    max prompt plus output                 6,719
    plus a 25 per cent margin              8,398

The context in use is 8,192, which is smaller than the derived figure rather
than larger: on the worst question the prompt and its output fill 82 per cent
of it. There is no oversized context to trim.

### Query rewriting, decided

P3 measured that model rewrites lowered recall@25 against the hand-written
keyword lists in questions.json. That is a true finding about a comparison the
product cannot make, because hand-written keyword lists do not exist at run
time. P4 measured the rewrite against the reader's own question instead, on the
same ten questions, configuration F, canon 66, recall@25 against MUST.

    raw       0.3625   the question embedded alone, its content words as the
                       keyword terms
    rewrite   0.3500   what P3 ran: question and rewrites embedded together,
                       the rewrites as the keyword terms
    fused     0.4000   the same vector, content words and rewrites together
    hand      0.4875   for scale only; not available at run time

    fused minus raw       +0.0375   95 per cent [-0.0125, +0.0875]
    fused minus rewrite   +0.0500   95 per cent [-0.0250, +0.1250]
    rewrite minus raw     -0.0125   95 per cent [-0.1375, +0.1000]

No pair is separable on ten questions. By the standard this project already
applied to the embedding models, that is a tie, and the tie is broken on cost:
the rewrite needs a second chat-model load and a generation, measured at 3.7
seconds and about 6.6 seconds on this machine, for a gain the interval says may
be zero. The default is the raw question; --query rewrite and --query fused
remain.

The figure worth keeping is the last one. Hand-written keyword lists still beat
everything the product can actually do, 0.4875 against 0.3625, and closing that
gap is a retrieval problem rather than a prompt problem.

## Results

Populated in P2 (retrieval), P3 (model selection) and P4 (the Rust backend).
