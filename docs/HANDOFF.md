# HANDOFF

Session: P-MAC, macOS support on Apple Silicon and Intel
Date: 2026-09-01
Status: **v1.0.3 is published as a pre-release, and it is the first release with
macOS installers.** Run `33523973443` was green on every job, including both
macOS jobs at once. Windows is unchanged from 1.0.2 in every respect a reader
can see.

**No person has run the macOS build on a Mac.** Everything a machine can prove
about it is proven on GitHub's own Apple Silicon and Intel runners; everything
visual is unseen and labelled as such wherever it appears.

The laptop is still on 1.0.1. Its steps below are unchanged, now aimed at 1.0.3.

## The release

    https://github.com/haomuch1/pastor-bible/releases/tag/v1.0.3

Public, marked **Pre-release**, titled "The Pastor Bible 1.0.3 (pre-release: no
Mac has run this)".

    The.Pastor.Bible_1.0.3_x64-setup.exe     467,313,610 bytes
    The.Pastor.Bible_1.0.3_aarch64.dmg       483,548,142 bytes
    The.Pastor.Bible_1.0.3_x64.dmg           484,038,437 bytes
    The.Pastor.Bible_1.0.3_amd64.deb         515,176,388 bytes
    The.Pastor.Bible_1.0.3_amd64.AppImage    574,925,304 bytes
    SHA256SUMS.txt                                   504 bytes

Downloaded from a shell with no credentials and hashed there; `sha256sum -c`
verifies all three installers:

    2a7b197a8469b4480bdf9a4d566ad9dd47f5c6b3f67416238627b70e9bd37f6f  x64-setup.exe
    759cdc5fdab1ddebe6e2947ef66df19ebe6b6cea2c1fae9dcc5ed49d4fe30a18  aarch64.dmg
    ebed4848aa3eb109414e8ed786d9d450549be54548dcb6449a608f135a36accf  x64.dmg

**The asset labels ran for real for the first time and worked.** All six assets
carry plain-words labels, set by the workflow rather than by hand, and the two
new ones read "Mac installer — Apple Silicon (M1–M4, 2021 and newer) — unsigned,
see install steps (<name>)" and "Mac installer — Intel Macs, pre-2021 —
unsigned, see install steps (<name>)". Verified on the published page.

### The first answers a Mac ever gave

Both through the shipped `.app`'s own index.db, search model and model server,
on graded eval question g01, with the smaller model:

    Apple Silicon   verdict ok, 293 retrieved / 92 cited, 0 violations
                    126 tokens in 58.3 s = 2.16 tokens/s, 104.4 s in all
    Intel           verdict ok, 293 retrieved / 92 cited, 0 violations
                    126 tokens in 100.7 s = 1.25 tokens/s, 108.8 s in all

The Apple Silicon figure was taken with the free-memory guard **overridden** —
the runner read 4.81 GB free and the pipeline needs about 5.0 before the chat
model loads. The job says so in its own log. The Intel job, 14 GB, never
overrides it. Both disk images reported `Finder layout applied (.DS_Store
present, 10244 bytes)` with the background picture in them.

## The reversal

Jared reverses the locked decision "macOS dropped (notarization requires a paid
Apple account)". The pastors who are about to review this use MacBooks, some
possibly Intel, and a reviewer who cannot install the program cannot review it.

The premise of the original decision is untouched: notarisation does require a
paid Apple Developer account, and this project will never have one. What
changed is the conclusion. Notarisation is not required to *run* an app on
macOS; it is required to run one without the reader being stopped once. So the
app ships **ad-hoc signed** — a signature with nobody's name on it — the reader
is stopped once, and the instructions say so plainly and say exactly what to do
about it. Jared explicitly approves that trade. It is the same trade Windows
already makes with SmartScreen, on a platform that makes it harder.

## What macOS turned out to need

Four things, none of which could have been found by reading code on this
machine.

**1. llama.cpp b10639 does not start on Apple Silicon.** The first CI run of
this phase printed:

    dyld: Library not loaded: /usr/lib/librdma.dylib
      Referenced from: .../src-tauri/resources/llama/libggml-rpc.0.dylib

`libggml-rpc` is a hard `LC_LOAD_DYLIB` of `llama-server`, so it cannot be left
out the way the Windows bundle leaves it out — on Windows ggml opens it at run
time, on macOS dyld demands it at launch — and in b10639 it hard-links a system
library that does not exist before macOS 26. The binary's own
`LC_BUILD_VERSION` says it supports 13.3.

The load commands of every release between b10639 and b10700 were read. Hard
through **b10693**; `LC_LOAD_WEAK_DYLIB`, which dyld tolerates when the file is
absent, from **b10694**. The x64 build references `librdma` at neither tag,
which is why the Intel job passed the step the Apple Silicon job failed.

