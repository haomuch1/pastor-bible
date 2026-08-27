# HANDOFF

Session: P5 Frontend, first run, history
Date: 2026-08-26
Status: P5 COMPLETE, with everything visual awaiting Jared's eyes

## State

Repository at D:\Haomuch-Programs\The-Pastor-Bible, branch main, pushed to
https://github.com/haomuch1/pastor-bible, still private.

The app runs. `npx tauri build --no-bundle` produces
`src-tauri/target/release/pastor-bible.exe`, which opens a window, asks a
question, shows every passage it found while the answer is being written, and
keeps the answer in a history file that never leaves the machine.

New in the backend (`src-tauri/core`): `userdb.rs` is user.db, history and
settings with FTS5 and an export; `download.rs` is the model downloader, the
only code here that reaches the internet; `hardware.rs` reads this machine for
the advisory first-run check; `session.rs` is one open app, two loaded sidecars,
staged progress and cancellation. `src-tauri/src/lib.rs` is the Tauri shell:
twenty commands, two events, and a window-close handler that stops both models.

New in the frontend (`src/`): `types.ts` and `api.ts` are the whole surface the
window has; `screens/FirstRun.tsx`, `screens/Main.tsx`, `screens/Settings.tsx`,
`components/Synopsis.tsx`, `components/PassagePanel.tsx`, `styles.css`. There is
no browser storage anywhere: `grep -rn "localStorage\|sessionStorage\|indexedDB"
src/` returns nothing, and every setting and every answer goes through user.db
by way of a command.

docs/screenshots/ holds seventeen PNGs of the app as it stands, captured by
tools/shot.ps1 and read back as images. One screen is missing from them: an
answer that cites a deuterocanonical passage, with the dashed "Deuterocanon"
tag on it. The both-canon question that was asked sent eight deuterocanonical
verses and the model cited none of them, so the tag never appeared; the tag is
implemented and the passage panel applies it from `canon`, but nobody has seen
it. Step 9 of the checklist below is how to see it.

Tests: 46 in cargo, 2 more that are run explicitly, and 134 in pytest, all
passing.

## Not verified: everything visual

**Claude Code cannot see the window.** The screenshots were captured by a script
and read back as images, so the layout, the wording and the colours have been
looked at, but nobody has used the app. Everything in the list below is Jared's
to judge, and every visual choice in it is a placeholder until he says
otherwise:

- whether the type is large enough and the spacing generous enough
- the colour palette, which is a warm off-white with a brown accent
- the wording of every label and every note
- whether the topic grouping is worth having at all (see the flags)
- the window's title bar follows the system theme and is dark while the app is
  light; nothing was done about it
- keyboard use beyond Ctrl+Enter, tab order, and anything to do with screen
  readers: none of it has been tested
- the app has only ever been run on this machine, at one window size, by a
  script

## Verified

**user.db.** Created on first run at
`%APPDATA%\io.github.haomuch1.pastorbible\user.db`, schema version 1, with a
migration hook and a refusal to open a file written by a newer version. History
with FTS5 over question and answer, settings, and a plain-text export. 14 tests:
round trip, the passages re-rendered from the index installed now, a note when
the index version differs, paging, search including hostile input a reader might
type, delete one, clear all, export contents, and the two token cases below.

**The model host, PLAN section 16's last VERIFY item.** huggingface.co, URL form
`https://huggingface.co/<org>/<repo>/resolve/main/<file>`, no token, Range
honoured. All three files' sizes and checksums match the host and the copies on
this machine. Recorded in docs/SIDECAR.md and NOTICE.md.

**The downloader.** 9 tests against a server inside the test process: a clean
download, a file already correct not fetched again, a truncated partial resumed
from exactly where it stopped, a corrupt download rejected and deleted, a
present-but-wrong file replaced, a host offering a different size refused, a
cancellation keeping what was already fetched, and the allow-list refusing three
URLs. Two more, run explicitly: the real host's headers, and the checksums of
the model files on this machine.

**The self-test.** Three canned questions (s01, s03, s13) end to end in the real
app: all three verified, zero fabricated references, 7 minutes 33 seconds.

**Five questions in one session, through the window.** All five verified, 135.8
to 184.5 seconds each. Peak resident memory of both sidecars together: 11,763
MB. The app and its webview: 969 MB. P4 measured 9,001 MB with a fresh server
per question; P3 measured 15,068 MB over ten. README's memory figure has been
corrected to say about 9 GB for one answer and about 12 GB over a long session.

**Stop.** Measured at 16.3 seconds before the fix and 2.69 seconds after it: a
cancellation is only noticed between chunks of the response stream, and during
prompt processing no chunk arrives for tens of seconds, so two seconds after
Stop the answering model is stopped outright. It is not reloaded on the way out;
the next question pays the four seconds. A question asked after two
cancellations answered normally.

**First run, from a fresh application data directory with the model already in
place.** Window on screen 0.57 seconds after launch; 0.28 seconds on later
launches. The models are not loaded until the first question, which is why. The
machine's part of first run is then the checksum of the 4.7 GB model and the
7 minute 33 second self-test.

