# CAREER FILE — The Pastor Bible

Source note for the reader: facts below come from (a) docs/PLAN.md v1 dated 2026-08-26 and (b) working-session history 2026-08-26/27 as retained in memory and chat search. Where the plan and the as-built state diverge, the as-built state is stated and the divergence is listed in "Discrepancies" at the end. Anything marked [INFERRED] is Claude's inference, not a recalled fact. Anything marked [UNKNOWN] was not in context and must not be filled in from guesswork.

## 1. Name and one-liner

The Pastor Bible — a free, fully offline Windows/Linux desktop app that answers a plain-language question about life or scripture with a cited synopsis of every place the Bible addresses it, plus the full retrieved passage set shown from the text. Audience: general Bible readers and pastors who need answers they can check; built so a non-technical person can install it.

## 2. Status and location

- Status: in progress. Phases P0–P5.2 complete (scaffold, ingestion, index/retrieval, model evaluation, Rust generation+verifier, full UI, UI corrections). Next phase is P6 (packaging, installers, GPU sidecar bundling, CI). P7 is clean-machine verification and v1.0.0 public release; P8 is code signing via SignPath.
- Repo: github.com/haomuch1/pastor-bible — private until v1.0.0 ships. No public URL yet.
- Runs as a Tauri 2 desktop app; Windows is the primary target, Linux builds planned in P6; macOS dropped (no paid Apple developer account).
- Zero-cost project: free, public, never sold, no fees of any kind.

## 3. Problem solved (business language)

Finding "everything the Bible says about X" by hand means cross-checking a concordance, a topical index, and cross-reference tables, then reading each hit — slow, and the result depends on the reader's skill. Asking a general-purpose chatbot is fast but unreliable: language models routinely invent verse references that don't exist, and most send the question to a cloud server, which many churches and individuals do not want for personal or pastoral questions.

The Pastor Bible runs entirely on the user's machine (internet used only to download the installer and, once, the model file), searches the actual Bible text plus two public-domain reference indexes, and produces a themed, cited summary where every citation is mechanically checked against the retrieved passages before the user sees it. It takes no denominational position and includes no commentary. A crisis note (988 / local emergency / talk to a real person) is shown above — never instead of — the answer when the question contains self-harm or harm-to-others language.

## 4. Tech stack and architecture

Languages / frameworks
- Shell: Tauri 2 (Rust backend, React + TypeScript + Vite frontend).
- Build-time data pipeline: Python (parse USFM, build index). Runs only on the developer's machine; the user's machine never builds embeddings.
- Local inference: llama.cpp (`llama-server`, build b10639) bundled as a Tauri sidecar bound to 127.0.0.1 on a random port; killed on exit with Windows Job Object / Linux PDEATHSIG so no orphan process survives.
- Export: rust_xlsxwriter for .xlsx history export.

Databases
- index.db (SQLite, shipped read-only inside the installer, ~76 MB): Bible text, FTS5 keyword index, verse/pericope/topic embeddings stored as float32 blobs with brute-force search (sqlite-vec evaluated and rejected), TSK cross-reference graph, Nave's topic tables.
- user.db (SQLite, per-user app data at %APPDATA%\io.github.haomuch1.pastorbible, schema v2): question history with the exact passage IDs each answer rested on, FTS5 over history, settings. Survives upgrades and reinstalls.

AI models (all local, all Apache-2.0; no API calls of any kind)
- Embedding: nomic-embed-text-v1.5 (768-dim, ~262 MB), bundled as a Tauri resource.
- Chat default: Qwen3-8B Q4_K_M. Fallback: Qwen3-1.7B Q8_0. Qwen3-4B evaluated and rejected. Chat model downloaded on first run from a hard allow-list of three pinned Hugging Face URLs, checksum-verified, resumable.
- No reranker (bge-reranker-v2-m3 tested: lowered recall and cost ~2 GB).
- GPU: Vulkan path on the reference RTX 3080 gives ~12 s per answer vs ~178 s CPU (7.5 GB VRAM for the 8B). Bundling decision belongs to P6.

Data sources (all public domain, vendored unchanged, credited in NOTICE.md)
- World English Bible, Classic edition (USFM from eBible.org). 66-book Protestant canon always on; single Deuterocanon toggle, off by default, every Deuterocanon passage labeled.
- Treasury of Scripture Knowledge cross-references and Nave's Topical Bible (CrossWire SWORD modules) as index corpora only. No commentary of any kind.

