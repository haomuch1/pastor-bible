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
- Servers run below normal priority so the machine stays usable.
