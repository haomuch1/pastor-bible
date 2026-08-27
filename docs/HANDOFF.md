# HANDOFF

Session: P7-fix-1, the installed app could not find its own text
Date: 2026-08-27
Status: v1.0.1 is published as a pre-release. The repository is public. The
laptop still has v1.0.0 on it, and the next step is Jared installing v1.0.1
over it without uninstalling first.

## The release

    https://github.com/haomuch1/pastor-bible/releases/tag/v1.0.1

Public, marked **Pre-release**, from tag `v1.0.1`, commit `fd8a8f9`.

    The.Pastor.Bible_1.0.1_x64-setup.exe     467,250,688 bytes
    The.Pastor.Bible_1.0.1_amd64.deb         515,160,224 bytes
    The.Pastor.Bible_1.0.1_amd64.AppImage    574,913,016 bytes
    SHA256SUMS.txt                                   306 bytes

**The Windows installer's sha256:**

    17164c3b4280b0bbd409d55e362a9f991289215e93858c9730ec082246b363f9

Verified by downloading it from a shell with no credentials at all -- no token
in the environment, `curl` rather than `gh` -- and hashing it. Not from the
build log.

**`sha256sum -c SHA256SUMS.txt` now works**, which it could not for v1.0.0. Both
warts are fixed: the names in the file are the dotted names GitHub actually
serves, and the file no longer lists itself with the hash of an empty file. On
Windows:

    certutil -hashfile "The.Pastor.Bible_1.0.1_x64-setup.exe" SHA256

## What v1.0.0 got wrong

Jared installed the public v1.0.0 on a clean laptop. SmartScreen appeared as
README 9.4 describes, More info then Run anyway worked, and the installer
completed with no administrator prompt -- **so the three things README claims
about installing on a machine that has never seen this program are confirmed,
on the first machine that could confirm them.** Then the app opened to:

    The Pastor Bible could not start
    cannot read disclaimer.txt: The system cannot find the path specified (os error 3)

**It was nine files, not one.** `paths::repo_root()` was
`env!("CARGO_MANIFEST_DIR")`, an absolute path on whoever ran the compiler, and
nine runtime files resolved through it and nothing else: the disclaimer, the
crisis note, the crisis term list, the three prompts, and the evaluation set the
self-test draws its questions from. `strings` on the shipped binary shows what
it was hunting for:

    ...\Haomuch-Programs\The-Pastor-Bible\src-tauri\core

`disclaimer.txt` was only the first one read. Behind it stood the crisis term
list, which by its own design refuses to start when it holds no terms, so the
app could not have reached the main screen even if the disclaimer had loaded.
`os error 3` rather than `error 2` is itself the evidence: that is
ERROR_PATH_NOT_FOUND, raised for a missing intermediate directory, and the
laptop has no `D:` drive at all.

The three large resources -- index.db, the search model, the sidecar -- were
never affected. They resolve through Tauri's resource directory, which is the
mechanism P5.1 taught the shell to use. That lesson was never carried across to
the small text files.

### Why P6's install tests did not catch it

Every P6 check ran the installed program on the machine that built it, where
`D:\Haomuch-Programs\The-Pastor-Bible\data` is a real directory holding exactly
the right files. The installed app quietly read the repository. P6's 27 files,
its single Add/Remove entry, its ten verified answers and its passing upgrade
test were all true, and all true of an app silently using something no reader
would ever have. The defect was invisible not because those tests were careless
but because the machine supplied the missing piece. P6's own rule -- "never
against a build script" -- was the right instinct applied one level too shallow:
it is not enough to test the artifact, the artifact has to be tested somewhere
the source is not.

### The test that would have caught it

`tools/clean-machine-check.ps1` renames the repository directory away, clears
every `TPB_*` variable, and requires the installed binary to resolve everything
it needs. The rename is undone in a `finally`, and the restoration is verified
again outside it, so failing to put the repository back is itself a failure of
the script.

    powershell -ExecutionPolicy Bypass -File tools\clean-machine-check.ps1

It asks the program a question the program can answer about itself:

    "%LOCALAPPDATA%\The Pastor Bible\pastor-bible.exe" --self-check

which performs exactly the reads that failed, names each with where it came
from, and exits non-zero if any is missing. It writes its report to
`%APPDATA%\io.github.haomuch1.pastorbible\self-check.txt`, because a release
build is a GUI-subsystem binary with no console and anything printed would be
seen by nobody.

**Verified both ways on this machine.** Against the installed v1.0.0 the app
reproduced the laptop's error character for character. Against v1.0.1 the check
passes, and the window reaches the main screen with `data/` gone.

