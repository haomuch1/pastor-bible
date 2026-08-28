# HANDOFF

Session: P7-fix-2, three defects a person found by clicking
Date: 2026-08-27
Status: v1.0.2 is published as a pre-release. The repository is public. The
laptop is still on 1.0.1 and stays there, so 1.0.2 can be installed over it by
hand. Next after that: a pastor's review, on a fresh install.

## The release

    https://github.com/haomuch1/pastor-bible/releases/tag/v1.0.2

Public, marked **Pre-release**, from tag `v1.0.2`, commit `f07e534`.

    The.Pastor.Bible_1.0.2_x64-setup.exe     467,259,322 bytes
    The.Pastor.Bible_1.0.2_amd64.deb         515,159,954 bytes
    The.Pastor.Bible_1.0.2_amd64.AppImage    574,913,016 bytes
    SHA256SUMS.txt                                   306 bytes

**The Windows installer's sha256:**

    bba7a7f30dccef747dcbfe75b7dff054374c4c5380b4746bed8b21834cba4656

Downloaded from a shell with no credentials at all and hashed there, and
`sha256sum -c SHA256SUMS.txt` verifies it, which it could not for v1.0.0.

## What the laptop found

The install of 1.0.1 over 1.0.0 worked and the app started. **The startup
defect P7-fix-1 shipped for is fixed on a machine that never built this
program**, which is the thing v1.0.1 existed to prove. Along the way it also
confirmed, for the first time on a machine that could confirm it:

    SmartScreen appeared, and matched what README describes
    no WebView2 install was needed
    the model downloaded at about 10 MB/s
    the quick check passed in three to four minutes on the processor
    a real question was answered in under two minutes

Then a person clicking found three defects. No script here could have found
any of them.

### 1. The upgrade was five screens and two questions about deleting data

Going from 1.0.0 to 1.0.1 the reader saw: "Already Installed" with two radio
buttons defaulting to *uninstall first*, a licence page, the **old version's
uninstaller** with a "Delete the application data" checkbox, the custom Yes/No
from `installer.nsh`, "The Pastor Bible is running! Click OK to kill it",
"Choose Install Location", and Finish.

P6's upgrade test ran with `/S`. A silent installer shows no pages at all, so
that test never saw a single one of them. A test that cannot see the thing it
is testing is not a test of it.

**What was measured first, because it decided the fix:** the already-published
1.0.0 and 1.0.1 uninstallers run perfectly under `/S` -- exit 0 at once,
nothing shown, install directory and Add/Remove entry gone, `user.db` and the
4.7 GB model untouched. They never needed avoiding, only invoking properly.
Tauri's template simply never passes `/S`.

**The mechanism, and its cost.** Every screen above is decided before the
install section starts, and all four of Tauri's installer hooks run inside it,
so no hook could reach them. `bundle.windows.nsis.template` is the only
supported extension point that can. `src-tauri/nsis-installer.nsi` is Tauri
2.11.4's own template, pulled out of the CLI binary by
`tools/extract_nsis_template.py` and verified before anything was changed:
built from the unmodified extraction, the generated `installer.nsi` differed
from Tauri's own by two `CreateDirectory` lines in a non-deterministic order
and nothing else. Our changes are 135 lines, each bracketed by
`; >>> PASTOR BIBLE` and `; <<<`.

The cost is real and is the thing to remember: **a Tauri upgrade now needs
those blocks re-applied and the screen-by-screen test re-run.**

    python tools/extract_nsis_template.py --diff    ours against a fresh stock

An upgrade now shows no reinstall, welcome, licence or location page, closes
the running app quietly instead of offering to kill it, and runs the previous
uninstaller with `/S`. A downgrade shows no page either and runs nothing --
which closed a hazard nobody had noticed, because the stock page would have
uninstalled the newer version *first* and only then been refused.

The uninstaller asks **once**: the stock checkbox is gone, the worded question
that names the folder and defaults to keeping stays.