**Closing.** Both sidecars stop when the window closes, by a handler and by the
Job Object P4 built underneath it. Three tests: both models loaded at once and
both stopped by the close handler, dropping a session stopping them too, and a
cancellation leaving a server that can still answer.

**The crisis note** appears above the answer and never instead of it, on a real
question, in the running app. Screenshot 14.

## Jared's click-through checklist

From a fresh state. To start over: close the app and delete the folder
`%APPDATA%\io.github.haomuch1.pastorbible`. To run it without an installer:

    set TPB_MODEL_DIR=D:\Haomuch-Programs\The-Pastor-Bible\models
    src-tauri\target\release\pastor-bible.exe

1. **Welcome.** The disclaimer and the crisis note, word for word as README has
   them, then Continue. *Is this the first thing you want a stranger to read?*
2. **This computer.** Your machine beside the reference machine, five rows, and
   either a green line saying they match or a plain warning saying what is
   below. Continue is enabled either way. *Try it on a laptop if you have one.*
3. **One download.** With the model already in place it says so and Continue
   lights up. To see the download itself, move Qwen3-8B-Q4_K_M.gguf out of the
   models folder first; it is 4.7 GB, and stopping it part way and starting
   again is the thing worth testing.
4. **A quick check.** Press it and wait about seven and a half minutes. Three
   questions, three ticks, then Start using The Pastor Bible.
5. **Ask something.** Within a second the passages appear, under topic headings,
   with a line saying to read them while the answer is written. Watch the token
   count climb. *This is the part I would most like you to judge: is the wait
   bearable now?*
6. **The answer.** Themed headings, and every citation a small chip with a
   reference on it. Click one: it scrolls to that passage and outlines it.
7. **The passage panel.** Switch between Topic and Book. *The topic labels are
   the thing I am least sure of; see the flags below.* Expand a group, read the
   verse text, look at the origin tags.
8. **Stop.** Ask something and press Stop. It should give up within about three
   seconds and leave the app usable.
9. **The Deuterocanon.** Turn it on beside the question box and ask about
   wisdom, almsgiving or the fear of God. Look for the dashed "Deuterocanon"
   tags in the passage panel, and for the footer line under the answer if the
   answer happens to cite one. *This is the one screen no screenshot shows.*
10. **A question in the reader's own words about despair or self-harm.** The
    crisis note must appear above the answer, and the answer must still run.
11. **History.** Click a past question: the answer and its passages come back.
    Search for a word that is in an answer but not in a question. Delete one.
12. **Settings.** Change the canon, change the model (the smaller one downloads,
    1.7 GB), export the history to a text file and read it, delete all history.
13. **About.** Credits, licences, index version, model, reference hardware, and
    the offline statement.
14. **Close the window** and check Task Manager: no llama-server should remain.

## Flags for Jared

**The topic grouping is the weakest thing here, and it is a data problem.** P4
flagged that Nave's subtopic headings are unusable as labels. P5 groups by the
root topic instead, which turned "INSTANCES OF Ahithophel Naaman, refusing to
wash in the..." into "PRIDE", and that is a real improvement. But the second
line still shows the matched subtopic, and some of those are still paragraphs.
Screenshots 08 and 10 show it. It is worth deciding whether the grouping earns
its place at all, or whether by-book should be the default.

**Two defects were found by looking at screenshots, not by tests.** A reopened
answer showed raw `[P19]` markers, because history stored a flat list of verse
ids and lost which passage was which; and a reopened answer was grouped under
the topics of whatever had been asked most recently. Both are fixed, both now
have tests, and both would have shipped if the screenshots had not been read.
That is worth knowing about the shape of the risk here: the parts a test can
check are in good order, and the parts only eyes can check had two real bugs in
them on the first look.

**Answers still take two and a half minutes, and now the reader has something
to do.** The passages are on screen within a second. Whether that is enough is
the judgement I would most like reversed if you disagree.

**Memory is higher than P4 suggested.** 11.8 GB over a five-question session
against 9.0 GB for a single answer. README now says both. On a 16 GB machine
that is close, and the smaller model is the answer for one.

**The GPU path is measured and switched off.** P4 measured 12 seconds against
178. Settings shows the option greyed out and labelled. P6 turns it on.

**Nothing here is signed, packaged, or installed.** The app runs from
`target\release`. P6 does all of that.

## Next session

P6 Packaging and CI, per PLAN sections 11 and 13.

Installers with the fixed product id, in-place upgrade, the uninstall prompt for
user data, sidecar bundling per Tauri's externalBin convention, WebView2
bootstrapping on a clean Windows 10, the offline gate, the upgrade gate, and the
release workflow.

Three things P5 leaves for it. **The app must be built with the Tauri CLI, not
with `cargo build`**: a plain cargo build produces a binary that loads the dev
URL and shows "localhost refused to connect". **The Vulkan sidecar** is fetched
and measured but not bundled; the Compute setting is already in the window,
disabled, waiting for it. **`tools/fetch_llama.py --sidecar`** already places the
CPU build in `src-tauri/binaries` under the target-triple name Tauri wants, and
that layout has not been tested against an actual bundle.

Read PLAN.md, DECISIONS.md, API.md, SIDECAR.md and this file before starting.
Do not begin P7.