**One caveat on how it was run here.** Windows will not rename a directory that
any process holds open, and `docs/EVAL-GOLD-REVIEW.md` was open in Notepad, so
the full-repository rename could not run. It was run as `-DataOnly`, which hides
only `data/` -- where all nine files lived. That is the weaker claim: it proves
the app no longer reads `data/`, not that it reads nothing from the repository
at all. Close anything holding the directory and run the full form once before
the next release.

## The fix, per file

All nine are compiled into the binary with `include_str!`, in
`core/src/builtin.rs`. **None became a bundle resource.** A resource is a file
that can be missing; each of these is part of what the program *is* rather than
data it operates on, and compiled in there is no path to resolve and so nothing
about a machine that can make them absent.

    data/disclaimer.txt          329 B     include_str!
    data/crisis_note.txt         333 B     include_str!
    data/crisis_terms.txt        2.7 KB    include_str!
    data/prompts/synopsis.txt    1.4 KB    include_str!
    data/prompts/retry.txt       505 B     include_str!
    data/prompts/rewrite.txt     770 B     include_str!
    data/eval/questions.json     148 KB    include_str!

`summarize_batch.txt` and `summarize_merge.txt` are deliberately **not**
compiled in. Nothing loads them; they belong to the summarize-the-whole-set mode
P2 planned and nobody wired in. They stay on disk where that decision can still
be seen.

The evaluation set goes in whole rather than as the three questions lifted out
of it, because the self-test's value is that it asks what a reader would ask
rather than something chosen to pass, and it keeps that only by still being read
from the evaluation set. 148 KB inside a 445 MB installer is not worth a build
step to avoid.

They are still files on disk. `include_str!` reads them at build time, so
`data/prompts/` is versioned and diffable exactly as the P3 decision requires,
and `data/disclaimer.txt` is still the single source README is checked against.

`Settings.prompts_dir`, `crisis_terms` and `crisis_note` are now
`Option<String>`. `None` means the compiled-in copy, which is what the app
passes; `Some(path)` reads that file, which is what the harness passes so a
variant can still be tried without rebuilding. The old code could not express
"there is no file", so it always answered with a path, and the path was the
build machine's.

**The disclaimer's README-parity test did not exist.** The P5 decision says "The
welcome screen and README both come from it and a test asserts they match"; a
test asserted it for the crisis note and never for the disclaimer. It exists
now, in `tests/builtin.rs`, along with a test that every compiled-in copy is
byte-identical to its file on disk -- without which `include_str!` could point
somewhere else and every other test would still pass. That is the second
claimed-but-absent test this phase has found, after the credits one in P7-prep.

## The workflow run

Run 33125580837, on tag `v1.0.1`. Every job green:

    version    7s        ubuntu
    offline    1m27s     ubuntu           now runs the builtin suite too
    build      8m21s     ubuntu-22.04     .deb and .AppImage
    build      12m54s    windows-latest   NSIS
    upgrade    2m29s     windows-latest   a real version change, at last
    sign       skipped                    waiting on SignPath, by design
    publish    1m17s     ubuntu

**Billable: 0.** The repository is public, so Actions minutes are free. Had it
still been private this run would have cost 46, against v1.0.0's 68.

**The upgrade gate did something new.** Every previous run found no published
release and installed the candidate twice, testing reinstall-over-itself. This
time it fetched the published v1.0.0, installed it, and upgraded:

    installed 1.0.0
    one Add/Remove entry, now 1.0.1
    history survived the upgrade
    the model was not re-downloaded
    install directory holds 29 files
    downgrade refused with exit code 4; still 1.0.1

That last line had never run in CI before -- the downgrade step is skipped when
there is no previous release. The whole gate is finally doing what PLAN 11
describes.

## The same upgrade, on this machine

Published v1.0.0 downloaded and installed, then 1.0.1 installed over it without
uninstalling:

    before 1.0.0, after 1.0.1, one Add/Remove entry
    user.db 118,784 bytes before and after
    chat model timestamp unchanged, 2026-08-26 21:45:24
    29 files in the install directory

29 rather than P6's 27, because the sidecar now carries the two licence texts
P7-prep added.

## The laptop script, resumed

v1.0.0 stays on the laptop. Do **not** uninstall it. Installing v1.0.1 over it
is the first real upgrade this project has ever had on a machine that did not
build it.

Before starting: still no GitHub login, and nothing else installed. The
repository is public, so every step is a step a stranger takes.

1. Download the Windows installer from
   https://github.com/haomuch1/pastor-bible/releases/tag/v1.0.1 -- the file is
   `The.Pastor.Bible_1.0.1_x64-setup.exe`. Run it **without uninstalling v1.0.0
   first**.

   **Expect the blue SmartScreen screen again.** It is a different file, so
   Windows has no reputation for it either. **This time take the two
   screenshots**: the blue "Windows protected your PC" screen, and the screen
   after "More info" showing "Run anyway". Save them as
   `docs/screenshots/install-1-smartscreen.png` and `install-2-more-info.png`.
   Those are the two images README 9.4 has been describing since P1 and that no
   machine here can produce.

