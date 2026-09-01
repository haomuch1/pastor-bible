# SIDECAR

The model server, its exact invocation, and the endpoints the backend uses.
This document closes the PLAN section 16 VERIFY item "llama-server binary name,
flags, and OpenAI-compatible endpoint", owned by P4. Everything below was run
against the binary on this machine and its output pasted from the run, not
recalled.

## The pinned build

    project     llama.cpp, ggml-org/llama.cpp, MIT
    release tag b10639
    commit      5e6a37cb115dc1074e274ac004373f5661909695
    asset       llama-b10639-bin-win-cpu-x64.zip
    sha256      3bffee4da688dc404e8599571a6f79ca5f38f42b428c4f74a51945520305284e
    size        18,076,865 bytes
    reports     version: 0.3.0-dev (build 10639, commit 5e6a37cb1)
                built with Clang 20.1.8 for Windows x86_64

The Linux counterpart of the same tag, for the Linux target in P6:

    asset       llama-b10639-bin-ubuntu-x64.tar.gz
    sha256      3f928f12abc5aaec2b21e9c8116292910f9f5e76eb2605ae6a9578b0413de626

The Vulkan builds, bundled from P6:

    asset       llama-b10639-bin-win-vulkan-x64.zip
    sha256      3fb85c859f2cf90b9626a66e9742baed416c1ceda767d5c906520547b36425ad
    asset       llama-b10639-bin-ubuntu-vulkan-x64.tar.gz
    sha256      6168bd9affe15b5cdbf553d70d2f162df5268c50da038000dcd3f0dc537ec7ca
    size        32,936,221 bytes

## There is one build, not two

Measured on 2026-08-27, on both platforms: every file in the Vulkan archive is
byte-identical to the file of the same name in the CPU archive, and the Vulkan
archive holds exactly one file the CPU archive does not — `ggml-vulkan.dll` on
Windows, `libggml-vulkan.so` on Linux. ggml loads its backends as dynamic
libraries at run time, so that one file beside the CPU build is the whole
difference:

    > llama-server.exe --list-devices          (without it)
    Available devices:
      (none)

    > llama-server.exe --list-devices          (with it)
    Available devices:
      Vulkan0: NVIDIA GeForce RTX 3080 (10267 MiB, 9495 MiB free)

So the installer ships one server and one set of libraries with the Vulkan
backend among them, and `-ngl` decides at launch which processor runs the model.
`tools/fetch_llama.py --bundle` assembles it into `src-tauri/resources/llama/`:
23 binaries, 90.3 MB, trimmed from the archive's 51 to what llama-server
actually loads. That trim was established by removing files until it stopped
starting; dropping `mtmd.dll` makes it exit 0xC0000135, DLL not found.

Beside them go two licence texts, making 25 files in all. llama.cpp is MIT and
`libomp.dll` is Apache-2.0 with LLVM exceptions, and both licences require their
notice to travel with any copy of the software; an installer is such a copy.
They are vendored in `src-tauri/licenses/` rather than lifted out of the
archive, because the archive has no llama.cpp LICENSE in it at all -- the only
licence file it carries is `LICENSE-LLVM-OpenMP`, for libomp. `--bundle` refuses
to assemble without both. Found in P7-prep, when NOTICE.md was checked against
the shipped directory and turned out to claim a licence text that was not there.

`tools/fetch_llama.py` fetches a named asset of the pinned tag, checks the
sha256 before unpacking, and refuses on a mismatch. The zip already on this
machine was checksum-matched against the release asset rather than re-fetched.

`--sidecar` also places the build in `src-tauri/binaries`, naming the server
for its target triple as Tauri's externalBin convention requires. Run on
2026-08-26 it placed 51 files with the server as
`llama-server-x86_64-pc-windows-msvc.exe`. That directory is gitignored except
for its `.gitkeep`: the sidecar is fetched by tag and checksum and bundled by
the installer, and nothing binary is ever committed.

## macOS, both chips

Added in P-MAC on 2026-09-01, when the "macOS dropped" decision was reversed.
Everything in this section was measured against the archives themselves, on the
build machine, by downloading them and reading their bytes. Nothing here was
recalled and nothing was taken from GitHub's own digest field without checking
it.

    release tag b10694
    commit      2bf04151520843e9ea5694e655e7d4a537973b54
    asset       llama-b10694-bin-macos-arm64.tar.gz
    sha256      e5423012dfb20fefe586906a24ade087632b89660abfdab810b2612557f2e081
    size        11,032,471 bytes
    asset       llama-b10694-bin-macos-x64.tar.gz
    sha256      b52b861baf8540d23b7b0aecf2a36994749256796d3d119a002fd522bd02cabb
    size        11,094,939 bytes

