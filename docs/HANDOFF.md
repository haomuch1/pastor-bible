# HANDOFF

Session: P4 Generation and verifier in the Rust backend
Date: 2026-08-26
Status: P4 COMPLETE

## State

Repository at D:\Haomuch-Programs\The-Pastor-Bible, branch main, pushed to
https://github.com/haomuch1/pastor-bible, still private.

The Rust backend exists and answers. `src-tauri/core` is a new crate,
`pastor-bible-core`, carrying the whole query pipeline with no GUI dependency:
index access, retrieval, the citation verifier, the crisis matcher, prompt
loading, the sidecar manager and the orchestration that ties them together. The
Tauri shell in `src-tauri` is now a workspace root over it and is otherwise
untouched; P5 wires the two together.

`pastor-bible-cli` is the harness. `ask "<question>"` runs the whole pipeline
and prints the answer structure as JSON plus a readable rendering, with
`--canon 66|both`, `--model default|fallback|<file>`, `--query raw|rewrite|fused`,
`--ctx N`, `--threads N`, `--gpu-layers N`, `--allow-both-servers`, `--json <path>`
and `--quiet`. `selftest` exercises the sidecar without a chat model.

New documents: docs/SIDECAR.md closes PLAN section 16's llama-server VERIFY
item with flags and endpoints verified against the binary; docs/API.md
specifies the answer structure P5 consumes. docs/VERIFIER.md now carries 35
test vectors rather than 25, and records why. docs/EVAL.md has a "Rust backend
(P4)" section with every number this session measured.

data/crisis_terms.txt is populated: 117 phrases covering harm to self, harm to
others, and the language of despair and danger. data/crisis_note.txt is the
single source for PLAN 9.3's wording, and a test asserts README quotes it
exactly.

New Python: pipeline/make_fixtures.py writes the parity fixtures,
pipeline/rewrite_decision.py measured the query-mode question,
pipeline/run_p4.py drives the CLI over the graded set, pipeline/report_p4.py
recomputes every figure from the stored artefacts. tools/fetch_llama.py fetches
a pinned llama.cpp asset and refuses to unpack a mismatched checksum; it is the
one file under tools/ that is committed.

## Verified

**The sidecar cannot be orphaned.** Three lifecycle tests: spawn, health,
embed, stop with the child confirmed gone; a second spawn refused while one is
alive and the manager usable again afterwards; and the one that matters, a
child that does not survive a hard kill of its parent. That last test launches
the CLI as a separate process, reads the sidecar's pid from it, kills the
parent with TerminateProcess so no destructor and no handler runs, and asserts
the sidecar died with it. On Windows the guarantee is a Job Object with
KILL_ON_JOB_CLOSE; on Linux it is PR_SET_PDEATHSIG set before exec.

**Retrieval reproduces the Python harness exactly.** 14 cases, 12,685
candidates in rank order with scores, origin tags and canon tags, the passages
they group into, the cut sent to generation and the matched topics. Zero
mismatches at a 1e-5 tolerance that was not loosened. One real mismatch was
found and fixed: a sequential f32 dot product ranked two passages whose true
cosines differ by 1.8e-7 the wrong way round, and accumulating in f64 is both
more accurate and what the harness does.

**The verifier agrees with Python on everything either has ever seen.** All 35
contract vectors, with identical violation records rather than merely identical
verdicts. Then all 82 outputs P3 stored, being the first pass and the final
answer of 41 generations, compared on verdict, on each violation's kind, text,
reason and character span, on the stripped text, on the retry note and on the
fallback rendering. Zero differences.

**A hole in the citation guarantee was found and closed.** Rule B could not see
any multi-word book name, because the pattern was built from the space-stripped
normalised key. Measured against 83 realistic book names, 14 were invisible:
"Song of Solomon", "Song of Songs", "Acts of the Apostles" and eleven
deuterocanonical names, which is exactly what a both-canon answer cites. Fixed
in Python and Rust together, ten new contract vectors added in both directions,
and proved to change nothing on P3's record: 41 first-pass verdicts, 14
violation records field by field, 41 final answers, all identical before and
after. Jared decided this rather than the alternative of porting the gap for
parity's sake.

**Ten graded questions end to end, canon 66, Qwen3-8B, through the CLI.**
Fabricated references reaching output: 0, checked against the text a reader
would see. First-pass violations 0, retries 0, fallbacks 0, structure
compliance 1.00 with four or five themes every time. Median 157.2 seconds
against P3's 156.3, maximum 210.1 against P3's 234.1. Peak sidecar RAM 9,001
MB on every question.

**Sanity runs.** Two questions on the fallback 1.7B: verdict ok both, zero
fabrications, 30 and 34 seconds, peak 2,769 MB, and one theme each, which is
the list-style behaviour P3 documented and README now warns about. g19 in
both-canon mode: 32 passages sent of which 7 deuterocanonical, canon tags
carried in the passage panel, the Deuterocanon footer present, verdict ok, zero
fabrications.