2. Open Add/Remove Programs. Expect **one** entry showing **1.0.1**. Two
   entries, or one still showing 1.0.0, is a failure -- record which.

3. Open the app from the desktop icon. It must reach the first-run screen. If it
   shows "The Pastor Bible could not start", that is the bug again, the test
   stops there, and the message should be sent verbatim.

Then the ten steps, from the first-run screen:

4. First run: record what the hardware check said about the 3050; the download
   time and size; whether the self-test passed and how long it took.
5. Ask three questions in 66-book mode. Record the time of each and whether the
   answer showed. Open Settings > Compute: record the device name shown and
   which path it chose. Expected: 8B on CPU, because 4 GB is below the
   6,325 MiB threshold.
6. Settings > switch to the smaller model. Let it download. Ask one question.
   Record the time and the Compute readout. Expected: GPU.
7. Turn on the Deuterocanon, ask "What does Tobit say about giving to the poor?",
   confirm the tag appears.
8. Click a citation; open Read chapter; delete one history entry; export as
   spreadsheet and open it.
9. Close the app. Open Task Manager: confirm no llama-server or pastor-bible
   process remains.
10. Reboot the laptop. Reopen the app from the desktop icon; confirm history is
    there.
11. Uninstall from Add/Remove Programs, choose Keep. Reinstall from the same
    file. Confirm history returns. Uninstall again, choose Delete.

Report back: each step pass/fail with the numbers, the two SmartScreen
screenshots, and anything that confused you even slightly. Confusion is a
defect.

### Four notes, from the repository

- **Step 4 will be slow, and that is the expected result.** On the processor an
  answer took 2 to 3 minutes on a Ryzen 7 5800X; a laptop will be slower. Three
  questions is likely half an hour. The 8B needs about 9 GB for one answer, so
  if the laptop has 8 GB of RAM it will swap -- record that as a number rather
  than treat it as a fault.
- **Step 6 is close to the line.** The smaller model needs 2,994 MiB of *free*
  graphics memory. A 4 GB card has about 4,096 MiB total and Windows spends some
  on the desktop. If Compute says it chose the processor and names a free figure
  below 2,994, that is the rule working correctly -- record the number.
- **Step 10 has a desktop icon to click.** The installer creates one, and a
  Start Menu entry.
- **If it will not start**, run this and send the file it names:

      "%LOCALAPPDATA%\The Pastor Bible\pastor-bible.exe" --self-check

  It writes `%APPDATA%\io.github.haomuch1.pastorbible\self-check.txt` listing
  everything the program needs before it can show anything and what it found for
  each. It would have identified this defect in one line.

## The two paths afterwards

**Pass** -- take the pre-release mark off v1.0.1 and finish it:

    gh release edit v1.0.1 --repo haomuch1/pastor-bible \
      --prerelease=false --title "The Pastor Bible 1.0.1"

Then drop the pre-release note from README's install section, add the two
SmartScreen screenshots, and delete the "added after the clean-machine test"
sentence under README 9.4. Leave v1.0.0 marked pre-release permanently: it does
not start on a clean machine and must never be the one somebody downloads.
Applying to SignPath is what remains, and it is P8's, in its own session.

**Fail** -- fix and ship **v1.0.2**. Neither published version is ever recreated:
somebody may already have the file, and two different files claiming one version
number defeats the checksum printed beside them.

    # bump src-tauri/Cargo.toml, commit
    git tag -a v1.0.2 -m "The Pastor Bible 1.0.2"
    git push origin main v1.0.2

## Still not verified

- **SmartScreen screenshots.** Jared saw the screen on the laptop and confirmed
  README 9.4's wording, but took no screenshots. Step 1 above is the second
  chance, and v1.0.1 being a new file means the screen should appear again.
- **The Linux packages have never been run.** Built in CI, checksums recorded,
  no Linux machine here.
- **WebView2 bootstrapping.** Still unknown, but narrowed: the laptop reached
  the app's own error screen, which is rendered in the web view, so WebView2 was
  either already present or installed silently without Jared noticing. Worth
  asking him which.
- **The full-repository clean-machine check** has not been run, only `-DataOnly`.
  See the caveat above.
- **Everything visual is still Jared's.** Nothing this session changed the
  palette, the type or the spacing, beyond refreshing `12-settings.png`, which
  was two phases out of date and public.

## Housekeeping

`docs/pastor-bible-history.txt` is an export of Jared's own question history,
untracked in `docs/`. It is in `.git/info/exclude` so `git add -A` cannot take
it, and its name is not published. It should come off the machine before P8.

## Running and building here

    src-tauri\target\release\pastor-bible.exe          the built app
    npx tauri build --bundles nsis                     the installer
    tools\clean-machine-check.ps1                      the installed app, honestly

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
