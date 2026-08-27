# HANDOFF

Session: P5.1 Jared's first-look fixes
Date: 2026-08-27
Status: P5.1 COMPLETE. Five things to re-check, listed below. Do not begin P6
until they are checked.

## State

Repository at D:\Haomuch-Programs\The-Pastor-Bible, branch main, pushed to
https://github.com/haomuch1/pastor-bible, still private.

Jared ran the app for the first time on 2026-08-27 and found five things. All
five are fixed and every one of them was checked in the running app rather than
only in a test.

**The app now runs with no environment variables at all.** That was the defect
it opened with, and it is the change with the widest reach: the embedding model
is a resource of the application rather than a file in the application data
directory, and `npx tauri build` copies it beside the binary exactly as the
installer will ship it. `set TPB_MODEL_DIR=...` is no longer needed to run
`src-tauri\target\release\pastor-bible.exe`, and `npm run tauri dev` works on a
machine that has the repository's `models\` folder.

Tests: 60 in cargo, 2 more that are run explicitly, 134 in pytest, all passing.
tsc and vite build clean.

## The cause of the model-path defect

`src-tauri/src/lib.rs` resolved the model directory as `TPB_MODEL_DIR` if set,
and otherwise `%APPDATA%\io.github.haomuch1.pastorbible\models`, then built both
model paths by joining a file name onto it. That directory has never existed on
this machine and nothing in the program ever creates it. The chat model would be
downloaded there by a first-run screen, which `first_run_done = 1` had already
caused the app to skip; the embedding model is not downloaded at all, because
DECISIONS 2026-08-26 says it ships with the installer. Every P5 run launched the
app the way this file used to instruct, with `TPB_MODEL_DIR` pointing at the
repository's `models\`, which is why the self-test passed in the real app.
Jared ran `npm run tauri dev` with no such variable, the fallback took over, and
the first model the pipeline loads produced

    cannot read model ...\models\nomic-embed-text-v1.5-f16.gguf:
    The system cannot find the path specified (os error 3)

two and a half minutes into a question. Nothing was copied or moved during P5;
the file was never in application data and the app never put it there.

The defect was invisible to every test and every measurement because the
environment variable was in the instructions rather than in the program. That is
the shape of it worth remembering.

## Jared's re-check list

Five things, in the order they will come up. Run
`src-tauri\target\release\pastor-bible.exe` with **no** environment variables, or
`npm run tauri dev`.

1. **Ask works.** Type a question and press Ctrl+Enter. It must answer, not
   report a path. If a model file really is missing you get a paragraph naming
   the file and saying what puts it back, before the wait rather than after it
   (screenshot 22).
2. **Full book names.** Every citation chip, passage heading, verse number,
   by-book group label and reading-view title says "1 Kings", "2 Chronicles",
   "Psalms". No "1Ki" anywhere. *You chose these forms on 2026-08-27; if any of
   the eighty-one reads wrong to you, they are one table in
   `src-tauri/core/src/index.rs`.*
3. **New question.** Top of the main area and top of the sidebar, both always
   visible. Open a past answer, then come back with it.
4. **Clear history.** Foot of the sidebar, with a confirmation. It appears only
   when there is history to clear. Per-entry delete is unchanged.
5. **Read chapter.** On every passage in the panel. It opens the whole chapter
   with your cited verses marked, previous and next chapters, and Close puts you
   back exactly where you were. *This is the new screen; it is the one most
   worth your eyes.*

## Verified this session, in the running app

**The tithing question.** "What does the Bible say about tithing while
unemployed?" in 66-book mode: verified, verdict `ok`, 170.0 seconds, 341
passages found and 25 sent, 35 cited verses, every `[P#]` rendered as a chip
with a full book name. Screenshot 09.

**The Deuterocanon, at last.** "What does Tobit say about giving to the poor?"
with the Deuterocanon on: verified, verdict `ok`, 142.4 seconds, 376 passages
found and 33 sent, 11 used in the answer.
The answer cited Tobit 4:7 and Tobit 4:16, both chips carry "· Deuterocanon",
both passages in the panel carry the dashed tag, and the footer line appears
under the answer. **Screenshot 18 is the screen no screenshot showed at the end
of P5.** Screenshot 20 is the same passage opened in its chapter, with the tag
on the reading view.

**Citation chips, fresh and reopened.** On the fresh answer a chip scrolled to
and outlined its passage. The answer was then reopened from history and a chip
did the same thing (screenshots 16 and 15). No entry now shows the "saved
before..." notice, because the entries that could are gone.

