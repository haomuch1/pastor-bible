# HANDOFF

Session: P2 Index and retrieval harness
Date: 2026-08-26
Status: P2 COMPLETE

## State

Repository at D:\Haomuch-Programs\The-Pastor-Bible, branch main, pushed to
https://github.com/haomuch1/pastor-bible, still private.

The evaluation set is approved. data/eval/questions.json carries status
"approved-as-drafted" and a note field saying in plain words that the lists are
index-derived and were never reviewed by a pastor. g19 and g20 each gained six
deuterocanonical MUST passages, taking those two lists to 14 entries;
docs/EVAL-GOLD-REVIEW.md is now a readable record of what was approved rather
than a form to mark up.

index.db is version 0.2.0, schema 2, and holds embeddings from all three
shortlisted models side by side. It is 1,001,975,808 bytes, which is a
measurement rig and not a shippable artifact: only one model's vectors ship.
It is gitignored, as are its journal and WAL sidecars, which P2 added to
.gitignore after the journal briefly got staged.

New in pipeline/: embed.py runs llama-server for embedding and reranking and
owns the truncation and tokenizer helpers; build_embeddings.py writes the
vectors; retrieve.py is the retrieval harness implementing PLAN 5.1 to 5.5 with
every stage behind a flag; evaluate.py runs the configurations over the graded
questions; report_eval.py turns that into the tables in docs/EVAL.md;
check_determinism.py measures embedding variance. All three of build_index.py,
build_embeddings.py and retrieve.py honour a TPB_INDEX_DB environment variable
so a second database can be built without destroying the first.

tools/ holds the llama.cpp binaries and models/ holds the four GGUF files. Both
are gitignored. Nothing binary is committed.

pipeline/requirements.txt now also pins numpy 2.5.2, used only by the harness.

## Verified

Prerequisites and setup. llama.cpp release b10639, win-cpu-x64 prebuilt, which
identifies itself as version 0.3.0-dev build 10639 commit 5e6a37cb1. The
llama-cpp-python route was rejected because PyPI ships only an sdist for it and
no CMake exists on this machine, so building it would have meant installing
system software.

Licences were read from each model's own card through the Hugging Face API,
not from memory: bge-small-en-v1.5 mit, nomic-embed-text-v1.5 apache-2.0,
Qwen3-Embedding-0.6B apache-2.0, bge-reranker-v2-m3 apache-2.0, and the GGUF
repository gpustack/bge-reranker-v2-m3-GGUF apache-2.0 as well. Parameter
counts and context lengths came from the safetensors metadata and config.json
of each model. Every GGUF was downloaded and its sha256 recorded in
docs/EVAL.md and in the embedding_models table.

Prefix conventions were verified from the model cards rather than assumed. They
matter: nomic's search_document and search_query prefixes are mandatory, not
advisory. The exact strings are stored in the index next to the vectors, and the
harness reads them from there.

Pericope sizes, measured before embedding, verses then tokens, as min, median,
p90, max. OT protestant, 6209 pericopes: 1, 3, 7, 298 verses and 11, 98, 245,
6074 tokens. NT protestant, 2466: 1, 3, 6, 16 verses and 10, 76, 185, 477
tokens. Deuterocanon, 1377: 1, 4, 10, 38 verses and 11, 128, 309, 1068 tokens.
Overall 10,052 pericopes: 1, 3, 8, 298 verses and 10, 96, 239, 6074 tokens.
Pericopes are short because they are WEB paragraphs and no headings were added.

Embedding coverage, queried from the finished database. Each of the three
models has 38,029 verse vectors, 18,837 topic vectors, and complete pericope
coverage: 10,189 parts for bge-small with 106 pericopes split to fit its
512-token context, 10,056 parts and 3 splits for nomic, 10,055 parts and 2
splits for qwen3. Wall time 478s, 1369s and 7825s respectively. Peak resident
memory 74.9 MB, 251.4 MB and 945.9 MB; the reranker peaked at 2009.3 MB.

Embedding determinism was measured, not assumed. The same fixed 200-verse
sample embedded twice gives a maximum absolute difference of 9.6e-05 for
bge-small, 6.1e-05 for nomic and 2.7e-03 for qwen3, with minimum cosine
similarity between the two runs of 0.999999877, 0.999999898 and 0.999747860.
197 of 200 vectors were bit-identical in each case. Byte-identical output was
never expected from a threaded CPU matmul; the variance is orders of magnitude
below anything that could reorder a result.

Reproducibility of the whole pipeline was verified against a second artifact,
not asserted. P1's structural build is still byte-deterministic: rebuilt from
scratch it produced the same pre-checksum digest, 6ed1005e. A fresh index was
then built from scratch into a separate file, embedded with the recommended
model, and evaluated. All six configurations reproduced their recall@25 to
three decimals, and a per-question comparison across 120 question-configuration
pairs and three cutoffs each found zero differences.

The headline numbers are in docs/EVAL.md and were read out of the built
database by the harness. Vector-only recall@25 against MUST, which is the
evidence: bge-small 0.346, nomic 0.428, qwen3 0.462 for verses and pericopes
fused. Keyword-only scores 0.559 and the full pipeline 0.571 to 0.629, both
near-circular against these lists and reported as such.

