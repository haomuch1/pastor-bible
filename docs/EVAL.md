# EVAL

Evaluation protocol for The Pastor Bible. Derived from PLAN.md sections 6.2 and
6.3.

This file is a protocol skeleton. It holds no results and no numbers. Thresholds
are set in P3, against measurement, never estimated. Retrieval-only figures
arrive in P2; full-pipeline figures in P3. Nothing is written here that was not
measured on this project's own data.

## Evaluation set

Location: data/eval/questions.json

Roughly 40 questions of the kind a pastor would actually ask, spanning everyday
life topics and doctrinally neutral study topics. Each question carries a gold
list: the passages a correct answer should surface.

How gold lists are made: Claude drafts candidates from the built indexes. Jared
reviews and approves every list. Gold lists are judgment, not output. They are
not auto-generated and not delegated. P2 does not begin until the gold lists are
approved.

The eval set does not exist yet. It is built in P1/P2 once there is an index to
draw candidates from.

## Metrics

Retrieval recall@25
  Fraction of gold passages appearing in the top 25 retrieved passages, measured
  per question and averaged. Measures index and retrieval quality, independent of
  the chat model. Reported per configuration in P2.
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

## Results

None yet. Populated in P2 (retrieval) and P3 (full pipeline).
