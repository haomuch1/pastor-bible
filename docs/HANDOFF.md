# HANDOFF

Session: P6 Packaging, GPU sidecar, upgrades, CI
Date: 2026-08-27
Status: P6 COMPLETE except the SmartScreen screenshots, which only a clean
machine can produce. Next session is P7. Do not begin P8.

## State

Repository at D:\Haomuch-Programs\The-Pastor-Bible, branch main, pushed to
https://github.com/haomuch1/pastor-bible, still private.

P6's deliverable is met: unsigned Windows and Linux installers from a tagged
build, and the upgrade test passed on this machine and again in CI.

    Windows   The Pastor Bible_0.9.1_x64-setup.exe        445.6 MB   NSIS, per user
    Linux     The Pastor Bible_0.9.1_amd64.deb                       from CI
              The Pastor Bible_0.9.1_amd64.AppImage                  from CI

The version now lives in one place, `src-tauri/Cargo.toml`. `tauri.conf.json`
has no version key, so Tauri falls back to the Cargo one and the installer and
the About screen cannot disagree — they did until this session.

Tests: 70 in cargo, 2 more run by hand, 6 in Vitest, 134 in pytest. tsc and vite
clean.

## The two findings that reshaped the session

**There is one llama.cpp build, not two.** Every file in the Vulkan release
archive is byte-identical to the CPU archive's, on Windows and on Linux, and the
Vulkan archive holds exactly one extra file: `ggml-vulkan.dll`, or
`libggml-vulkan.so`. ggml loads its backends as dynamic libraries at run time,
so that one file beside the CPU build turns `(none)` into
`Vulkan0: NVIDIA GeForce RTX 3080 (10267 MiB, 9495 MiB free)` from the same
binary. The installer ships one server, 23 files, 90.3 MB, with the Vulkan
backend among its libraries; `-ngl` decides at launch. Two copies would have
cost 99 MB for nothing. This corrects the brief's "both sidecars" and PLAN 11's
externalBin wording; DECISIONS carries both.

**Tauri's NSIS does not refuse a downgrade.** It compares versions only to word
the reinstall page, and `/S` never shows a page, so the 0.9.0 installer replaced
a 0.9.1 installation without a word. `src-tauri/installer.nsh` now refuses it in
`NSIS_HOOK_PREINSTALL`, before anything is written.

## Verified, against the artifacts

Every check below was made against a built installer, an installed directory,
the registry, or the running installed program — never against a build script.

**Install directory**, after installing 0.9.1: exactly 27 files, 734.8 MB, in
`%LOCALAPPDATA%\The Pastor Bible` — `pastor-bible.exe`, `uninstall.exe`,
`resources\index.db`, `resources\nomic-embed-text-v1.5-f16.gguf`, and
`resources\llama\` with 23 files. Nothing else. **App data**, in
`%APPDATA%\io.github.haomuch1.pastorbible`: `user.db`, `logs\`, `models\` with
the downloaded chat model. Nothing of the reader's is in the install directory
and nothing of the program's is in app data.

**Installer size** 445.6 MB, compressing 734.8 MB: index.db 366.8, embedding
model 261.6, sidecar 90.3, program 16.1.

**Upgrade**, measured: install 0.9.0, ask two questions, install 0.9.1 over it →
one Add/Remove entry showing 0.9.1, 27 files, 16 history entries intact, chat
model timestamp unchanged at 07:53:14, About shows 0.9.1.

**Downgrade**, measured: 0.9.0 over 0.9.1 → exit code 4, Add/Remove still 0.9.1,
27 files, history and model untouched.

**Uninstall**, both answers measured. Keep: install directory gone, Add/Remove
entry gone, app data and model present, and a reinstall found the history again.
Delete: app data and the 4.7 GB model gone. Screenshot 31 is the prompt, with
No as the default.

**GPU**, from the shipped binary: the ten P3 graded questions asked through the
installed window, **all ten verified, no fallbacks, zero fabricated references**,
median 6.5 s, and 13.2 s for the first of a session including the model load.
Thresholds measured at full offload with the 8192 context: 5,750 MiB for the 8B
and 2,722 MiB for the 1.7B, pinned as those plus a tenth. With `TPB_NO_GPU=1`
the probe found nothing, Settings said so, and the processor answered in 134.0 s.

**CI**, one full run on tag `v0.9.1-ci`, every job green: version gate, offline
gate, Windows NSIS, Linux .deb and .AppImage, upgrade gate, draft publish. The
tag and its draft release have been deleted.

**Offline gate** runs the query suite inside a network namespace with only
loopback, then proves it: `curl https://huggingface.co` inside the same
namespace failed with exit 6, and loopback still worked. A gate that quietly had
a network, or that passed because it had no server to talk to, would prove
nothing.