**Recorded, screen by screen, with the app running and nothing silent.**
`tools/record-installer.ps1` reads the installer's own windows, because "the
upgrade test passed" with no screen list is exactly what let this through.

    1.0.1 -> 1.0.2    "Updating The Pastor Bible / Updating from version 1.0.1
                       to version 1.0.2. Your saved questions and your
                       downloaded model are kept."
                      then "Installation Complete".
                      Two pages, no dialog boxes. One Add/Remove entry at
                      1.0.2, 29 files, user.db unchanged at 118,784 bytes,
                      model untouched, running app closed without a word.
    1.0.2 -> 1.0.2    the same, reading "Reinstalling version 1.0.2".
    1.0.2 over 1.0.3  one message, "A newer version of The Pastor Bible is
                      already on this computer", nothing changed.

### 2. The memory message sent the reader looking for disk space

The quick check failed with "needs 6.7 GB free, only 4.3 GB available". The
reader took it for disk, uninstalled programs looking for room, and finally
rebooted -- which fixed it, because it was memory all along.

It now reads: *"The Pastor Bible needs about 6.7 GB of free memory to load the
answering model, and this computer has 4.3 GB free right now. Close other
programs or restart the computer, then try again."* It says memory twice, names
what it was loading, says what to do, and never says space or disk. A test
asserts all of that, including that the word "space" does not appear.

Pressing the button again **did** retry -- the check re-measures on every
attempt. What it did not do was look like it: it failed under identical
conditions with identical words, which is indistinguishable from nothing
happening. `TPB_FAKE_FREE_RAM_GB` now makes the path testable, the way
`TPB_NO_GPU` does for graphics, and a test proves two attempts report two
different readings. A failed attempt also drops the session, so the search
model it loaded is not still holding a quarter of a gigabyte of the memory the
message just called short.

The disk figure disagreed with the installer's because they measure different
things. This screen already measured the drive holding the app data directory,
where the model goes; it just never said which drive. It names it now.

### 3. "This computer" was wrong about graphics, twice in one sentence

It said "No separate graphics card was found. The Pastor Bible does not use one
yet" -- on a machine with an RTX 3050, from an app that has used graphics cards
since P6. It was asking the OS for one display adapter and knew nothing about
whether a model would fit on it.

It now uses `llama-server --list-devices`, the same probe Settings > Compute
uses, lists every device with its memory, and says what will happen: *"NVIDIA
GeForce RTX 3050 Laptop GPU, 4.0 GB: too small for the standard model, which
will run on the processor instead. The smaller model in Settings can use it."*
Verified both ways on this machine: the real RTX 3080 reads "big enough", and
`TPB_NO_GPU=1` reads "No graphics card was found that The Pastor Bible can use,
so the processor will answer."

## Also this session

- **README 9.4 said the SmartScreen page is blue. It is purple.** Jared's
  screenshots show it. README and PLAN 9.4 now say "a full-screen warning",
  changed together as P1 did the last time that paragraph moved. Both
  screenshots are embedded in README and the note promising them later is gone.
- README's first-run section now says the quick check takes a few minutes on a
  laptop processor, so a reader does not think it has hung.
- DECISIONS corrects P6's in-place-upgrade claim. Its numbers were all true and
  all measured with `/S`; what was verified was that a *silent* upgrade
  preserves data, not what an upgrade *looks like*.

## The laptop, next

The laptop is on 1.0.1 and stays there. Install 1.0.2 over it **by hand,
without uninstalling first** -- that is the whole point, and it is the first
time this flow will be seen on a machine that did not build it.

1. Download `The.Pastor.Bible_1.0.2_x64-setup.exe` from the release page and
   run it. Expect SmartScreen again; it is a new file.
2. **Write down every screen you see, in order.** Expected: one window that
   says "Updating from version 1.0.1 to version 1.0.2. Your saved questions and
   your downloaded model are kept", a progress bar, then Installation Complete.
   Nothing should ask about deleting anything. Nothing should mention killing.
3. Add/Remove Programs: one entry, 1.0.1 gone, 1.0.2 there.
4. Open it. Your three questions from the 1.0.1 session must still be listed.

Then the things the laptop has still never done:

5. **Settings > Compute**: record the device name and which path it chose.
   Expected: the 3050 named, and the processor chosen, because 4 GB is below
   the 6,325 MiB the standard model needs.
