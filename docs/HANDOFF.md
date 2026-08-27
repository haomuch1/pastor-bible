# HANDOFF

Session: P7-prep, then P7-publish
Date: 2026-08-27
Status: The repository is public and v1.0.0 is published as a pre-release.
What remains is P7 itself, which only Jared can do: install on a clean machine
and follow the README.

## The release

    https://github.com/haomuch1/pastor-bible/releases/tag/v1.0.0

Public, and reachable by anyone with no account. It is marked **Pre-release**,
because nothing README claims about a first install has been checked on a
machine that did not build the program. That flag is the whole point: it says
in GitHub's own vocabulary what a paragraph of prose would need a reader to
notice. It comes off when the test passes.

Because a pre-release carries no "Latest" badge, `/releases/latest` does not
resolve to it -- the API returns 404 and the web URL falls back to the releases
index, which does list it. README's install section now sends people to the
releases page and says to take the topmost entry, rather than to "the latest
release page", which would have been the stranger's very first stumble.

Four assets, from tag `v1.0.0`, commit `200b97b`:

    The.Pastor.Bible_1.0.0_x64-setup.exe     467,271,875 bytes   Windows NSIS
    The.Pastor.Bible_1.0.0_amd64.deb                             Linux
    The.Pastor.Bible_1.0.0_amd64.AppImage                        Linux
    SHA256SUMS.txt

**The Windows installer's sha256:**

    093f26f2a8eb39bb21c5b32d28921278c291743fa11804b960f9e7ca47f616c0

That was verified twice: once by downloading the asset from the draft and
hashing it, and again after publication from a shell with no GitHub credentials
at all -- no token in the environment, `curl` rather than `gh` -- which is the
path a stranger takes. Both matched, and neither read the build log. It matches
the line in `SHA256SUMS.txt`. The release notes carry the index.db checksum,
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
minutes, though minutes are free now that the repository is public. They are not
worth a rebuild of their own before the clean-machine test. Fix them in whatever
build comes next, whether that is v1.0.1 or a later version.

## The workflow run

Run 33102314189, on tag `v1.0.0`. Every job green:

    version    9s        ubuntu
    offline    2m26s     ubuntu
    build      13m4s     ubuntu-22.04     .deb and .AppImage
    build      20m47s    windows-latest   NSIS
    upgrade    2m35s     windows-latest   ran the candidate twice
    sign       skipped                    waiting on SignPath, by design
    publish    1m5s      ubuntu           draft, published since

**Billable: 69 minutes.** 68 for this run and 1 for the run before it, which the
version gate stopped in 13 seconds. Windows is counted at 2x and each job is
rounded up to the minute: 1 + 3 + 14 + (21x2) + (3x2) + 2. The timing API still
returns zero for both runs, so these are computed from the job durations, the
same way P6's 84 was.

The upgrade gate found no previous release and said so in the log, running the
candidate twice. That tests reinstall over itself, not a version change. v1.0.0
is published now, so the next tag makes it a real version change by itself —
including a v1.0.1 built on the fail path below.

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
the dev tools on it. Do not sign in to GitHub on it, and do not sign in to
anything else either. The repository is public now, so every step below is a
step a stranger takes, and that is the point of the test.

1. Download the Windows installer from
   https://github.com/haomuch1/pastor-bible/releases/tag/v1.0.0 . Run it. Take a
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

**Pass** -- remove the prerelease flag and retitle:

    gh release edit v1.0.0 --repo haomuch1/pastor-bible \
      --prerelease=false \
      --title "The Pastor Bible 1.0.0"

Then drop the pre-release note from README's install section, add the two
SmartScreen screenshots, and that is v1.0.0 finished. Applying to SignPath is
what remains, and it is P8's, in its own session.

**Fail** -- fix the fault and ship **v1.0.1**. Do not remake v1.0.0:

    # bump src-tauri/Cargo.toml to 1.0.1, commit
    git tag -a v1.0.1 -m "The Pastor Bible 1.0.1"
    git push origin main v1.0.1

v1.0.0 is published and public, so somebody may already have it. A published
release is not a draft and must not be deleted and recreated under the same
number: that would leave two different files claiming to be v1.0.0, and the
whole point of printing a checksum is that a version names exactly one file.
This is the one thing that changed when the repository went public, and it is
why the version gate asks about published releases -- it will now correctly
refuse a second v1.0.0.

Leave v1.0.0 in place, marked pre-release, and let v1.0.1 supersede it. Fix the
two SHA256SUMS.txt warts above in the same pass, since a rebuild is being spent
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
- **The upgrade gate has still never seen a version change.** v1.0.0 is now a
  published release, so the next tag makes it a real version change by itself.
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
