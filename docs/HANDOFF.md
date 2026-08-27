# HANDOFF

Session: P7-prep, the v1.0.0 candidate
Date: 2026-08-27
Status: The draft release exists and is unpublished. Nothing is public. The
repository is still private. What remains is P7 itself, which only Jared can
do: install on a clean machine and follow the README.

## The draft release

    https://github.com/haomuch1/pastor-bible/releases/tag/untagged-9daa14f14a2eebe72091

That is a draft, so it has no tag URL yet and it is visible only to someone
signed in to GitHub with access to this private repository. It appears at the
top of https://github.com/haomuch1/pastor-bible/releases marked **Draft**.

Four assets, from tag `v1.0.0`, commit `200b97b`:

    The.Pastor.Bible_1.0.0_x64-setup.exe     467,271,875 bytes   Windows NSIS
    The.Pastor.Bible_1.0.0_amd64.deb                             Linux
    The.Pastor.Bible_1.0.0_amd64.AppImage                        Linux
    SHA256SUMS.txt

**The Windows installer's sha256:**

    093f26f2a8eb39bb21c5b32d28921278c291743fa11804b960f9e7ca47f616c0

That was verified by downloading the asset from the draft release and hashing
it, not by reading the build log. It matches the line in `SHA256SUMS.txt`. The
release notes carry the index.db checksum,
`d3b0579281b6a5044b6f59a0e50ec3424ef3fc25dac43042d5c8168cb29bec58`, and its
byte count.

### Two warts in SHA256SUMS.txt, neither blocking

Read these before the laptop test so they do not read as defects when they turn
up.

1. **The names in the file have spaces; the assets on GitHub have dots.** GitHub
   rewrites spaces in asset filenames on upload, so the file says
   `The Pastor Bible_1.0.0_x64-setup.exe` while what downloads is
   `The.Pastor.Bible_1.0.0_x64-setup.exe`. `sha256sum -c SHA256SUMS.txt` will
   therefore report all three as missing. Verify one file by comparing the
   number instead:

       certutil -hashfile "The.Pastor.Bible_1.0.0_x64-setup.exe" SHA256

2. **The file lists itself, with the hash of an empty file**
   (`e3b0c442...b855`). The publish step regenerates the checksums over a
   directory that already contains a copy of the file it is writing, so the
   glob catches it after the redirect has emptied it. Harmless; the three real
   hashes are correct.

Both are cosmetic and both cost a full rebuild to fix — about 69 billable
minutes. They are not worth spending that before the clean-machine test. If the
test fails and the tag has to be remade anyway, fix them in the same pass.

## The workflow run

Run 33102314189, on tag `v1.0.0`. Every job green:

    version    9s        ubuntu
    offline    2m26s     ubuntu
    build      13m4s     ubuntu-22.04     .deb and .AppImage
    build      20m47s    windows-latest   NSIS
    upgrade    2m35s     windows-latest   ran the candidate twice
    sign       skipped                    waiting on SignPath, by design
    publish    1m5s      ubuntu           draft

**Billable: 69 minutes.** 68 for this run and 1 for the run before it, which the
version gate stopped in 13 seconds. Windows is counted at 2x and each job is
rounded up to the minute: 1 + 3 + 14 + (21x2) + (3x2) + 2. The timing API still
returns zero for both runs, so these are computed from the job durations, the
same way P6's 84 was.

The upgrade gate found no previous release and said so in the log, running the
candidate twice. That tests reinstall over itself, not a version change.
Publishing this release makes the next one a real version change.

## What this session changed before tagging

The pre-release check failed on eight findings. All eight are fixed in `a334194`,
one commit ahead of the version bump, and DECISIONS records each with its
reason.

- **The installer shipped llama.cpp's binaries with no licence text at all**,
  while NOTICE.md said the MIT text travelled with them. It does not: the
  release archives carry no llama.cpp LICENSE, only `LICENSE-LLVM-OpenMP` for
  libomp, and the bundler selected files by extension. Both texts are now
  vendored in `src-tauri/licenses/`, copied into `resources/llama/` by
  `fetch_llama.py --bundle`, which refuses to assemble without them, and
  asserted by a test. **The sidecar is 25 files and the install directory is 29,
  not the 23 and 27 P6 recorded.** This was the only finding with a real legal
  obligation behind it.
- **README's three placeholder sections are written** — "Using it", "Building
  from source", "Sources and credits" — each naming a phase that had long since
  finished. P7's script says to follow only the README, so a README that did not
  describe using the app would have made every step of the test read as a
  defect.
- **Credits live in `pastor_bible_core::credits` and nowhere else.** About reads
  them; `tests/credits.rs` asserts README's list matches, in order, and runs in
  the offline gate. The check that failed was "About matches README credits",
  and it could not be made: About had carried the list since P5 and the README
  section said "filled in at P3".
- Two false statements in NOTICE.md corrected, the release notes' claim that the
  README has the SmartScreen screenshots corrected, and two figures reconciled
  to their measurements: disk 5.3 to 5.4, and graphics memory 5.4 to 5.6, the
  latter being the disk figure copied by mistake.

Then the tag itself found a ninth thing, fixed in `200b97b`.

- **The version gate compared the candidate against its own tag** and refused
  v1.0.0 in 13 seconds with "version 1.0.0 is already released". It read
  `git tag -l 'v*'`, and the tag being built is in the checkout. P6 recorded
  this fault as fixed and verified by a full tagged run; its fix filtered the
  tag list to plain `vX.Y.Z`, which excluded `v0.9.1-ci` — the only tag any P6
  run could have collided with. The gate was never exercised on the path a
  release takes. It now asks `gh release list --exclude-drafts` what has been
  published, which is what the step's own name always said.

## The laptop script