**Read chapter.** Leviticus 27 opened from the Leviticus 27:30-33 passage:
whole chapter, verses 30 to 33 marked and scrolled to, "← Leviticus 26" and
"Numbers 1 →" at the foot. Close returned to a screen identical to the one
before it opened, scroll position included (screenshot 19).

**The plain message.** With the bundled model hidden, the app opened, said which
file was missing and where it was expected, and `ask` refused with the same
paragraph instead of `os error 3` (screenshot 22). Restoring the file and
rebuilding returned it to normal. `npx tauri build` itself refuses if the
declared resource is not there, which is a second net under the same hole.

**Closing.** No `llama-server` process remained after the window closed, checked
after each of the three runs.

**The nine stale history entries** were deleted from this machine's user.db on
2026-08-27, and schema 2 deletes any that a future user.db still carries.

## Not verified

- **Everything visual is still Jared's to judge**, as at the end of P5, and this
  session changed only behaviour and wording he asked for. The palette, the
  type, the spacing and the wording of every label are unchanged and remain
  placeholders.
- **The history export was tested, not seen.** `export_text` now resolves each
  `[P#]` into its reference and lists the passages; a cargo test asserts both,
  including that no abbreviation survives. Writing one from the running app
  needs a file dialog, which cannot be driven from here. Settings > Export to a
  text file, and read it.
- **The reading view has been used at two window sizes and by one person who
  cannot see it.** Keyboard: Escape closes, the left and right arrows turn the
  page. Neither has been tried by hand.
- **Nothing is signed, packaged or installed.** P6.
- **The bundled resource has never been through an actual installer.** The
  declaration is in `tauri.conf.json` and `tauri-build` copies it in a
  development build, which is what was tested. index.db is *not* declared and
  still resolves through the fallbacks; P6 owns both.

## Carried forward from P5, unchanged

**The topic grouping is still the weakest thing here, and it is still a data
problem.** This session's screenshots show it again: the Tobit answer grouped
passages under "TOB-ADONIJAH" and "HAMATH", which are Nave's root topics that
happen to contain a matching verse. Screenshots 10 and 11 are the same answer by
topic and by book. It is still worth deciding whether the grouping earns its
place or whether by-book should be the default.

**Answers still take two and a half minutes.** 170.0 and 142.4 seconds this
session, on the reference machine, CPU only.

**Memory, the GPU path, and the hardware floor** are as P5 measured them.

## Next session

P6 Packaging and CI, per PLAN sections 11 and 13.

Installers with the fixed product id, in-place upgrade, the uninstall prompt for
user data, sidecar bundling per Tauri's externalBin convention, WebView2
bootstrapping on a clean Windows 10, the offline gate, the upgrade gate, and the
release workflow.

Four things P5.1 leaves for it.

- **The app must be built with the Tauri CLI, not with `cargo build`.** A plain
  cargo build produces a binary that loads the dev URL and shows "localhost
  refused to connect". CI must call the CLI.
- **index.db is not a declared bundle resource.** The embedding model now is,
  and it works; index.db is a gigabyte and still resolves through the fallback
  chain in `resolve_paths`. P6 decides whether it is declared, shipped
  separately, or fetched.
- **The Vulkan sidecar** is fetched and measured but not bundled; the Compute
  setting is in the window, disabled, waiting for it.
- **`tools/fetch_llama.py --sidecar`** places the CPU build in
  `src-tauri/binaries` under the target-triple name Tauri wants, and that layout
  has not been tested against an actual bundle. `tools/fetch_model.py` is the
  same idea for the embedding model and has been.

Read PLAN.md, DECISIONS.md, API.md, SIDECAR.md and this file before starting.
Do not begin P7.

## Running it without an installer

    src-tauri\target\release\pastor-bible.exe

That is the whole command. The embedding model is beside the binary in
`target\release\resources\`, put there by `npx tauri build`; the chat model is
in `%APPDATA%\io.github.haomuch1.pastorbible\models\`, where the first-run
download puts it, and a development build will also accept the repository's
`models\` folder. To start over, close the app and delete
`%APPDATA%\io.github.haomuch1.pastorbible`.

If the repository has just been cloned, `python tools/fetch_model.py` places the
embedding model in `src-tauri/resources/` and checks it against its pinned
sha256; `npx tauri build` will refuse to build without it.