**The crisis matcher.** 117 phrases, both halves present, 15 positive and 15
negative sentences all correct, and a list with no terms in it refuses to load
rather than pretending to work.

**Context window.** PLAN's assumption that P3's 15 GB came from an oversized
context is wrong, and the measurement says so. Prompt lengths are 2,709 to
5,819 tokens; with the 900-token budget and a 25 per cent margin the derived
context is 8,398, larger than the 8,192 in use. Re-running the three largest
questions at 8,448 cost 36 MB and no time. The 15 GB was server lifetime: P3
kept one llama-server alive across ten questions; a fresh one per question
peaks at 9,001 MB.

**Query rewriting, decided by measurement.** raw 0.3625, rewrite 0.3500, fused
0.4000 recall@25 against MUST; fused minus raw is +0.0375 with a 95 per cent
interval of [-0.0125, +0.0875]. Not separable on ten questions, so the tie goes
to the cheaper mode. Default is raw; the other two are one flag away.

## Not verified

The fallback path has still never been reached in a real run. Across P3 and P4
together, 55 generations, no retry has ever failed. Its output shape is covered
by tests and by the fixtures; it has not been seen in production of any kind.

Citation precision and coverage are not a like-for-like comparison with P3. P3
retrieved with the model's rewrites and P4 retrieves from the raw question, so
the two runs sent different passages. The verifier figures, the fabrication
count and the latency are like for like.

Ten questions is still a small sample, and the gold lists are still
index-derived and unreviewed by a pastor.

Peak memory was measured with a fresh server per question. P5 will keep a
server alive between questions for speed and must measure again before
README's 16 GB floor is trusted; P3's 15,068 MB is what a long-lived server
looked like.

The concurrent-sidecar path was smoke-tested, not measured. `--allow-both-servers`
keeps the embedding server up beside the chat server and the answer records
that it did; nobody has measured what it saves.

Nothing was measured on any machine but this one.

## Flags for Jared

**The verifier had a hole and it is worth knowing how it got there.** The
implementation and its own specification disagreed for two phases, and nothing
caught it because the 25 test vectors were written from the same mental model
as the code. The ten new vectors were written from the failure. It is worth
assuming there are others, and the cheapest defence is more vectors drawn from
what models actually write.

**The Deuterocanon marker cannot be left to the model.** On g19 the 8B cited a
deuterocanonical passage and dropped the "(Deuterocanon)" marker the prompt told
it to keep. The passage panel carries `canon` on every passage, so P5 renders
the tag itself. The prompt still asks; nothing rests on the asking now.

**Nave's headings are not usable as headings.** The topic grouping that
replaces summarize-all is grouped under Nave's subtopic labels, and some of
those labels are whole paragraphs; one matched topic on g01 has a heading 1,200
characters long listing every instance of answered prayer in scripture. The
answer structure carries `heading_display`, a trimmed label, beside the source
text. It makes the grouping shippable rather than good. Better topic labels are
a P1 change to how subtopics were derived, and it is worth deciding whether the
grouping earns its place at all if the labels stay like this.

**Hand-written keywords still beat everything the product can do.** 0.4875
recall@25 against 0.3625 for the raw question. That gap is the single largest
retrieval lever left, and it is not a prompt problem: it is that a person who
knows the Bible picks better search terms than a question does. Worth thinking
about whether some curated query expansion could ship in the index.

**Two and a half minutes is still the honest speed.** Nothing this session
changed it. The Vulkan measurement is in EVAL.md and P6 decides what to do with
it.

**Answer quality is still unrated.** The graded answers from this session are
in data/eval/runs/p4-rust-8b/raw/ as JSON if you want to read ten of them.

## Next session

P5 Frontend and first-run, per PLAN section 7 and the amendments logged today.

Screens per section 7, with three things this session decided. Retrieved
passages are displayed as soon as retrieval returns, which is 40 milliseconds,
so the reader is in the text while generation runs. The synopsis appears only
after the verifier passes; nothing unverified is ever on screen, even briefly.
Progress is a stage indicator and a token counter, never streamed text.

The full passage set is grouped under the matched Nave's topic headings, per
the amended PLAN 5.6 and 7.2: topics in match order, passages within a topic in
canonical order, everything else under "Other passages". The backend already
returns this as `topic_groups`. Summarize-all is gone from v1.

The Deuterocanon tag in the synopsis is rendered from `passages[].canon`, not
from the model's prose. See docs/API.md.

History per section 8, in user.db in the app data directory. The downloader,
with checksum and resume, is for the chat model only: the embedding model is
bundled. Self-test on first run.

Read PLAN.md, DECISIONS.md, API.md, SIDECAR.md and this file before starting.
Do not begin P6.