### Why macOS is on a later release than Windows and Linux

**b10639's Apple Silicon build cannot start on any macOS before 26.** This was
not deduced; a runner said so, in the first CI run of this phase:

    dyld[3471]: Library not loaded: /usr/lib/librdma.dylib
      Referenced from: .../src-tauri/resources/llama/libggml-rpc.0.dylib
      Reason: tried: '/usr/lib/librdma.dylib' (no such file), ...

That is macOS 15 refusing a binary whose own `LC_BUILD_VERSION` claims macOS
13.3. `libggml-rpc` is a hard `LC_LOAD_DYLIB` of `llama-server`, of
`libllama-server-impl` and of `libggml`, so it cannot simply be left out — the
Windows bundle drops the RPC backend, but on Windows it is loaded at run time
and on macOS it is linked at launch. And in b10639 `libggml-rpc` itself carries
a hard `LC_LOAD_DYLIB` on `/usr/lib/librdma.dylib`, a system library that
llama.cpp's own Apple Silicon builder has (it runs macOS 26, SDK 26.5) and that
no earlier macOS does.

The load commands of every release between b10639 and b10700 were read. The link
is hard through **b10693** and weak — `LC_LOAD_WEAK_DYLIB`, which dyld tolerates
when the file is absent — from **b10694** onward. The x64 build does not
reference `librdma` at either tag, which is why the Intel job passed the same
step that the Apple Silicon job failed, and why this was invisible until an arm64
machine ran the binary.

So the macOS assets are pinned to b10694 and Windows and Linux stay on b10639.
Their installers are published, their behaviour is measured, and nothing about
this defect touches them. `tools/fetch_llama.py` carries a per-asset tag for
exactly this, and prints which release each checksum matched.

Everything else about b10694 was checked against b10639 before pinning it: same
file names, same `.0.dylib` reference names, `LC_BUILD_VERSION minos 13.3.0` on
both chips, and llama.cpp's `LICENSE` still byte-identical to the vendored copy.
The only external libraries either build needs beyond `libSystem` and `libc++`
are `CoreFoundation`, `Security`, `Accelerate`, `libobjc`, and — on arm64 only —
`Foundation`, `Metal` and `MetalKit`. All of those have existed since long
before 13.3.

Both archives unpack to a single directory `llama-b10694/`: 60 entries on arm64,
56 on x64, of which 18 and 16 respectively are symlinks between the three names
each library carries. Each archive contains llama.cpp's own `LICENSE`, 1,078
bytes, sha256 `94f29bbe...f0f1d010d` — **byte-identical to the copy already
vendored in `src-tauri/licenses/llama.cpp-LICENSE.txt`**, which the Windows and
Linux bundles ship because the Windows archive carries no licence text at all.
The same file therefore ships on all three platforms and is now confirmed
against upstream rather than merely believed.

### Metal is on Apple Silicon and nowhere else

The arm64 archive carries `libggml-metal.0.22.0.dylib` (2,078,712 bytes) and
`ggml-metal-tuning`. **The x64 archive carries no Metal backend of any kind**,
and its `llama-server` does not reference one. This is not an inference from
hardware: it is the file list, and it is the load commands.

    arm64  llama-server references @rpath/libggml-metal.0.dylib
    x64    llama-server references no metal library

So on an Intel Mac this program has no graphics path at all, whatever card the
machine has, and the "no graphics card The Pastor Bible can use" wording is a
statement about the build rather than a guess about the reader's hardware.
There is no `default.metallib` in the archive: the shaders are embedded in
`libggml-metal`, which is llama.cpp's release default.

### What the bundle keeps, and why exactly those files

The Windows trim was established by deleting files until the server stopped
starting. No Mac exists here to do that on, so the list was read out of the
binaries instead. Every Mach-O in the archive was scanned for its `@rpath`
load-command strings; the union of what `llama-server` and everything it loads
ask for is:

    @rpath/libllama-server-impl.dylib     @rpath/libggml.0.dylib
    @rpath/libllama.0.dylib               @rpath/libggml-base.0.dylib
    @rpath/libllama-common.0.dylib        @rpath/libggml-cpu.0.dylib
    @rpath/libmtmd.0.dylib                @rpath/libggml-blas.0.dylib
                                          @rpath/libggml-rpc.0.dylib
    and, on arm64 only, @rpath/libggml-metal.0.dylib

`@rpath` itself resolves to `@loader_path` in `llama-server` and in every
library — the only LC_RPATH either carries. **The server and its libraries must
therefore sit in one directory**, exactly as on Windows and for the same
practical reason, though the mechanism is different.