Before starting: the laptop must never have had The Pastor Bible, Rust, Node, or
the dev tools on it. Because the repo is private, you will sign in to GitHub in
the laptop's browser to reach the draft release; that is the only step a
stranger would not do.

1. Download the Windows installer from the draft release. Run it. Take a
   screenshot of the SmartScreen warning if one appears, and of the
   "More info / Run anyway" step. Record whether the README's description
   matched what you saw.
2. Follow only the README from here. Do not use anything you know from building
   it.
3. First run: record what the hardware check said about the 3050; the download
   time and size; whether the self-test passed and how long it took.
4. Ask three questions in 66-book mode. Record the time of each and whether the
   answer showed. Open Settings > Compute: record the device name shown and
   which path it chose. Expected: 8B on CPU, because 4 GB is below the
   6,325 MiB threshold.
5. Settings > switch to the smaller model. Let it download. Ask one question.
   Record the time and the Compute readout. Expected: GPU.
6. Turn on the Deuterocanon, ask "What does Tobit say about giving to the poor?",
   confirm the tag appears.
7. Click a citation; open Read chapter; delete one history entry; export as
   spreadsheet and open it.
8. Close the app. Open Task Manager: confirm no llama-server or pastor-bible
   process remains.
9. Reboot the laptop. Reopen the app from the desktop icon; confirm history is
   there.
10. Uninstall from Add/Remove Programs, choose Keep. Reinstall from the same
    file. Confirm history returns. Uninstall again, choose Delete.

Report back: each step pass/fail with the numbers, the SmartScreen screenshots,
and anything that confused you even slightly. Confusion is a defect.

### Four notes on that script, from the repository

Nothing in the repository contradicts the script. These four things it does not
say, and each would otherwise look like a failure.

- **Step 1, the file is named with dots.** It downloads as
  `The.Pastor.Bible_1.0.0_x64-setup.exe`, not with spaces. Its sha256 is above,
  and the `certutil` line above verifies it. Save the two screenshots as
  `docs/screenshots/install-1-smartscreen.png` and `install-2-more-info.png`;
  those are the two images README 9.4 has been describing since P1 and that P6
  could not produce.
- **Step 4 will be slow, and that is the expected result.** On the processor an
  answer took 2 to 3 minutes on a Ryzen 7 5800X; a laptop will be slower. Three
  questions is likely half an hour. Also watch memory: the 8B needs about 9 GB
  for one answer. If the laptop has 8 GB of RAM it will swap, which is worth
  recording as a number rather than treated as a fault.
- **Step 5 is close to the line.** The smaller model needs 2,994 MiB of *free*
  graphics memory. A 4 GB card has about 4,096 MiB total and Windows spends some
  of it on the desktop, so free is usually around 3,400 to 3,700. If Compute
  says it chose the processor and names a free figure below 2,994, that is the
  rule working correctly, not a failure — record the number it gives.
- **Step 9 has a desktop icon to click.** The installer creates one, and a Start
  Menu entry. Confirmed on this machine.

## The two paths afterwards

**If it passes**, that is P8: publish the release, make the repository public,
and apply to SignPath. Making it public also stops Actions minutes being
metered. Do not start P8 in the same session as the report; it is its own
session.

**If it fails**, fix the fault, then:

    gh release delete v1.0.0 --repo haomuch1/pastor-bible --cleanup-tag
    git tag -d v1.0.0
    git push origin :refs/tags/v1.0.0     # if --cleanup-tag did not
    git tag -a v1.0.0 -m "The Pastor Bible 1.0.0"
    git push origin v1.0.0

The version stays 1.0.0: nothing was published, so nothing was released, and the
version gate now asks about published releases rather than tags, so a remade tag
passes. That is what excluding drafts in the gate is for. Fix the two
SHA256SUMS.txt warts above in the same pass, since the rebuild is being spent
anyway.

## Still not verified

- **SmartScreen.** Unchanged from P6: no warning appeared on this machine,
  which has run these bytes many times. `docs/screenshots/` has no install
  images, and the release notes now say the screenshots are added after the
  clean-machine test rather than claiming the README already has them. Step 1
  above is the only place the claim can be checked.
- **The Linux packages have never been run.** Built in CI, checksums recorded,
  no Linux machine here. The `.deb` declares `libwebkit2gtk-4.1-0` and
  `libgtk-3-0` and has not met a real apt.
- **WebView2 bootstrapping.** The silent downloadBootstrapper has never actually
  run; this machine already had WebView2. The laptop is the first place it will.
- **The upgrade gate has still never seen a version change.** Publishing this
  release fixes that by itself.
- **Everything visual is still Jared's.** Nothing this session touched the
  palette, the type or the spacing.

## Housekeeping

`docs/pastor-bible-history.txt` is an export of Jared's own question history,
sitting untracked in `docs/`. It is not committed and will not reach a public
repository, but it should come off the machine before P8. A stray
`docs/screenshots/tmp-state.png` was removed this session, and the two files
both numbered 30 were renumbered.

## Running and building here

    src-tauri\target\release\pastor-bible.exe          the built app
    npx tauri build --bundles nsis                     the installer

From a fresh clone, three things must be fetched before a build; none is
committed, and each is checksummed:

    python tools/fetch_model.py        the embedding model, into resources/
    python tools/fetch_llama.py --bundle   the sidecar, into resources/llama/
    gh release download index-0.2.0 --pattern index.db --dir src-tauri/resources

`npm test` runs the frontend tests; `cargo test --manifest-path
src-tauri/core/Cargo.toml` runs the rest. `tools/click.ps1` drives the window —
WebView2 drops a zero-duration synthetic press, which is why `shot.ps1`'s own
click cannot be trusted for anything but screenshots.

Read PLAN.md, DECISIONS.md, API.md, SIDECAR.md and this file before starting.
