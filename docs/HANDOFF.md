# HANDOFF

Session: P5.2 Sidebar delete, by-book default, spreadsheet export
Date: 2026-08-27
Status: P5.2 COMPLETE. Three things to re-check, listed below.
Next session is P6. Do not begin P7.

## State

Repository at D:\Haomuch-Programs\The-Pastor-Bible, branch main, pushed to
https://github.com/haomuch1/pastor-bible, still private.

Jared approved everything from P5.1 and asked for three things. All three are
done and all three were used in the running app, including the export, which was
written through the real Windows save dialog and then read back with two
independent readers.

The app runs with no environment variables:

    src-tauri\target\release\pastor-bible.exe

Tests: 66 in cargo, 2 more run explicitly, **6 in Vitest — the first frontend
tests this project has had** — and 134 in pytest. tsc and vite build clean.

## The delete finding

**The control had never been written.** Not hidden, not zero-sized, not
overlapped, not undiscoverable: absent. `historyDelete` was exported from
`src/api.ts` and called from nowhere — `grep -rn "historyDelete" src/` returned
the definition and nothing else — and a sidebar entry rendered one button
holding the question and its date, with no other element in it.

What made the false claim easy is that everything beneath the button was real
and tested: `UserDb::delete`, the `history_delete` Tauri command, its
registration, the frontend binding, and a passing cargo test named
`deleting_removes_exactly_one_and_search_forgets_it`. P5 put "Delete one" on
Jared's click-through checklist, which was an instruction to try rather than a
claim; P5.1's handoff then wrote "Per-entry delete is unchanged", and that was
written in a session where the app was being driven and could have been looked
at. DECISIONS carries a dated correction.

**A passing test on the command underneath is not evidence that a reader can
reach it.** This repository had no test that rendered a component at all until
today; it has six now, and the first of them fails if the delete control is
missing from the DOM.

## Jared's re-check

1. **Delete one question.** Every entry has a waste-basket at its right edge,
   always visible, titled "Delete this question". Press it: Delete and Cancel
   appear on that entry and on no other, and the main area does not move. Press
   Delete and that entry goes. "Clear history" has left the sidebar; Delete all
   history is in Settings, next to the export.
2. **The passage panel opens grouped by book.** Canonical order, chapter and
   verse order inside a book, cited passages marked where they fall rather than
   lifted to the top. "Group by topic" is still there as the second button and
   your choice is remembered. *The topic view is the thing I would still like
   you to decide about; see the flag below.*
3. **Export a spreadsheet.** Settings → Export history → Spreadsheet (.xlsx).
   One tab listing every question, then one tab per question with the answer at
   the top and every passage underneath: reference, cited, Deuterocanon, and the
   verse text. **There is one already written for you at
   `tools\history-check.xlsx`**, produced by the running app; open it in Excel
   and see whether the widths and the freeze work on your screen. *That file is
   the one thing here I could measure but not look at.*

## Verified this session, in the running app

**Deleting one entry.** Pressed the waste-basket on "What does the Bible say
about rest?", pressed Delete, and that row left the sidebar while the other
three stayed and the main area did not navigate. `user.db` afterwards holds
three rows and `history_fts` holds three, so the FTS index followed the delete
rather than keeping a ghost that search would still find. Screenshots 23 and 24.

**By book as the default.** A fresh question, "What does the Bible say about
rest?", verified, 1 minute 57 seconds, 317 passages found and 15 used. The panel
opened on "Group by book" with the stored setting cleared first, so the default
is what was exercised: Genesis, Exodus, Leviticus, Numbers, Deuteronomy, Joshua,
Judges, Ruth, 1 Samuel, in order, with Exodus 23:12, 34:21 and 35:1-3 in verse
order and marked "IN THE ANSWER" where they fall. Screenshot 25. Pressing "Group
by topic" wrote `group_by=topic` to user.db and pressing "Group by book" wrote
it back, so the choice is remembered.

**The export, through the real save dialog.** Settings → Export history offers
"Text file (.txt)", "Spreadsheet (.xlsx)" and Cancel before any dialog opens
(screenshot 27). The Windows save dialog was driven, a 36 KB workbook was
written, and it was then read back twice: once by the cargo test with calamine,
and once here by a reader built from the Python standard library. Five sheets
for four entries, the tabs named by number and question, the two questions
beginning "What does the Bible say abou" kept apart by their numbers, references
in full book names, Deuterocanon marked yes on all eight Tobit rows, and the
verse text matching index.db character for character.