Note that every referenced name is the `.0.dylib` middle name, which in the
archive is a symlink to a `.0.22.0.dylib` or `.0.3.0.dylib` real file. Tauri's
resource copying is not relied on to preserve symlinks: `fetch_llama.py`
resolves each one and writes the real bytes under the referenced name, so the
bundled directory contains eleven ordinary files and no links. Nothing
llama.cpp's own CLI tools need — `libllama-cli-impl`, `libllama-bench-impl`,
`libllama-perplexity-impl` and the rest — is shipped, because the tools are not
shipped either.

    arm64   llama-server + 10 libraries + LICENSE   12 files, 24,983,694 bytes
    x64     llama-server +  9 libraries + LICENSE   11 files, 24,406,374 bytes

`libggml-rpc` is in that list and is not wanted: this app never uses the RPC
backend and the Windows bundle drops it. On macOS it cannot be dropped, because
it is a hard load command of the server rather than something ggml opens at run
time. It is the reason for the tag above.

`libomp` does not exist in either archive, so `LICENSE-LLVM-OpenMP.txt` is a
Windows and Linux obligation only and is not shipped in the .app.

### The floor is macOS 13.3, on both chips

Read from `LC_BUILD_VERSION` in the shipped binaries:

    arm64  minos 13.3.0   sdk 26.5.0
    x64    minos 13.3.0   sdk 15.5.0

That declared floor is what the binary claims, and b10639 proved a claim is not
a fact: it declared 13.3 and would not start on 15. The floor this project
states is the one it has evidence for — a CI job that builds, runs and asks a
real question on a macOS 15 machine of each chip. **13.3 and 14 are unverified
by anybody**, and README does not pretend otherwise.

llama.cpp builds its Intel release against the same 13.3 deployment target as
its Apple Silicon one, so the Intel build buys no older-macOS support. That is
why `bundle.macOS.minimumSystemVersion` is `13.3` for both and not Tauri's
`10.13` default: with the default, a Mac on macOS 12 would install the app
happily and then fail inside dyld with a message about a library, which is the
worst possible place to learn it.

### Signing state of the upstream binaries

    arm64  every Mach-O carries LC_CODE_SIGNATURE (ad-hoc; arm64 requires it)
    x64    no Mach-O carries one (Intel does not require it)

Neither is signed by anybody. This project adds no Developer ID either; see
DECISIONS for the ad-hoc decision and README for what the reader sees.