**Actions minutes: 84 billable**, Windows counted at 2x — 17 across three cheap
dry runs that found the real faults, 65 for the full tagged run, 2 for two runs
the version gate stopped in seconds.

## Not verified

- **SmartScreen.** README 9.4 describes "Windows protected your PC" and promises
  screenshots. I gave the installer a browser's mark-of-the-web and ran it, and
  no warning appeared on this machine — which has executed these exact bytes
  dozens of times and has a local reputation for them. **The screenshots are not
  in docs/screenshots/, because I will not fabricate them.** A machine that has
  never seen the file is the only place the claim can be checked, and that is
  P7. If the wording turns out to be wrong there, README 9.4 needs the edit.
- **The Linux installers have never been run.** They were built in CI and their
  checksums recorded; there is no Linux machine here. `.deb` dependencies are
  declared as `libwebkit2gtk-4.1-0` and `libgtk-3-0` and have not been tested
  against a real apt.
- **WebView2 bootstrapping.** Configured as the silent downloadBootstrapper. This
  machine already has WebView2, so the bootstrapper has never actually run.
- **The upgrade gate has never seen a version change.** There is no previous
  release, so it installed the candidate twice and said so in the log. The first
  real release makes it a version change by itself.
- **Everything visual is still Jared's.** Nothing in this session changed the
  palette, the type or the spacing.

## Jared's P7 script, on a clean Windows machine

Follow only the README. If a step is not in the README, that is the finding.

1. Download the Windows installer from the release page. **Expect the blue
   "Windows protected your PC" screen. Photograph or screenshot it, and the
   screen after "More info".** Those two images are what README 9.4 promises and
   the only thing P6 could not produce. Save them as
   `docs/screenshots/install-1-smartscreen.png` and `install-2-more-info.png`.
2. Install. It should ask for no administrator password.
3. Open it. If the machine has never had WebView2, it should install itself
   without asking. Note whether anything appeared.
4. First run: the model download, then the check. Note the download time and
   whether the check passed.
5. Pull the network cable, or turn off Wi-Fi, and ask three questions. All three
   must answer.
6. Look at Settings > Compute. Note what it says about that machine's graphics
   card, and whether answers match the wording.
7. Install a second build over the first. Confirm one entry in Add/Remove
   Programs, your questions still there, and no second model download.
8. Uninstall, choose **Keep**. Reinstall. Confirm the questions came back.

## Next session

**P7 Fresh-machine verification**, per PLAN section 13. Jared installs on a
clean Windows machine following only the README, confirms offline operation,
then installs a second build over it and confirms a single install with history
intact. Deliverable: v1.0.0 public release.

Four things P6 leaves for it.

- **The SmartScreen screenshots**, above. README 9.4 is unverified prose until
  they exist.
- **The Linux packages need a Linux machine.** They build; nobody has run one.
- **The repository is still private.** Making it public is part of v1.0.0, and
  it also makes Actions minutes free rather than metered.
- **`index-0.2.0` is a prerelease holding the built index** so CI can fetch it.
  It is a build input, not a product release, and its notes say so. If the index
  is ever rebuilt, that asset and the `INDEX_SHA256` in the workflow move
  together.

Read PLAN.md, DECISIONS.md, API.md, SIDECAR.md and this file before starting.
Do not begin P8.

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
