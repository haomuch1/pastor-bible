# HANDOFF

Session: P3 Chat model evaluation
Date: 2026-08-26
Status: P3 COMPLETE, including the secondary Deuterocanon work

## State

Repository at D:\Haomuch-Programs\The-Pastor-Bible, branch main, pushed to
https://github.com/haomuch1/pastor-bible, still private.

Default chat model: Qwen3-8B, Q4_K_M, Apache-2.0. Fallback: Qwen3-1.7B, Q8_0,
Apache-2.0. Qwen3-4B was evaluated and rejected. Both selections are logged in
DECISIONS.md as decided, with the gate table in docs/EVAL.md behind them.

The evaluation set now carries p3_graded, the ten questions P3 graded, and
smoke_pool, the thirty that did not gate. The ten graded questions that moved
to the pool kept their gold lists untouched; twenty pool questions are reserved
for post-v1 testing.

New documents. docs/VERIFIER.md is the citation-guarantee specification with 25
test vectors, and it is the contract P4 ports to Rust. docs/SMOKE.md holds ten
answers exactly as the verifier passed them, with the passages each cited, for
reading rather than rating. README's hardware section is written from
measurement and no longer says "filled in at P3".

New code in pipeline/: verifier.py implements docs/VERIFIER.md; chat.py runs
llama-server for generation with a single slot, a free-RAM check and
below-normal priority; run_p3.py is the sequential graded and smoke harness;
summarize_all.py is the batched full-set path; report_p3.py recomputes metrics
uniformly from the stored artifacts; write_smoke.py renders SMOKE.md.

data/prompts/ holds five versioned prompts: rewrite, synopsis, retry,
summarize_batch, summarize_merge, all version 1.

data/eval/runs/ holds committed metrics.jsonl and summary.json per run. The raw
per-question dumps are gitignored; the metrics are not.

Four chat GGUFs and the llama.cpp binaries are in models/ and tools/, both
gitignored. Nothing binary is committed.

## Verified

Sequential execution is enforced, not merely intended. llama-server runs with
-np 1 so it cannot serve two requests concurrently. The harness loops one
question at a time and appends each metrics row before starting the next. The
embedding server and the chat server never overlap: the whole question set is
embedded first, the embedding server is stopped, and only then does the chat
server start, which every summary records as embed_phase_separate true. One
bug of exactly this kind was written and caught in review before it ran: an
early draft of summarize_all.py opened a chat server and an embedding server in
the same with-statement, and was fixed before execution.

Machine safety held throughout. Free RAM was read before every load and each
model was checked against its file size plus 2 GB. Readings were 18.5 to 21.0
GB free; the largest requirement was 6.7 GB for the 8B. No candidate was
refused, nothing swapped, and servers ran below normal priority. The machine is
an AMD Ryzen 7 5800X, 8 physical and 16 logical cores, 31.9 GB RAM. The
llama.cpp build is win-cpu-x64, so the RTX 3080 present in the machine was not
used and every figure is a CPU figure.

The verifier passes all 25 of its test vectors. Two of them failed on the first
implementation, "First Corinthians 13" and "I Corinthians 13", because the
ordinal-word and roman-numeral prefixes normalised to keys the book table did
not hold; the lookup now rewrites such a prefix to its digit and only accepts
the result if it names a real book, so "Isaiah" is not mangled into "1saiah".
The vectors include the eight false positives P3 named: "the third day", "seven
times", "chapter and verse", Job and Mark as names, lower-case "he acts 2
ways", "Judges 12 times", and Numbers, Kings and Acts used as ordinary words.

Ten graded questions were run through each of the three candidates in canon 66,
one question at a time, 30 runs. Results are in docs/EVAL.md. The headline gate
numbers: first-pass violation rate 0.00, 0.30, 0.00 for the 1.7B, 4B and 8B;
fallback rate 0.00 for all three; structure compliance 1.00, 0.70, 1.00;
median end-to-end 63.5, 119.7 and 156.3 seconds.

No fabricated reference reached a simulated reader in any of the 31 generations
this session. That holds by construction rather than by good behaviour: the
verifier strips, retries once, and falls back. The fallback path was never
reached, so every answer shown was a clean generation. tests/test_p3_runs.py
asserts this property against the committed artifacts rather than trusting the
report.

Structure compliance was recomputed from the stored outputs by report_p3.py
using one definition for all three models, because the harness's own check was
tightened after the first model had already run. The definition used is P3's:
headings present, every theme cites at least one sent token, no theme cites an
unsent token.

The hardware floor was measured twice, deliberately, because peak resident
memory and private bytes answer different questions. Controlled single answer:
the 8B at 8998 MB resident and 4615 MB private, the 1.7B at 2768 MB and 1076
MB, the embedding server at 246 MB and 139 MB. Observed worst case over a full
ten-question run: 15068 MB for the 8B and 7095 MB for the 1.7B. Disk was
measured from the files: a shipping index.db carrying only the chosen embedding
model's vectors is 366.8 MB, built by copying the index, deleting the other two
models' vectors and vacuuming, rather than estimated.