What macOS says about the bundle Tauri produces, read off a macOS 15 runner on
2026-09-01 rather than described:

    codesign -dvvv     Signature=adhoc, flags=0x2(adhoc), TeamIdentifier=not set
                       Sealed Resources version=2 rules=13 files=14
    codesign --verify  valid on disk; satisfies its Designated Requirement
    spctl --assess     rejected
                       (on a copy carrying a downloaded file's quarantine flag)

`rejected` is the expected answer and the wanted one. A bundle whose signature
had been broken — by editing it after signing — assesses differently and
produces the "is damaged and can't be opened" dialog instead of the
unidentified-developer one, which is why `tools/make_dmg.sh` copies the `.app`
in and does nothing else to it. None of this is evidence about the *wording* of
the dialog a reader sees; that is Apple's documentation, cited in README, and
nobody here has seen it.

### Binary name and flags

`llama-server`, the same name as Linux, and the same flags — nothing in the
invocation is platform-specific. `-ngl` still decides which processor runs the
model; on Apple Silicon the "card" it offloads to is the same physical memory
the rest of the machine is using, which is why the Compute wording differs
there and only there.

## Binary name

    Windows   llama-server.exe
    Linux     llama-server

The Windows build is not a single file. `llama-server.exe` loads
`llama-server-impl.dll`, `llama.dll`, `ggml.dll`, `ggml-base.dll`, `ggml-cpu.dll`
and one `ggml-cpu-<microarch>.dll` per CPU generation, selected at run time.
Every DLL in the archive ships beside the exe. Tauri's `externalBin` names one
file per target triple, so the sidecar entry is `llama-server` and the DLLs are
bundled as resources next to it; P6 confirms the layout in a built installer.

## Flags

Common to both roles:

    -m <path>            model file
    --host 127.0.0.1     loopback only; never a routable interface
    --port <n>           a free port chosen by the parent, never a fixed one
    -ngl 0               CPU only. The GPU path is a P6 decision.
    --no-webui           the server serves no HTML; the app is the only client
    -t <n>               threads

Chat role adds:

    -c <n>               context size, derived from measured prompt length
    -np 1                one slot. The server cannot serve two requests at once.

Embedding role adds:

    --embeddings         restrict the server to the embedding use case
    -c 2048              the embedding model's own context
    -b 2048 -ub 2048     physical batch must hold the longest single input,
                         which is a different constraint from the context

## Endpoints

Verified against the running server on 2026-08-26:

    GET  /health              -> 200 {"status":"ok"}   readiness
    POST /v1/embeddings       -> OpenAI shape: {"data":[{"index":n,"embedding":[...]}]}
    POST /v1/chat/completions -> OpenAI shape: {"choices":[{"message":{"content":...}}],
                                                "usage":{"prompt_tokens":...}}
    POST /completion          -> llama.cpp's own: {"content":...,"tokens_evaluated":...}
    POST /apply-template      -> the prompt string a message list renders to
    GET  /props, /v1/models, /slots  present, not used by the app

The backend uses `/health`, `/v1/embeddings` and `/v1/chat/completions`.

### Why /v1/chat/completions rather than P3's /completion

P3 built the Qwen3 chat prompt by hand and posted it to `/completion`. The Rust
backend posts a message list to `/v1/chat/completions` instead, and this is a
parity claim, not a preference. `POST /apply-template` with the same message
returned exactly the string P3 built by hand:

    <|im_start|>user\nSay OK. /no_think<|im_end|>\n<|im_start|>assistant\n

Both endpoints reported `prompt_tokens` 15 for it. The one difference is that
`/v1/chat/completions` removes the `<think>` block itself, which P3 removed with
a regular expression; the resulting text was identical. The Rust client strips a
residual think block anyway, because the guarantee should not rest on the
server's behaviour staying the same across builds.

## The model host

PLAN section 16's "Model download host, size, checksum" VERIFY item, owned by
P5. Checked on 2026-08-26 against the live host, from Python and again from the
Rust client that ships.

    host            huggingface.co, redirecting to us.aws.cdn.hf.co
    URL form        https://huggingface.co/<org>/<repo>/resolve/main/<file>
    token needed    no. HEAD returns 200 with no WWW-Authenticate, and a
                    ranged GET returns 206, both unauthenticated.
    HTTP Range      honoured. A request for bytes 100-199 returns 206 with
                    Content-Range naming the full size, so resume works and
                    the size can be learnt without downloading anything.
    TLS             required. The client is rustls with a bundled root store,
                    so a clean machine needs nothing installed.

    file                              bytes          sha256
    Qwen3-8B-Q4_K_M.gguf              5,027,783,488  d98cdcbd...5745785
    Qwen3-1.7B-Q8_0.gguf              1,834,426,016  061b54da...6590cb1a
    nomic-embed-text-v1.5.f16.gguf      274,290,560  f7af6f66...cf1c2fdb

Every size above came from the host's own Content-Range and matches the file on
the build machine byte for byte; every checksum matches the one EVAL.md
recorded in P3. The full values are in src-tauri/core/src/download.rs, which is
where the code reads them.

The reader downloads only the answering model. The search model is bundled with
the installer and the smaller answering model is fetched only if they choose it
in Settings.

Note on Content-Length: after the redirect to the CDN, a HEAD response does not
carry a usable Content-Length. The downloader therefore learns the size from
the GET it is already making, and from Content-Range when resuming, and refuses
outright if what the host offers is not the size this build expects.

## Lifecycle

- One process per role. Embedding and chat are never both alive unless
  `--allow-both-servers` is passed and the free-RAM check has cleared the sum.
  Which path ran is recorded in the output as `sidecar_path`.
- A free port is taken by binding a TcpListener to port 0 and reading the port
  back. The listener is dropped before the server is spawned.
- Readiness is `/health` polled until 200, with the child's exit status checked
  on every iteration so a server that dies at load fails fast with its log tail
  rather than at the timeout.
- A second spawn while one is running is refused by the manager, not by luck.
- The child is killed on drop, on panic, and when the parent dies. On Windows
  that last guarantee is a Job Object with
  `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`: the kernel kills the child when the
  parent's handle closes, which covers a hard kill of the parent that no
  user-space handler would survive. On Linux it is `PR_SET_PDEATHSIG` set in
  the child before exec.

  **macOS has neither.** `PR_SET_PDEATHSIG` is a Linux call and there is no
  Darwin equivalent, so the same guarantee is built out of a pipe. The parent
  makes one, keeps the write end (close-on-exec, so no child inherits it), and
  spawns `/bin/sh -c 'cat >/dev/null; kill -9 <pid>'` with the read end as its
  standard input. That shell blocks forever on a pipe nothing writes to. When
  the parent dies — cleanly, by panic, by Force Quit, by `kill -9`, by anything
  at all — the write end closes with it, `cat` reads end-of-file, and the next
  thing that shell does is kill the model server. On an orderly stop the reaper
  is killed first and the server second, so the `kill -9` is never issued
  against a process id that could by then belong to somebody else.
  `sidecar_lifecycle`'s hard-kill test is the same test on all three platforms
  and runs on the macOS runners.
- Servers run below normal priority so the machine stays usable.