**macOS is pinned to b10694. Windows and Linux stay on b10639** — their
installers are published and their behaviour measured. `tools/fetch_llama.py`
carries a per-asset tag and prints which release each checksum matched, so the
two cannot be confused. **This is a thing to remember at the next llama.cpp
bump: there are two pins now, not one.**

**2. The orphan guarantee had no macOS half.** `PR_SET_PDEATHSIG` is Linux and
only Linux, and the code called it under `cfg(unix)`, which would not have
compiled. It is now a pipe: the parent holds the write end close-on-exec, a
`/bin/sh -c 'cat > /dev/null; kill -9 <pid>'` holds the read end as its stdin,
and the instant the parent dies for any reason — Force Quit included — that
shell kills the model server. On an orderly stop the reaper is killed first and
the server second, so the `kill -9` is never issued at all. The existing
hard-kill test in `sidecar_lifecycle` is the same test on all three platforms
and **passes on both macOS runners**.

**3. There is no `/proc` on Darwin.** Free memory, total memory, the
processor's name and free disk all fell through to Linux code that reads
`/proc` and would have reported zero — which would have made the free-memory
check refuse every model load with "0.0 GB free". They now read `vm_stat`,
`sysctl` and `statvfs`. `vm_stat` and `sysctl` are run as subprocesses rather
than called through Mach: no Mac exists here to compile against, and an FFI
signature `libc` does not export for Darwin is a build failure discovered at
the end of a runner queue. They are read once per model load, not in a loop.

**4. Resources are not beside the executable in a `.app`.** `--self-check`
derived the resource directory from the binary's own folder, which is right on
Windows and Linux and is `Contents/MacOS` on a Mac, where nothing lives. It
would have reported index.db, the search model and the model server all missing
inside a bundle that had all three. It now tries `Contents/Resources` as well.

## What the release run cost, so a repeat is recognisable

Six attempts. Every failure was a real defect except the last, and none is
outstanding.

    run 1   arm64: llama-server would not start (b10639 librdma) -> repinned
    run 2   arm64: free-memory guard refused the model on a 7 GB runner
    run 3   both green but arm64 memory; found Accelerate and the MTL name
    tag 1   arm64: memory again -- purge ran, then a 57 s compile undid it
    tag 2   intel: hdiutil "couldn't eject disk4 - Resource busy" on a disk
            image that was already correct. A flake; the detach now retries.
    tag 3   green on everything, published.

## What is proven, and by what

Both macOS jobs are `macos-15` and `macos-15-intel`, native, no
cross-compilation and no Rosetta. GitHub still offers Intel runners, so the
STEP 0 branch about cross-compiling never had to be taken.

Each job, in order: fetch index.db, the search model and the sidecar by pinned
checksum; refuse if a symlink reached the bundle; run `llama-server
--list-devices` and print what it says; `npm run build` and `vitest`; the Rust
suites; the hard-kill orphan test; `tauri build --bundles app`; record what
`codesign` and `spctl` say about the bundle with a downloaded copy's quarantine
flag on it; build the `.dmg`; assert the image holds the app, the Applications
link and READ-ME-FIRST.rtf; copy the app out as a reader would and run its own
binary with `--self-check`; fetch the smaller answering model by pinned
checksum; ask one graded eval question end to end **through the app's own
index.db, search model and model server**; require a verified answer with zero
fabrications; then replace the app from the image and prove the reader's
questions and downloaded model survived it.

## What is not proven, and cannot be here

- **Everything visual on macOS.** The disk image window, the Gatekeeper
  dialogs, the app's own screens on a Mac. There are no macOS screenshots and
  none is invented.
- **The two Gatekeeper flows.** Written down from Apple's own documentation,
  dated, and marked "not yet walked through on a real Mac" in README, in the
  reviewer guide and in READ-ME-FIRST.rtf itself.
- **macOS 13.3 and 14.** Both runners are macOS 15. 13.3 is what the binaries
  declare, and b10639 is proof that a declaration is not a fact. README says
  which is which.
- **Whether a Metal device appears on real Apple Silicon.** The runner is a
  virtual machine; what Metal does inside one says nothing about a MacBook. The
  job records what `--list-devices` said rather than asserting it.
- **The Linux packages, still.** Built in CI, checksums recorded, never run.

## The laptop, next — now to 1.0.3

Unchanged from the P7-close-prep list except the version. The laptop is on
1.0.1 and stays there; install 1.0.3 over it **by hand, without uninstalling
first**.

1. Download `The.Pastor.Bible_1.0.3_x64-setup.exe` and run it. Expect
   SmartScreen; it is a new file.
2. **Write down every screen, in order.** Expected: "Updating from version
   1.0.1 to version 1.0.3. Your saved questions and your downloaded model are
   kept", a progress bar, then Installation Complete. Nothing should ask about
   deleting anything.
