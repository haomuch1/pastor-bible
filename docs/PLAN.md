# THE PASTOR BIBLE — PROJECT PLAN v1

Date: 2026-08-26
Authors: Jared (direction, orchestration, judgment) and Claude (architecture, code via Claude Code)
Status: APPROVED PLAN. No code is written until Jared approves this document.
Items marked VERIFY are not facts until the named phase confirms them.

---

## 0. Purpose

A free, public, fully offline desktop app. A user types a question about life or scripture and receives a cited synopsis of every place the Bible addresses it, grouped by theme, with the verses themselves shown from the text. Built so a non-technical person can install and use it. Built so a pastor can trust it: zero fabricated references, ever.

Stance: scripture-first, strictly nondenominational. The app reports what the text says and where. It does not take sides between traditions.

---

## 1. Locked decisions (dated, one-line reason each)

- 2026-08-26 — Display name "The Pastor Bible". Repo slug "pastor-bible" on Jared's personal GitHub. No org: not a business, never sold.
- 2026-08-26 — Zero cost, forever. No fees of any kind. Every dependency is free and open source.
- 2026-08-26 — Repo license Apache-2.0. Permissive, OSI-approved, explicit patent grant, formal NOTICE file for source attribution.
- 2026-08-26 — RAG over the actual text, not fine-tuning. Fine-tuning yields confident fake references; retrieval yields checkable ones.
- 2026-08-26 — Text: World English Bible, Classic edition (US spelling, "Yahweh"). Public domain worldwide; no Crown-copyright quirk. Trademark condition: we ship an unmodified faithful copy, so we may call it the World English Bible.
- 2026-08-26 — Canon: 66 books always on. One toggle, off by default, adds the WEB Deuterocanon/Apocrypha. Every Deuterocanon citation is labeled in output. 1 Enoch, Kybalion, and all non-biblical texts are OUT of this app (section 15).
- 2026-08-26 — Index corpora: Treasury of Scripture Knowledge (TSK) cross-refs and Nave's Topical Bible. Public domain. No commentary of any kind: commentary is one tradition's interpretation.
- 2026-08-26 — Sources are structured text only (USFM/JSON/plain text with verse delimiters). No PDFs: extraction breaks verse boundaries.
- 2026-08-26 — Internet is used exactly twice: downloading the installer from GitHub, and downloading the model file on first run. After that the app makes no outbound connection, and a test proves it.
- 2026-08-26 — Model server: llama.cpp (MIT), bundled as a Tauri sidecar. Not Ollama, which installs a separate always-on service with its own tray icon and updater.
- 2026-08-26 — Models (chat, embedding, reranker): Apache-2.0 or MIT only. No Llama community license, no Gemma terms, no acceptable-use policy flowing to users.
- 2026-08-26 — Shell: Tauri 2 (MIT/Apache). Real installer, icon, owns the model server's lifecycle, cross-platform by design.
- 2026-08-26 — Targets: Windows and Linux. macOS dropped (notarization requires a paid Apple account). Mac users may build from source.
- 2026-08-26 — Signing: ship unsigned first (SignPath requires an existing release). Apply to SignPath Foundation after the first public release. README states up front that Windows will show a SmartScreen warning and shows exactly how to proceed.
- 2026-08-26 — Upgrades are manual re-downloads, and the installer upgrades in place: it detects any previous version and replaces it. Never two versions on one machine. User data (history, settings, downloaded models) lives outside the install directory and survives every upgrade; only program and index are replaced.
- 2026-08-26 — Model size: default = one size up from the smallest model that passes evaluation. Installer auto-selects the smaller passing model on machines below the RAM threshold.
- 2026-08-26 — Zero fabricated references, enforced mechanically (5.6), not by prompt alone.
- 2026-08-26 — Question history stored locally; searchable, deletable, exportable. Never leaves the machine.
- 2026-08-26 — Prebuilt index ships inside the installer. The user's machine never parses sources or builds embeddings.
- 2026-08-26 — README opens with a study/educational disclaimer and a crisis note covering harm to self or others. The crisis note also appears in-app above (never instead of) an answer when crisis language is detected.
- 2026-08-26 — Credit: Jared and Claude, both, in README and About screen.
- 2026-08-26 — Workflow: this plan lives in project instructions. Claude Code executes one phase per session (section 13). Jared directs.