Query pipeline (as built)
- Raw question (no model rewrite) → hybrid retrieval (vector over verses+pericopes + FTS5 keyword, reciprocal-rank fused) → Nave's topic expansion + one-hop TSK expansion, tagged by origin → top passages sent to the chat model with opaque IDs [P1..Pn] → citation verifier → synopsis.
- Both-canon mode retrieves additively: the 66-book result set is preserved and a Deuterocanon slice is appended, because enabling Deuterocanon was measured to displace Protestant results.
- Passages display immediately while the synopsis generates; the passage panel renders verse text from index.db, never from model output.

Notable architectural decisions and why
- RAG over the text, not fine-tuning: fine-tuning yields confident fake references; retrieval yields checkable ones.
- llama.cpp sidecar, not Ollama: Ollama installs a separate always-on service with its own tray icon and updater.
- Apache-2.0 / MIT models only: no Llama community license or Gemma terms flowing obligations to end users.
- Structured text sources only, no PDFs: PDF extraction breaks verse boundaries.
- Prebuilt index in the installer: deterministic (same sources + same model = same checksum); user machine never parses or embeds.
- Single app with a canon toggle rather than two builds (Claude recommended, Jared accepted).
- Installer must upgrade in place with a permanent product identifier; user data lives outside the install directory.
- Ship unsigned first, apply to SignPath after first public release; README states the SmartScreen warning up front.

## 5. Scale indicators

- Lines of code / file count: [UNKNOWN — not in context; re-derive from the repo before use].
- index.db: ~76 MB (approx). Target verse count for the 66 books is 31,102 per plan; the parsed count was re-derived in P1 but the figure is not in this context [UNKNOWN].
- Database tables: index.db as planned has 10 (books, verses, pericopes, verse_fts, embeddings, tsk_refs, nave_topics, nave_topic_verses, topic_embeddings, meta); user.db has 4 (history, history_fts, settings, meta). As-built table names may differ — verify against the schema.
- Citation verifier: 35 test vectors, ported identically to Python and Rust.
- Crisis phrase list: 117 phrases.
- Book display-name table: 81 entries (66 + Deuterocanon).
- Evaluation set: 10 graded questions with gold lists + smoke questions (see Discrepancies for the exact split).
- Generation runs with zero fabricated references: 55 (from the P3/P4 work).
- Test suites: cargo test (Rust), Vitest (frontend), pytest (pipeline). Counts: [UNKNOWN].
- UI screenshots reviewed: 17 in docs/screenshots/.
- Phases completed: 6 of 9 (P0–P5, plus P5.1/P5.2 corrections).

## 6. Hardest problems and how they were solved