6. **Switch to the smaller model** in Settings, let it download, ask one
   question. Expected: the graphics card, this time. It is close to the line --
   the smaller model needs 2,994 MiB free of about 4,096 total -- so if it
   chooses the processor and names a figure below that, the rule is working.
7. **Turn on the Deuterocanon**, ask "What does Tobit say about giving to the
   poor?", confirm the Deuterocanon tag appears.
8. **Click a citation**, open Read chapter, delete one history entry, export as
   a spreadsheet and open it.
9. **Close the app**, then Task Manager: no `llama-server` and no
   `pastor-bible` left behind.
10. **Reboot**, reopen from the desktop icon, confirm the history is there.
11. **Uninstall**, choose Keep. There should be exactly **one** question about
    your saved questions, in words, defaulting to keeping them. Reinstall and
    confirm the history came back.

If it will not start:

    "%LOCALAPPDATA%\The Pastor Bible\pastor-bible.exe" --self-check

writes `%APPDATA%\io.github.haomuch1.pastorbible\self-check.txt` listing
everything the program needs before it can show anything.

## After that: the pastor's review

A fresh install of 1.0.2 on a machine that has never had it, given to someone
who reads scripture for a living, with no instructions beyond the README. The
gold lists were never reviewed by a pastor -- docs/EVAL.md says so in those
words -- and that is the largest unexamined claim this project makes.

## The two paths

**Pass** -- take the pre-release mark off 1.0.2 and finish it:

    gh release edit v1.0.2 --repo haomuch1/pastor-bible \
      --prerelease=false --title "The Pastor Bible 1.0.2"

Then drop the pre-release note from README's install section. Leave v1.0.0 and
v1.0.1 marked pre-release permanently: 1.0.0 does not start on a clean machine,
and 1.0.1's upgrade asks the two questions this release removed. SignPath is
P8's, in its own session.

**Fail** -- fix and ship **v1.0.3**. No published version is ever recreated.

## Still not verified

- **Everything in steps 5 to 11 above.** The laptop has never seen the Compute
  readout, the smaller model, the Deuterocanon tag, the export, or the orphan
  check after closing.
- **The Linux packages have never been run.** Built in CI, checksums recorded,
  no Linux machine here.
- **A deliberate downgrade from the published 1.0.1 installer.** Someone who
  has 1.0.2 and then runs the published 1.0.1 gets the old flow, which
  uninstalls 1.0.2 before refusing the downgrade, leaving no program installed
  -- their questions and model survive, and reinstalling fixes it. The 1.0.1
  installer is published and cannot be changed.
- **The uninstaller's single question, clicked through.** The stock checkbox is
  observed gone; the worded question's presence and its keep-by-default are
  proven by the `/S` run, in which the data survived, and by screenshot 31 from
  P6. Nobody has re-recorded the click-through since the checkbox was removed.
- **Everything visual is still Jared's.**

## Housekeeping

`docs/pastor-bible-history.txt` is Jared's own exported history, untracked and
in `.git/info/exclude`. It should come off the machine before P8.

`src-tauri/target/release/bundle/nsis/` holds a stray `1.0.3` installer built
only to test the downgrade refusal. It is not committed and not published.

## Running and building here

    src-tauri\target\release\pastor-bible.exe          the built app
    npx tauri build --bundles nsis                     the installer
    tools\clean-machine-check.ps1                      the installed app, honestly
    tools\record-installer.ps1                         every screen an installer shows
    tools\extract_nsis_template.py --diff              what we changed in Tauri's template

From a fresh clone, three things must be fetched before a build; none is
committed, and each is checksummed:

    python tools/fetch_model.py        the embedding model, into resources/
    python tools/fetch_llama.py --bundle   the sidecar, into resources/llama/
    gh release download index-0.2.0 --pattern index.db --dir src-tauri/resources

`npm test` runs the frontend tests; `cargo test --manifest-path
src-tauri/core/Cargo.toml` runs the rest. `tools/click.ps1` drives the window --
WebView2 drops a zero-duration synthetic press, which is why `shot.ps1`'s own
click cannot be trusted for anything but screenshots.

Read PLAN.md, DECISIONS.md, API.md, SIDECAR.md and this file before starting.
