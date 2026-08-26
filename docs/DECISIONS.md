# DECISIONS

Append-only. One line per decision, dated, with its reason. Never edit or delete a
past entry; if a decision is reversed, append the reversal with its own date and
reason. The reason matters as much as the decision: it is what tells a later
session whether the ground has shifted.

## Seeded from PLAN.md section 1 (approved plan, 2026-08-26)

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

## P0 session, 2026-08-26

- 2026-08-26 — Repo is private until v1.0.0 is released, then made public. Reason: nothing to show or support before a working release.
- 2026-08-26 — End-user install has zero manual prerequisites: the Windows installer bootstraps the WebView2 runtime if absent; first run checks RAM and disk (plan 7.1) and refuses cleanly if below floor; the user never installs a runtime, toolchain, or dependency by hand. Reason: non-technical users; plan section 0.
- 2026-08-26 — Windows installer runs per-user (Tauri NSIS installMode currentUser): no admin/UAC prompt, installs under the user's AppData. Trade-off: one user account per machine. Confirmed or reversed in P6. Reason: fewest clicks; works on machines where the user is not an administrator.
- 2026-08-26 — Product identifier io.github.haomuch1.pastorbible. Reason: fixed product id required for in-place upgrades (plan 7.5); sets app data path; permanent after v1.0.0.
- 2026-08-26 — Copyright holder is Jared; Claude credited as co-author in README, About, and NOTICE. Reason: Claude cannot hold copyright.
- 2026-08-26 — CODE_OF_CONDUCT.md is Contributor Covenant 2.1, licensed CC BY 4.0, shipped unmodified. Reason: Jared's call when told 3.0 is now current under CC BY-SA 4.0; 2.1 avoids a share-alike term and is the version most contributors expect.
- 2026-08-26 — The Contributor Covenant's "[INSERT CONTACT METHOD]" placeholder is left in place. Reason: shipping it unmodified was the instruction; a real reporting address must be filled in before the repo goes public at v1.0.0.
- 2026-08-26 — data/crisis_terms.txt is not created in P0, though plan section 12 lists it. Reason: an empty crisis-term list matches nothing, and plan 5.8 holds that under-triggering is unacceptable; the file is created with real content in P4, where the crisis matcher is built.
- 2026-08-26 — Frontend template is React + TypeScript + Vite via create-tauri-app; package manager npm. Reason: plan 3.4 and section 16 name React + TypeScript; npm ships with Node and adds no toolchain.
- 2026-08-26 — Git identity for this repo is set locally to Jared with his address, not globally. Reason: the repo records Jared as author; no machine-wide config is changed.
- 2026-08-26 — .gitattributes normalizes all text to LF in the repository and on checkout ("* text=auto eol=lf"). Reason: git's autocrlf was on, which would have checked docs/PLAN.md out with CRLF and broken its byte-identity with the approved plan; it also keeps Windows and Linux checkouts identical for the cross-platform build.
- 2026-08-26 — tauri-plugin-opener, serde and serde_json were removed from the scaffold. Reason: P0 requires only a window; the opener plugin grants a capability to launch external programs, which an offline app should not carry until something needs it.
- 2026-08-26 — Commit author email is the GitHub noreply address 293447797+haomuch1@users.noreply.github.com, set locally for this repo only. Reason: GitHub rejected the first push with GH007 rather than publish Jared's private address; noreply keeps that protection on and keeps his real address out of a repo that becomes public at v1.0.0.
- 2026-08-26 — Application icons remain the stock Tauri logo for now. Reason: icon design is an aesthetic choice and belongs to Jared; must be replaced before packaging in P6.

## P1 session, 2026-08-26

- 2026-08-26 — CODE_OF_CONDUCT.md contact method is "by opening an issue on the pastor-bible GitHub repository"; the preceding word "at" was dropped so the sentence is grammatical. Reason: the instructed replacement inserted verbatim produced "at by opening an issue"; the CoC is public-facing and must read correctly. Reverses the P0 entry that left the placeholder in place.
- 2026-08-26 — NOTICE.md now records the CoC as modified, with both the new and the upstream checksum. Reason: CC BY 4.0 requires modifications to be indicated, so editing the file obliged the notice to change with it.
- 2026-08-26 — 9.4 reworded to future tense until P8; revert at P8. Reason: plan text must be true at every commit.
- 2026-08-26 — docs/PLAN.md is no longer byte-identical to the plan as approved; its sha256 is now c2431f31134cd192d254ec137801d3d5c650e52aacf059fadc375f75b22cde48, was f07a7354683ad38a4be0219651a5b3fca23ed6ad534dbc1fed60fcff7ad57239. Reason: the 9.4 rewording above was applied to the plan as well so plan and README cannot drift; the plan is now a living document and future sessions must not treat its hash as fixed.