**One real defect found by opening that file**: theme headings arrived in the
cells as "## Tithing as a Divine Commandment", because a cell renders no
markdown. The hashes now come off and the row is set in bold. Found by looking,
not by a test; the test now covers it.

**Closing.** No `llama-server` remained after the window closed.

## Not verified

- **The spreadsheet has never been opened in Excel.** It has been parsed by two
  readers and every value checked, but column widths, the frozen header and the
  wrapped text are laid out by Excel and nobody here can see them.
  `tools\history-check.xlsx` is waiting.
- **jsdom does no layout.** The six frontend tests prove the delete control is
  in the DOM, is reachable by the name it announces, and deletes exactly one
  entry; they cannot prove it is visible, because every rectangle in jsdom is
  zero. What stands behind "visible" is that the control's coordinates were
  clicked in the running window and the confirm appeared. A browser-based test
  would close that gap and is not worth its weight yet.
- **Everything visual is still Jared's to judge.** The palette, the type and the
  spacing are unchanged and remain placeholders. The waste-basket icon is drawn
  in the source rather than fetched, and its size and weight are a guess.
- **Nothing is signed, packaged or installed.** P6.

## Flags for Jared

**The topic view has not earned its place, and I have left it in.** By book is
now the default because Nave's roots are not a category system: the Tobit answer
grouped passages under "HAMATH" and "TOB-ADONIJAH", roots that merely happen to
contain a matching verse. The switch is still there and still shows that. If you
want it gone, it is one button and one branch.

**A cited passage is now marked where it falls.** In book order it is no longer
lifted to the top of its book. That is what canonical order is for, but it does
mean that in a book with eleven passages the cited one may not be the first
thing you see until you expand it.

**The frontend test runner is 84 new npm packages**, all development-only and
none shipped. NOTICE records the counts before and after. It is the price of
being able to test that a control exists, and this session is the argument for
paying it.

## Carried forward, unchanged

Answers still take two to three minutes on the reference machine, CPU only:
1 minute 57 seconds this session. Memory, the GPU path and the hardware floor
are as P5 measured them.

## Next session

P6 Packaging and CI, per PLAN sections 11 and 13.

Installers with the fixed product id, in-place upgrade, the uninstall prompt for
user data, sidecar bundling per Tauri's externalBin convention, WebView2
bootstrapping on a clean Windows 10, the offline gate, the upgrade gate, and the
release workflow.

Five things earlier sessions leave for it.

- **The app must be built with the Tauri CLI, not with `cargo build`.** A plain
  cargo build produces a binary that loads the dev URL and shows "localhost
  refused to connect". CI must call the CLI.
- **index.db is not a declared bundle resource.** The embedding model is, and it
  works; index.db is a gigabyte and still resolves through the fallback chain in
  `resolve_paths`. P6 decides whether it is declared, shipped separately, or
  fetched.
- **The Vulkan sidecar** is fetched and measured but not bundled; the Compute
  setting is in the window, disabled, waiting for it.
- **`tools/fetch_llama.py --sidecar`** places the CPU build in
  `src-tauri/binaries` under the target-triple name Tauri wants, and that layout
  has not been tested against an actual bundle. `tools/fetch_model.py` is the
  same idea for the embedding model and has been.
- **The transitive licence audit** NOTICE defers to P6 now covers five more Rust
  crates for the spreadsheet writer, all permissive and all pure Rust, and a
  development-only frontend test runner that reaches no end user.

Read PLAN.md, DECISIONS.md, API.md, SIDECAR.md and this file before starting.
Do not begin P7.

## Running it without an installer

    src-tauri\target\release\pastor-bible.exe

The embedding model is beside the binary in `target\release\resources\`, put
there by `npx tauri build`; the chat model is in
`%APPDATA%\io.github.haomuch1.pastorbible\models\`, where the first-run download
puts it, and a development build will also accept the repository's `models\`
folder. To start over, close the app and delete
`%APPDATA%\io.github.haomuch1.pastorbible`.

If the repository has just been cloned, `python tools/fetch_model.py` places the
embedding model in `src-tauri/resources/` and checks it against its pinned
sha256; `npx tauri build` will refuse to build without it.

`npm test` runs the frontend tests; `cargo test --manifest-path
src-tauri/core/Cargo.toml` runs the rest.