---

## 2. Out of scope for v1

- Any Bible version other than WEB Classic.
- Commentary, devotionals, sermons, interpretive material.
- 1 Enoch, Kybalion, any non-biblical text.
- macOS builds and notarization.
- Auto-update (no internet after install; updates are a manual re-download).
- Telemetry, analytics, crash reporting, accounts, sync, mobile.

---

## 3. Architecture

### 3.1 Process model

    The Pastor Bible.exe (Tauri 2: Rust backend, TypeScript frontend)
      spawns on launch -> llama-server sidecar (llama.cpp), bound to
                          127.0.0.1 on a random free port, killed on exit
      opens            -> index.db  (bundled, read-only, install directory)
      opens            -> user.db   (history + settings, app data directory)
      reads            -> model .gguf files (app data directory)

Nothing else listens or connects. The frontend talks only to the Rust backend; the Rust backend talks only to the local sidecar.

### 3.2 Storage: two SQLite files, versioned schemas

**index.db** (bundled with the installer, replaced on upgrade, never written at run time)

    books             book_id, name, abbrev, testament, canon (protestant|deutero), order
    verses            verse_id, book_id, chapter, verse, text, pericope_id
    pericopes         pericope_id, book_id, start_verse_id, end_verse_id, heading
    verse_fts         FTS5 full-text index over verses.text
    embeddings        verse-level and pericope-level vectors
                      (VERIFY store: sqlite-vec is the candidate; confirm Windows
                      build and maturity in Phase 2; fallback is a flat binary
                      vector file loaded in Rust)
    tsk_refs          from_verse_id, to_verse_id
    nave_topics       topic_id, heading, parent_topic_id; nave_topic_verses join
    topic_embeddings  vectors over Nave's headings
    meta              index_version, schema_version, build_checksum

**user.db** (app data directory, survives upgrades and reinstalls)

    history           id, asked_at, question, canon_mode, answer_md,
                      passage_ids (json), model_id, index_version
    history_fts       FTS5 over question + answer
    settings          key, value
    meta              schema_version

### 3.3 Network policy

The downloader is the only module with network capability, granted via a scoped Tauri capability used solely by the first-run flow. No other module imports an HTTP client. CI runs the full query test suite with networking disabled and asserts pass (section 11).

### 3.4 Frontend stack