3. Add/Remove Programs: one entry, 1.0.1 gone, 1.0.3 there.
4. Open it. The three questions from the 1.0.1 session must still be listed.

Then the things the laptop has still never done, unchanged: **5.** Settings >
Compute, record the device and the path chosen. **6.** Switch to the smaller
model, download, ask one question. **7.** Turn on the Deuterocanon, ask about
Tobit, confirm the tag. **8.** Click a citation, Read chapter, delete a history
entry, export a spreadsheet. **9.** Close, then Task Manager: no `llama-server`
and no `pastor-bible`. **10.** Reboot, reopen, confirm history. **11.**
Uninstall, choose Keep; exactly one worded question, defaulting to keeping.

If it will not start:

    "%LOCALAPPDATA%\The Pastor Bible\pastor-bible.exe" --self-check

## The first Mac install — a guided script, for a phone call

This is the piece of work that matters most now, and it needs a person with a
Mac. **Do it on the phone with the pastor, or with anyone who has a Mac.** The
point is not to confirm it works; the point is to find out what it actually
looks like, because nobody knows.

Before sending anyone a link: **ask which Mac they have.** Apple menu → About
This Mac. "Apple M1/M2/M3/M4" means the Apple Silicon file; "Intel" means the
Intel one. Send the link and the name of the file, not just the link.

On the call, in order:

1. Ask them to read out what About This Mac says — the chip **and the macOS
   version**. Write both down. Everything below depends on the version.
2. Watch them download and open the `.dmg`. **Ask them to describe the window
   before they touch anything**, and to take a screenshot of it (Shift-Cmd-4,
   then space, then click the window). This is the first time anybody will have
   seen the disk image. Ask specifically: is there a background picture with
   words on it, or a plain window? Are the three items where they should be?
3. Have them read READ-ME-FIRST.rtf **aloud**. Anything that makes them pause
   is a defect in the writing.
4. Drag to Applications, then double-click. **Screenshot every dialog**,
   including Gatekeeper's, and write down its exact wording — the title, the
   body, and the buttons. Compare it against what README says. If it does not
   match, README is wrong and the words the Mac used are the ones to keep.
5. Walk the Open Anyway or Control-click path for their version. Screenshot
   System Settings > Privacy & Security if they get there. Note whether the
   button was where README says it is.
6. First run: note the download time and, more importantly, **the answer time
   for their first real question**, and whether Settings > Compute names a
   device. On an Intel Mac, check that the "no graphics card" line actually
   appears on the download screen and in Settings.
7. Ask them to close the app and, if they can, check Activity Monitor for a
   stray `llama-server`.

Then README gets real Mac pictures, and the "not yet installed by a person"
sentence comes out of it — and not before.

## The pastor's review

Unchanged and still the next thing. `docs/REVIEW-GUIDE.md` is what they are
handed; it now carries both install paths, and a reader follows only the one
for their computer. The three judgement questions are untouched:

    Does any answer say something the cited passages don't say?
    Is any passage you'd expect for a question missing?
    Does any wording in the app feel wrong for someone in your position?

## The two paths

**Pass** — take the pre-release mark off 1.0.3:

    gh release edit v1.0.3 --repo haomuch1/pastor-bible \
      --prerelease=false --title "The Pastor Bible 1.0.3"

Then drop the pre-release note from README's install section. Leave v1.0.0,
v1.0.1 and v1.0.2 marked pre-release permanently. SignPath is P8's.

**Fail** — fix and ship **v1.0.4**. No published version is ever recreated.

## Building here

Unchanged on Windows:

    src-tauri\target\release\pastor-bible.exe          the built app
    npx tauri build --bundles nsis                     the installer
    tools\clean-machine-check.ps1                      the installed app, honestly
    tools\record-installer.ps1                         every screen an installer shows
    tools\extract_nsis_template.py --diff              what we changed in Tauri's template

On a Mac:

    python3 tools/fetch_llama.py --bundle              picks the archive for this chip
    npx tauri build --bundles app
    bash tools/make_dmg.sh "src-tauri/target/release/bundle/macos/The Pastor Bible.app" \
      "out/The Pastor Bible_<version>_<aarch64|x64>.dmg"

`tools/mac_dmg_background.py` redraws `src-tauri/dmg/background.png`; the icon
positions in it and in `make_dmg.sh` must agree.

From a fresh clone, three things must be fetched before a build; none is
committed, and each is checksummed:

    python tools/fetch_model.py        the embedding model, into resources/
    python tools/fetch_llama.py --bundle   the sidecar, into resources/llama/
    gh release download index-0.2.0 --pattern index.db --dir src-tauri/resources

`npm test` runs the frontend tests; `cargo test --manifest-path
src-tauri/core/Cargo.toml` runs the rest.

Read PLAN.md, DECISIONS.md, API.md, SIDECAR.md and this file before starting.