1. Zero fabricated references as a mechanical guarantee, not a prompt request.
   Passages are sent to the model under opaque IDs; the prompt forbids naming any reference outside the set. A verifier then checks every [P#] against the sent set and resolves every free-text reference pattern (e.g. "John 3:16") to a verse inside the set. Failure strips the token and retries once with the failure named; a second failure falls back to showing the retrieved passages grouped by book with a note. The verifier was written first in the Python harness, then ported to Rust with a shared 35-vector test suite; the port surfaced a gap in multi-word book names ("1 Samuel", "Song of Solomon") that was fixed on both sides. Result across 55 generations: 0 fabrications.

2. Getting retrieval right without a domain expert to grade it.
   The plan assumed Jared would approve every gold list; he declined because he is not a pastor. The eval was redesigned around index-derived gold lists (Nave's + TSK + keyword hits, explicitly labeled unreviewed) and vector-only ablation recall instead of full-pipeline recall, so results measure agreement with the 19th-century human indexes rather than with a bigger model's opinion. Along the way several "obvious" components were measured and cut: the reranker lowered recall; model query rewrite lowered recall (raw question 0.36 vs hand-written keywords 0.49); enabling Deuterocanon displaced Protestant results, which forced additive retrieval. A static vocabulary-expansion table is logged as a post-v1 idea.

3. Making an 8B model usable on a desktop.
   CPU-only answers took 2–4 minutes on a Ryzen 7 5800X. A planned "summarize all passages" mode took 33 minutes and lost half its citations at merge, so it was cut and replaced with model-free grouping of the full passage set by Nave's root topic. Vulkan offload on the RTX 3080 cut answer time to ~12 s. The UI shows retrieved passages instantly while the synopsis streams, so the user is never staring at a blank screen. Sidecar lifecycle (random port, Job Object / PDEATHSIG kill, cancel returning in ~2.7 s) was built so the app leaves no background process behind.

## 7. Human-in-the-loop, verification, testing, safety

- Mechanical citation verifier with retry and graceful fallback (above); the user never sees an unverified reference.
- Full retrieved passage set is always shown, grouped and browsable, so the reader chooses what to read; the synopsis is a starting point, not the only output (Jared's explicit direction).
- Crisis matcher (117 phrases, harm to self or others) shows a static note above the answer; over-triggering accepted, under-triggering not.
- Deuterocanon passages visibly labeled everywhere they appear.
- Network policy: downloader is the only module with network capability, hard allow-list of three URLs, checksum pinned; plan calls for a CI job that runs the query suite with networking disabled (P6).
- Retrieval in the Rust backend was brought to exact parity with the Python harness before generation was built.
- Standing session rules: one phase per session, documentation before implementation, every decision dated in DECISIONS.md with a one-line reason, counts re-derived from data never carried over, verification against the produced artifact not the script, clean-stop protocol with HANDOFF.md, stop-clause to halt on any conflict with observed reality.
- Test suites: cargo, Vitest, pytest; all required green before every commit. Frontend tests were added in P5.1 after a UI element was found to be missing (see section 9).
- Hardware policy: built and tested on one stated reference machine (Ryzen 7 5800X, RTX 3080 10 GB, 32 GB, Windows 11); advisory-only warnings on weaker hardware, no refusal to run, no automatic model selection.

## 8. Impact

None measurable yet. Not released; no users, no deployments, no revenue by design (free forever). Observable only: working end-to-end app on the developer machine with zero fabrications on the eval set.

## 9. Documented instances of directing the AI (not accepting output)

- Refused to hand-judge gold passage lists ("I'm not a pastor"), forcing the evaluation methodology to change from plan section 6.2 (Jared approves every list) to index-derived, honestly labeled lists plus ablation recall.
- Cut evaluation scope (graded runs 20→10) and directed that exhaustive testing happen after v1.0.0, not during development.
- Rejected Claude's proposal to cap CPU threads to protect the dev machine; chose a RAM safety check instead.
- Overrode plan sections 6.4 and 7.1 (auto-select model by RAM; refuse to run below a floor) with an advisory-only hardware policy anchored to one reference machine.
- Required the app to show the full retrieved passage set, not only the passages the model cited, with matched Nave's topics and their verse lists exposed.
- Before sending the P2-prep prompt, stopped to ask which models would be querying the eval questions, and whether Claude Code's own answers would become the baseline — surfacing the risk of measuring small models against a big model instead of against the text.
- Scoped 1 Enoch, the Kybalion, and all non-biblical texts out of this app entirely, into a separately named future app, superseding an earlier decision to include 1 Enoch under an "Extended" canon.
- Corrected the display name to "The Pastor Bible" (with "The") after it had been recorded without it.
- Locked zero-cost, no-macOS, unsigned-first-then-SignPath, and in-place-upgrade rules ahead of any packaging work.
- After the P5 UI review, chose by-book canonical ordering as the passage panel default over Nave's topic grouping, and directed per-entry history delete with inline confirm while removing the sidebar "Clear history".
- P5.1 found that the delete button reported as done in P5 had never been written (all layers beneath it existed and were tested). This produced the project's first frontend tests. Attribution of who caught it — Jared's click-through or the P5.1 audit — is [UNKNOWN].
- Chose to hold the clean-machine install test (spare laptop with a 3050) until the v1 build is complete so the first test is the real one, and identified that the laptop's 4 GB GPU tests the exact "GPU present but too small" fallback branch.

## Discrepancies between plan and as-built (flag for anyone using this file)

- Plan 5.2 query rewrite by the chat model: removed; raw question used (rewrite measured to hurt recall).
- Plan 5.5 reranker: removed (lowered recall, +2 GB).
- Plan 3.2 sqlite-vec: rejected; float32 blobs with brute-force search.
- Plan 6.2 "~40 questions, Jared approves every gold list": as-built 10 graded questions with index-derived lists approved as drafted, unreviewed by a pastor. Memory holds two versions of the smoke split — "20 graded + 20 smoke" (P1) later "10 graded + 30 smoke" or "10 + 10" — the latest statement is 10 graded; the smoke count should be confirmed from data/eval/questions.json.
- Plan 6.4 / 7.1 auto model selection by RAM and hardware refusal: replaced by advisory-only policy, manual model choice.
- Plan 7.3 export "plain text": as-built offers .txt or .xlsx.
- Plan 5.4 Nave's subtopic headings for grouping: as-built groups by root topic name only (subtopic headings too long for UI); the topic switch itself is proposed for removal in P6.
- Plan 4.2 pericopes from USFM section headings: as-built uses WEB paragraphs, no headings.
- Plan 1 "installer auto-selects the smaller passing model": superseded by no auto selection.
- Plan 1 canon: an intermediate decision to include 1 Enoch (R.H. Charles 1917) in an "Extended" mode was superseded the same day; v1 is 66 books + Deuterocanon toggle only.