Vite + TypeScript + React (Tauri's default templates). Plain, readable, large type; no design system beyond what's needed. All visual/aesthetic choices are flagged for Jared's eyes before being called done.

---

## 4. Data pipeline (build-time, run by us, never on the user's machine)

### 4.1 Sources

VERIFY exact URLs, formats, and license text in Phase 1; record each in NOTICE.md with retrieval date and checksum.

- WEB Classic USA, full ecumenical set, from eBible.org, USFM preferred.
- TSK cross-reference dataset, public domain.
- Nave's Topical Bible dataset, public domain.

Vendored into repo under data/sources/ unchanged. No PDFs.

### 4.2 Parse

USFM -> verses table. Book codes map to canon flag. Deuterocanon books (Tobit, Judith, Greek Esther, Wisdom, Sirach, Baruch incl. Letter of Jeremiah as ch. 6, Daniel (Greek) incl. Song of the Three / Susanna / Bel, 1–2 Maccabees, plus the Orthodox set: 1–2 Esdras, Prayer of Manasseh, Psalm 151, 3–4 Maccabees; VERIFY exact file set in Phase 1) are flagged canon=deutero. Pericopes from USFM section headings where present, otherwise paragraph breaks. Every verse count is re-derived from the parsed rows and checked against known totals (31,102 for the 66; VERIFY against the parsed WEB, which may differ by versification).

### 4.3 Index build

1. Verse embeddings: each verse, prefixed with a context label "Book Chapter:Verse — pericope heading".
2. Pericope embeddings: each pericope as a unit, same labeling.
3. FTS5 over verse text (keyword path).
4. Nave's topic headings embedded as their own entry points.
5. TSK edges loaded as a graph table.

Output: index.db with meta.index_version and a checksum. Committed as a release asset and bundled into the installer. Deterministic: same sources + same embedding model = same checksum.

---

## 5. Query pipeline (run-time)

**5.1 Canon filter.** Active canon mode gates every retrieval step. Deutero rows are invisible when the toggle is off.

**5.2 Query rewrite.** The chat model produces 3–5 search queries in biblical vocabulary (e.g. "anxiety" -> "anxious, worry, care, troubled, fear").

**5.3 Hybrid retrieval.** Vector search (verse + pericope) and FTS5 keyword search for each rewritten query; results fused by reciprocal rank fusion. Wide net: ~100 candidates.

**5.4 Topic and cross-ref expansion.** Nave's topic hits contribute their full verse lists; TSK expands one hop from the top candidates. Expansion is capped and tagged by origin (direct / topic / cross-ref).

**5.5 Rerank.** A small Apache/MIT reranker orders the candidate set; top ~25 passages go to generation. Cutoff tuned in Phase 3 against the eval set.

**5.6 Generation with mechanical citation guarantee.**

- Passages are sent with opaque IDs ([P1]..[Pn]). The prompt permits citing only by these IDs and forbids naming any reference not in the set.
- The model returns a themed synopsis: theme headings, each with a short synthesis and the IDs it draws on.
- Verifier: every [P#] must exist in the sent set; every free-text reference pattern (e.g. "John 3:16") in the prose must resolve to a verse inside the sent set. Any failure -> the offending token is stripped and generation retried once with the failure named. A second failure -> the app shows the retrieved passages grouped by book with a one-line note that a synthesis could not be produced. The user never sees an unverified reference.
- The passage panel renders verse text from index.db, never from the model output.

**5.7 Deuterocanon labeling.** Any passage with canon=deutero is rendered with a visible "Deuterocanon" tag in both the synopsis and the panel, and the answer carries a one-line footer noting Deuterocanon passages were included.

**5.8 Crisis handling.** A maintained phrase list (data/crisis_terms.txt), covering harm to self and harm to others, is matched against the question. On match, the static crisis note (9.3) is shown above the answer. The answer still runs. Over-triggering is acceptable; under-triggering is not.

---

## 6. Model selection and evaluation

**6.1 Candidates.** Chosen in Phase 3 from models available at that time under Apache-2.0 or MIT, in roughly 2B / 4B / 8B classes, quantized for CPU. No model is named in this plan; the landscape moves too fast.

**6.2 Evaluation set** (data/eval/questions.json). ~40 questions a pastor would ask, spanning everyday-life topics and doctrinal-neutral study topics. Claude drafts candidate gold passage lists from the indexes; Jared reviews and approves every gold list. Gold lists are judgment; they are not auto-generated and not delegated.

**6.3 Metrics.**

- Retrieval recall@25 against gold passages (index quality).
- Fabricated-reference count across all runs: must be 0 (hard gate).
- Citation precision: fraction of cited passages judged relevant.
- Answer quality: Jared rates a fixed subset on a 1–5 rubric.
- Latency and peak RAM on CPU, per model size.

**6.4 Selection rule.** Smallest size passing the gates (recall threshold set in Phase 3, zero fabrications, quality floor) is the fallback model; one size up is the default. Both are offered by the installer; choice is automatic from detected RAM, overridable in settings.

**6.5 Hardware floor.** Stated in README from measured peak RAM and disk of the default model, with the fallback model's numbers listed as minimum. Numbers are measured, never estimated.

---

## 7. First-run, UX, and upgrades

### 7.1 First run

1. Welcome screen with the disclaimer (9.2) and the credits.
2. Hardware check: RAM and free disk; refuse cleanly with a plain message if below the fallback model's floor.
3. Model download: progress bar, size and time estimate, resumable, checksum-verified, one plain sentence: "This is the only time The Pastor Bible needs the internet."
4. Self-test: three canned questions run end to end; a green check.
5. Open the main screen.

### 7.2 Main screen

Question box; answer area (themed synopsis); passage panel (verse text, references, origin tags, Deuterocanon tags); history sidebar with search.

### 7.3 Settings

Canon (Protestant 66 / include Deuterocanon), Model (default / smaller), Delete all history, Export history (plain text), About (credits, licenses, index version, app version, offline statement).

### 7.4 Close

Sidecar terminated on window close. No background process remains. Reopen from the desktop icon.

### 7.5 Upgrades, reinstall, uninstall

- Install directory holds only the program, sidecar, and index.db. App data directory (per-OS standard location) holds user.db and model files.
- Installer uses a fixed product identifier so the OS treats a newer installer as an upgrade of the same app, not a second app. On Windows the installer removes the previous install before writing the new one; on Linux the package manager replaces the package. VERIFY exact Tauri/NSIS upgrade semantics in Phase 6; if in-place replacement is not automatic, the installer script explicitly uninstalls the prior version first.
- Downgrade (older installer over newer) is refused with a plain message.
- First launch after an upgrade: index.db is the new bundled copy; user.db is migrated by schema version; model files are kept and re-verified by checksum, never re-downloaded unless the pinned model changed.
- Uninstall removes the program and index.db. It asks, in plain words, whether to also delete question history and downloaded models; default is keep.
- Test in Phase 6: install v1.0.0, ask three questions, install v1.0.1 over it, confirm one entry in Add/Remove Programs, history intact, no model re-download, new index version shown in About.

---

## 8. Question history

Every answered question is stored in user.db with its answer, canon mode, model id, index version, and the exact passage IDs used, so a later view shows precisely what the answer rested on. History is searchable (FTS5 over question + answer), individually deletable, wholesale deletable, and exportable to a single text file. No history is ever transmitted.

---

## 9. README skeleton and wording

### 9.1 Section order

1. Title and one-line description
2. Disclaimer (9.2)
3. Crisis note (9.3)
4. What it is / what it is not (stance statement, 9.5)
5. Install: Windows (with SmartScreen walkthrough, 9.4), Linux; upgrading (download the new installer, run it, done)
6. First run: what to expect, one-time download, sizes
7. Hardware requirements (measured)
8. Using it; the Deuterocanon toggle explained neutrally
9. How answers are produced and why they can be trusted (5.6 in plain language)
10. Privacy: nothing leaves your computer
11. Building from source (incl. macOS note)
12. Sources and credits (WEB, TSK, Nave's, llama.cpp, Tauri, model)
13. License
14. Authors: Jared and Claude

### 9.2 Disclaimer (verbatim, top of README and first-run screen)

> The Pastor Bible is a study and educational tool. It searches the text of the Bible and summarizes what it finds, with citations. It is not a pastor, a counselor, or an authority, and its answers are not the final word on anything. Read the verses it cites for yourself. If you are struggling, please reach out to a real person.

### 9.3 Crisis note (verbatim, README and in-app)

> If you are in crisis, or thinking about harming yourself or someone else, please reach out to a real person right now. In the United States, call or text 988. In other countries, call your local emergency number. Talk to a pastor, a counselor, or someone you trust. This app is a study tool and cannot help you the way a person can.

### 9.4 Windows warning text (README, install section)

> Because this is a free project made by volunteers, the installer is not yet signed with a paid certificate. When you run it, Windows will show a blue screen that says "Windows protected your PC." This is expected. Click "More info," then "Run anyway." (Screenshots follow.) We will apply for free open-source signing after the first release; when granted, the publisher shown will be "SignPath Foundation."

### 9.5 Stance statement (README section 4)

> The Pastor Bible is nondenominational. It uses one public-domain translation and reports what the text says and where. It includes no commentary and takes no position on questions where Christian traditions differ. The optional Deuterocanon setting is provided because some traditions include those books and others do not; it is off by default and every passage from it is labeled.

---

## 10. Licensing and attribution

- Repo license: Apache-2.0. LICENSE at root; NOTICE.md as required by the license, listing every source with URL, license, retrieval date, and checksum.
- NOTICE.md covers: WEB (public domain; trademark note; unmodified copy), TSK, Nave's, llama.cpp (MIT), Tauri (MIT/Apache-2.0), chat/embedding/reranker models (Apache-2.0 or MIT, named at Phase 3), any Rust/JS dependencies with attribution requirements.
- End users incur no obligations. All attribution obligations are met by the repo itself.

---

## 11. Release pipeline

- GitHub Actions on tag: build Windows installer (VERIFY NSIS .exe vs MSI choice in Phase 6; default NSIS) and Linux AppImage + .deb. Sidecar binaries per target triple per Tauri's externalBin convention.
- Installer carries a fixed product identifier and a version; the release workflow fails if the version does not increase.
- WebView2 on Windows: VERIFY bootstrapper behavior in Phase 6 so a fresh Windows 10 machine works without manual steps.
- index.db bundled as a resource; checksum published in release notes alongside installer checksums.
- Offline gate: CI job runs the query test suite with networking disabled; release blocked on failure.
- Upgrade gate: CI installs the previous release, then the candidate, and asserts single-install, history preserved, no model re-download.
- Signing step present in CI behind a flag, disabled until SignPath approval; when approved, the flag routes artifacts through SignPath's trusted-build integration.
- Model files are NOT release assets (size); the downloader fetches them from the model's own permissively licensed host, with checksum pinned in the app. VERIFY host stability and size limits in Phase 5.

---

## 12. Repo layout

    pastor-bible/
      README.md, LICENSE (Apache-2.0), NOTICE.md, CODE_OF_CONDUCT.md
      docs/
        PLAN.md           (this document)
        DECISIONS.md      (dated one-line entries, append-only)
        HANDOFF.md        (current state, written before every session end)
        EVAL.md           (eval protocol and results by model)
      data/
        sources/          (vendored WEB, TSK, Nave's; unmodified)
        eval/             (questions.json with approved gold lists)
        crisis_terms.txt
      pipeline/           (Python: parse, index build; runs only for us)
      src-tauri/          (Rust backend, sidecar launch, DB access, verifier)
        binaries/         (llama-server per target triple)
        resources/        (index.db)
      src/                (TypeScript/React frontend)
      tests/              (retrieval, verifier, offline, upgrade suites)
      .github/workflows/  (build, offline gate, upgrade gate, release)

---

## 13. Phase sequence for Claude Code (one phase per session)

- **P0 Scaffold.** Repo, Apache-2.0 license, docs skeleton, README with 9.2–9.5 wording, DECISIONS.md seeded from section 1, empty Tauri 2 app that opens a window. Deliverable: window opens on Windows.
- **P1 Ingestion.** Source acquisition and license verification (VERIFY items in 4.1–4.2), USFM parse, verses/books/pericopes tables, count checks, TSK and Nave's loaded. Deliverable: index.db without embeddings; counts reported and re-derived.
- **P2 Index and retrieval harness.** Embedding model shortlist (2–3, permissive license), verse/pericope/topic embeddings, FTS5, hybrid fusion, TSK expansion, reranker; retrieval-only eval against the approved gold set. Deliverable: recall@25 by configuration. Gold lists are approved by Jared before P2 starts.
- **P3 Model evaluation.** llama.cpp local runs of 2B/4B/8B-class candidates; full pipeline incl. verifier; metrics in EVAL.md; selection per 6.4; hardware numbers measured. Deliverable: default and fallback models named with evidence.
- **P4 Generation and verifier in the Rust backend.** Sidecar lifecycle, prompt, themed synopsis, citation verifier with retry/fallback, crisis matcher, Deuterocanon labeling. Deliverable: end-to-end answers from a CLI harness with zero fabrications on the eval set.
- **P5 Frontend and first-run.** Screens per section 7, history per section 8, downloader with checksum and resume, self-test, user.db in app data directory. Deliverable: working app from a fresh state; UI screenshots flagged for Jared's review.
- **P6 Packaging and CI.** Installers with fixed product id, in-place upgrade, uninstall prompt, sidecar bundling, WebView2 handling, offline gate, upgrade gate, release workflow. Deliverable: unsigned Windows and Linux installers from a tagged build, upgrade test passed.
- **P7 Fresh-machine verification.** Jared installs on a clean Windows machine, follows only the README, confirms offline operation, then installs a second build over it and confirms single install with history intact. Deliverable: v1.0.0 public release.
- **P8 SignPath.** Application; enable signing flag on approval. Deliverable: signed v1.0.1.

---

## 14. Standing rules for every Claude Code session

- Stop-clause: halt and report if instructions conflict with observed reality, with a locked decision, or raise a concern. Do not improvise around a conflict.
- One phase per session. Do not start the next phase.
- Read docs/PLAN.md, docs/DECISIONS.md, docs/HANDOFF.md first. Targeted reads over broad exploration. Quiet tool output.
- Documentation before implementation: update DECISIONS.md and the relevant doc section, then write code.
- Every decision made in-session is logged in DECISIONS.md, dated, with a one-line reason.
- Re-derive every count and total from the data; never carry a figure over.
- Verify against the produced artifact (the db, the installer), not the script that produced it. State plainly what could not be verified.
- Clean stop: never stop mid-edit; finish or revert. Write HANDOFF.md before context runs out.
- Risky changes ship small, non-destructive, flag-gated, defaulting to current behavior. On failure, revert to known-good.
- No secrets exist in this project; if one ever appears, it is never printed and Jared is told to revoke it.
- Aesthetic and theological judgments are Jared's. Flag, do not decide.

---

## 15. Future: second app

After The Pastor Bible v1.0.0 is functional and public, a separately named app for public-domain religious and esoteric texts (1 Enoch, Kybalion, and others Jared chooses) is started in its own repo, reusing this architecture. Nothing from that project enters The Pastor Bible.

---

## 16. Assumptions to override, and VERIFY list

**Assumptions** (say so and they change):

- Repo slug "pastor-bible"; display name "The Pastor Bible".
- Frontend React + TypeScript.
- Windows installer NSIS .exe.
- English-only UI.

**VERIFY before use** (owning phase in brackets):

- WEB Classic USA source URL, format, and exact Deuterocanon file set [P1]
- TSK and Nave's dataset sources and public-domain statements [P1]
- Verse totals for parsed WEB [P1]
- sqlite-vec maturity and Windows build; fallback vector store [P2]
- Embedding, reranker, and chat model candidates and licenses [P2, P3]
- llama-server binary name, flags, and OpenAI-compatible endpoint [P4]
- Model download host, size, checksum [P5]
- NSIS vs MSI; WebView2 bootstrapping on clean Windows 10 [P6]
- Tauri 2 NSIS/.deb in-place upgrade behavior and product-id handling [P6]
- Uninstaller prompt for user data on Windows and Linux [P6]
- SignPath current requirements at time of application [P8]