Tests: 61 run, 61 passed. That is P1's 28, the eval set's 19 after the P2
amendments, and 14 new ones over the embeddings and the harness. Two P1-era
tests were deliberately inverted: one asserted the embedding tables did not
exist, and one tied questions.json to the live build checksum, which no longer
holds now that adding embeddings changes the file. Provenance is kept instead
by a gold_lists_index row in meta, and the tests assert against that.

## Not verified

The gold lists are still index-derived and unreviewed by a pastor. Everything
measured this session is measured against them. Recall of 0.43 means 43 per
cent of what those lists claim matters, not 43 per cent of what a pastor would
say matters. That distinction is written into questions.json and EVAL.md
because it is the largest single caveat on every number here.

No generation happened. No chat model ran, no answer was produced, no citation
was verified. Whether these passages support a good answer is P3's question and
is untouched.

The reranker was tested in one configuration only, at the top 60 candidates
with documents truncated to 2000 characters, using the query text without its
keywords. A different cut might behave differently. Given it cost 10 seconds
and 2 GB while lowering recall@25, that was not worth chasing further in P2.

Query rewriting is absent. The stored keyword lists stand in for PLAN 5.2, so
every number here reflects hand-written keywords rather than what the chat model
will actually produce. P3 can compare the two; the keyword lists are stored in
questions.json for exactly that.

The full three-model rebuild was not repeated end to end. Rebuilding all three
takes about 2.7 hours, dominated by qwen3. The reproducibility check was run
with the recommended model instead, which exercises the same code path from an
empty database, and P1's structural build was confirmed byte-identical
separately. The bge-small and qwen3 figures therefore come from the single
build that produced the current index.db, not from a repeated one.

Nothing was measured on any machine but this one. Latency and memory figures
are from this CPU.

## Flags for Jared

The reranker is out, and that is a finding rather than a preference. It was the
only reranker that met every condition, and it made retrieval worse: recall@25
fell from 0.629 to 0.529 with the recommended embedding model, while adding
about 10 seconds per query and 2 GB of resident memory. Retrieval without it
takes 3 to 24 milliseconds. If reranking is wanted later it needs a different
model, not a different setting.

The three embedding models cannot be told apart on 20 questions. A paired
bootstrap puts every pairwise gap's confidence interval across zero. The
recommendation of nomic-embed-text-v1.5 rests on it leading every configuration
on the point estimates, on its 2048-token context leaving 3 pericopes needing a
split where bge-small needs 106, and on it costing 251 MB where qwen3 costs 946
MB. If the P3 hardware floor turns out tight, bge-small is the fallback and the
recall cost is not one this eval set can prove is real.

Embeddings are what reach the Deuterocanon. In P2-prep no deuterocanonical
passage could get into a gold list, because neither study corpus indexes those
books. With vectors, bge-small returns Tobit 12:8-9 at rank 1 for g19, and 2 to
5 of the 6 deuterocanonical MUST passages per question land in the top 25. This
is the clearest demonstration in the session of what the embeddings buy.

The canon toggle changes what protestant readers see. Enabling the Deuterocanon
displaces between 5 and 15 of the 25 protestant passages, every model, both
configurations. It is not a filter over a fixed list. A reader who turns the
setting on loses passages they would otherwise have been shown. Whether to say
so in the interface is a P5 product decision and it is yours.

Six questions retrieve badly for every model: g04 on grief worst at 0.125
throughout, then g08, g11, g12, g14 and g16. P2-prep predicted g04 and g08 for
a reason that still stands, that Nave's indexes grief and fear by narrative
instance, so their gold lists are full of narrative that no abstract question
retrieves. On those questions the gold lists are as much on trial as the
retrieval.

Index size is now a real constraint on the installer. The base index is 76 MB;
the recommended model adds 206 MB of vectors, bge-small would add 102 MB, qwen3
275 MB. That is before the chat model, which the user downloads separately.

Some Nave's subtopic headings are long prose rather than headings, the longest
1,243 tokens. They are embedded truncated. This comes from P1 deriving subtopic
labels from entry text; it affects a handful of rows and was not worth
reopening P1 for.

## Next session

P3 Model evaluation, per plan section 13.

Scope: run 2B, 4B and 8B-class chat candidates under llama.cpp, all Apache-2.0
or MIT; run the full pipeline including the citation verifier; record metrics in
docs/EVAL.md; select per PLAN 6.4; measure the hardware numbers per 6.5.

P3 also owes three things this session created:

The summarize-all measurement that PLAN 5.6 now requires. Configuration F
returns 241 to 547 passages per question, median 368, covering 636 to 974
verses, 16,722 to 27,873 tokens of verse text, median 23,003. P3 must measure
latency and quality of batching that and merging it, on the largest eval sets,
before P4 builds the button.

A verdict on the weak questions. Either the gold lists for g04, g08, g11, g12,
g14 and g16 are wrong, or retrieval genuinely fails on emotional-support
questions. P3 can tell the two apart by reading what a chat model does with what
was retrieved.

Confirmation or reversal of P2's three recommendations, which are logged in
DECISIONS.md as "recommended, confirmed in P3": nomic-embed-text-v1.5, no
reranker, configuration F.

VERIFY items from plan section 16 owned by P3: the chat model candidates and
their licences, shared with P2 and now half-answered for the embedding side
only. The llama-server binary name, flags and OpenAI-compatible endpoint are
listed under P4 but are already exercised here: the embedding and rerank
endpoints work as used, and pipeline/embed.py records the exact invocation.

Read PLAN.md, DECISIONS.md and this file before starting. Do not begin P4.