Additive canon retrieval works and is proven. The canon-66 result is a prefix
of the both-canon result by construction, asserted for g19 and g20 in
tests/test_embeddings.py. Two bugs were found and fixed while proving it: the
retriever returned a longer prefix of the full set instead of the 66 slice plus
the appended slice, and the run harness re-derived its own top 25 from the full
set and so never saw the appended passages at all. After the fixes, 32 passages
were sent for g19 of which 7 were deuterocanonical, all 7 were cited, and the
"(Deuterocanon)" marker survived into the output.

The test suite runs and passes: 112 tests.

## Not verified

Ten questions is a small sample and the differences between models are mostly
not separable on it. The selection rests on gate failures that are categorical,
0.70 structure compliance and a 378-second worst case for the 4B, rather than
on small differences in averages.

Answer quality was not rated. No rubric was applied and none was asked for;
docs/SMOKE.md exists so Jared can read ten answers and form a view. Citation
precision is an index-derived proxy against gold lists that no pastor has
reviewed, and is reported as such.

The fallback path was never exercised in a real run, because no retry ever
failed. Its output shape is covered by the verifier's own tests but has not
been seen in production of any kind.

The summarize-all path was run once, on one question, with one model, at one
batch size. Its 33-minute wall time and its dropped citations are one
measurement, not a curve.

Nothing was measured on any machine but this one, and nothing was measured
under memory pressure. The 16 GB and 8 GB figures in README are a judgement
drawn from the measurements, labelled as such there, and P7 tests them on a
clean machine.

The Rust port does not exist. Everything verified here is verified in Python.

## Flags for Jared

Query rewriting hurts. Every model's rewrites lowered retrieval recall against
the hand-written keyword lists, by 0.16 to 0.26 recall@25, consistently across
questions. PLAN 5.2 assumes the opposite. I have not changed the plan: 5.2 is
an approved decision and ten questions is thin evidence. But P4 should measure
it again and decide, because on this evidence the rewrite step is costing the
reader answers.

Summarize-all is not viable as specified. Thirty-three minutes for one
question, 16.9 GB of memory, and the merge dropped 130 of the 253 passages the
batches had cited. The citation guarantee held at every stage, so it is not
unsafe, just impractical. It needs a smaller model for the batching, a merge
that cannot drop citations, or a smaller promise. Worth deciding before P4
builds it.

The middle model was the worst model. Qwen3-4B failed structure compliance and
worst-case latency while sitting between two models that passed everything.
Size did not order quality. If the 8B proves too heavy for real machines in P7,
the answer is the 1.7B, not the 4B.

The 1.7B passes the gates but writes lists, not synopses. Nine of its ten
answers were one heading followed by a passage-by-passage paraphrase. It is the
fallback, so a reader on a small machine gets a visibly poorer answer than one
on a large machine. That is a product consequence worth knowing about, and it
is not visible in any gate number.

Two minutes to four minutes per answer is the honest speed on a fast desktop
processor. README now says so plainly. If that is too slow to ship, the lever
is the model size, and the fallback already shows what the smaller end looks
like.

All three candidates are Qwen3. That was chosen to make size the only variable,
and it is the only current family with Apache-2.0 weights and first-party GGUF
builds at all three sizes. It does mean the selection has no cross-family
check. Phi-4-mini is MIT and is the obvious comparison if one is wanted.

The verifier does not flag a bare book name with no chapter after it. "The
passage from Hebrews says" passes, and should: it names no reference that could
be false. The prompt forbids it and the models mostly comply, but the guarantee
is about references, not about style.

## Next session

P4 Generation and verifier in the Rust backend, per plan section 13.

Port docs/VERIFIER.md to Rust with its 25 test vectors, which are the shared
contract: the Python and Rust implementations must agree on all 25. Then the
sidecar lifecycle, the prompt, the themed synopsis, the retry and fallback, the
crisis matcher, and Deuterocanon labelling. Deliverable per the plan:
end-to-end answers from a CLI harness with zero fabrications on the eval set.

P4 owes three answers this session created:

Whether to keep query rewriting. Measure it again with the selected model and
decide; if it is kept, consider making it additive to the raw question rather
than a replacement.

What to do about summarize-all. Rescope, re-engineer, or drop.

data/crisis_terms.txt is still a comment-only placeholder, and P4 owns it. The
file says in its own text that P4's tests must fail while it has no terms.

VERIFY items from plan section 16 owned by P4: the llama-server binary name,
flags and OpenAI-compatible endpoint. P3 has already exercised much of this and
pipeline/chat.py records the exact invocation that works: -np 1 for a single
slot, -c for context, -ngl 0 for CPU, --no-webui, /health for readiness and
/completion for generation. The rerank and embedding endpoints were exercised
in P2.

Read PLAN.md, DECISIONS.md, VERIFIER.md and this file before starting. Do not
begin P5.
