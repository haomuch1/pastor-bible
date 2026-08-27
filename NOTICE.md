# NOTICE

The Pastor Bible
Copyright 2026 Jared

This product includes software and other material developed by third parties.
This file is the NOTICE file required by the Apache License, Version 2.0, under
which this repository is licensed. See LICENSE for the license text.

Every source used by this project is listed here with its URL, its license, the
date it was retrieved, and, where a file is vendored into this repository, the
SHA-256 checksum of the file as committed.

Using The Pastor Bible places no obligations on you. All attribution obligations
are met by this repository.

## Scope of this file

This file lists only what is actually present in the repository today. It grows
as each phase adds material. Nothing is listed before it exists.

llama.cpp is pinned and fetched by tools/fetch_llama.py but is not vendored
into this repository; it is listed below because the backend runs it. The chat
and embedding model files are likewise not in the repository: the embedding
model is bundled by the installer and the chat model is downloaded on first
run, and both are listed here because they ship to users. No reranker is used.

## Pinned binaries and models

Not vendored into this repository. Listed because they ship to the user or are
run by the backend, and because their licences travel with them.

llama.cpp, the local model server
  Project:   https://github.com/ggml-org/llama.cpp
  Licence:   MIT
  Release:   b10639, commit 5e6a37cb115dc1074e274ac004373f5661909695
  Assets:    llama-b10639-bin-win-cpu-x64.zip
               sha256 3bffee4da688dc404e8599571a6f79ca5f38f42b428c4f74a51945520305284e
             llama-b10639-bin-win-vulkan-x64.zip
               sha256 3fb85c859f2cf90b9626a66e9742baed416c1ceda767d5c906520547b36425ad
             llama-b10639-bin-ubuntu-x64.tar.gz
               sha256 3f928f12abc5aaec2b21e9c8116292910f9f5e76eb2605ae6a9578b0413de626
             llama-b10639-bin-ubuntu-vulkan-x64.tar.gz
               sha256 6168bd9affe15b5cdbf553d70d2f162df5268c50da038000dcd3f0dc537ec7ca
  Fetched by tools/fetch_llama.py, which refuses to unpack a mismatched
  checksum.

  Corrected 2026-08-27: this entry previously said the MIT licence text ships
  beside the binaries in the archive and is included in the installer. Neither
  half was true. The release archives carry no llama.cpp LICENSE at all -- the
  only licence file in them is LICENSE-LLVM-OpenMP, which covers libomp -- and
  the bundler selected files by extension, so no licence text of any kind
  reached the installer. Both texts are now vendored below and copied into
  resources/llama/ by tools/fetch_llama.py, which refuses to assemble a bundle
  without them. The installed sidecar is 25 files.

  The Vulkan archives are the same build as the CPU archives with one extra
  library in them, the Vulkan backend, which ggml loads at run time; measured
  2026-08-27, every other file is byte-identical. The installer therefore ships
  one server and one set of libraries, 23 files, with that backend among them,
  and the two licence texts above beside them, making 25.
  docs/SIDECAR.md has the measurement.

Qwen3-8B, the answering model
  Project:   https://huggingface.co/Qwen/Qwen3-8B-GGUF
  Licence:   Apache-2.0
  File:      Qwen3-8B-Q4_K_M.gguf
  URL:       https://huggingface.co/Qwen/Qwen3-8B-GGUF/resolve/main/Qwen3-8B-Q4_K_M.gguf
  Size:      5,027,783,488 bytes
  sha256     d98cdcbd03e17ce47681435b5150e34c1417f50b5c0019dd560e4882c5745785
  Downloaded once on first run. No account or token is needed.

Qwen3-1.7B, the smaller answering model
  Project:   https://huggingface.co/Qwen/Qwen3-1.7B-GGUF
  Licence:   Apache-2.0
  File:      Qwen3-1.7B-Q8_0.gguf
  URL:       https://huggingface.co/Qwen/Qwen3-1.7B-GGUF/resolve/main/Qwen3-1.7B-Q8_0.gguf
  Size:      1,834,426,016 bytes
  sha256     061b54daade076b5d3362dac252678d17da8c68f07560be70818cace6590cb1a
  Downloaded only if the reader chooses it in Settings.

nomic-embed-text-v1.5, the search model
  Project:   https://huggingface.co/nomic-ai/nomic-embed-text-v1.5-GGUF
  Licence:   Apache-2.0
  File:      nomic-embed-text-v1.5.f16.gguf
  URL:       https://huggingface.co/nomic-ai/nomic-embed-text-v1.5-GGUF/resolve/main/nomic-embed-text-v1.5.f16.gguf
  Size:      274,290,560 bytes
  sha256     f7af6f66802f4df86eda10fe9bbcfc75c39562bed48ef6ace719a251cf1c2fdb
  Bundled with the installer; never downloaded.

## Rust and JavaScript dependencies with attribution terms

Compiled into the program. Every one is a permissive licence that this
repository's own notice satisfies; end users incur no obligations.

  rusqlite, libsqlite3-sys        MIT              SQLite access
  SQLite itself                   public domain    bundled, FTS5 enabled
  serde, serde_json               MIT or Apache-2.0
  regex                           MIT or Apache-2.0
  ureq                            MIT or Apache-2.0   HTTP client
  rustls, rustls-webpki           Apache-2.0, MIT or ISC   TLS for the one
                                                    download
  webpki-roots                    MPL-2.0          Mozilla's root store, used
                                                   unmodified as a data file
  ring                            ISC, MIT and OpenSSL     cryptography for TLS
  sha2, digest                    MIT or Apache-2.0   checksum verification
  rust_xlsxwriter                 MIT or Apache-2.0   writes the spreadsheet
                                                      export
  zip, typed-path                 MIT; MIT or Apache-2.0   the container an
                                                      xlsx file is
  zopfli                          Apache-2.0       deflate, for the same
  zlib-rs                         Zlib             a pure-Rust zlib, for the
                                                   same
  windows-sys                     MIT or Apache-2.0
  libc                            MIT or Apache-2.0
  Tauri, wry, tao                 MIT or Apache-2.0
  React, React DOM                MIT
  Vite, TypeScript                MIT and Apache-2.0

webpki-roots is MPL-2.0, which is file-level copyleft. It is used unmodified
and only as a compiled-in data file; no file of it has been changed, so the
licence's source-availability term is satisfied by the upstream project.

## Vendored files

Files copied into this repository from an external source. Checksums are of the
committed file and can be re-derived with sha256sum. Where a file has been
changed, the change is stated and the upstream checksum is given alongside, as
CC BY 4.0 requires modifications to be indicated.

Contributor Covenant Code of Conduct, version 2.1
  File:      CODE_OF_CONDUCT.md
  URL:       https://www.contributor-covenant.org/version/2/1/code_of_conduct/code_of_conduct.md
  License:   Creative Commons Attribution 4.0 International (CC BY 4.0)
             https://creativecommons.org/licenses/by/4.0/
  Retrieved: 2026-08-26
  SHA-256:   afe11bf27e117489d05850d4d12af620307500bb9687d20af9a234153b94a2c1
  Upstream:  977d781349351fd7c1f076e4c7dc7de2a05b40e12c773542c3815dd4ce7f37ba
  Modified:  Yes. One change, on 2026-08-26: the template's
             "[INSERT CONTACT METHOD]" placeholder was replaced with a real
             reporting route, "by opening an issue on the pastor-bible GitHub
             repository", and the now-redundant preposition "at" preceding it
             was dropped so the sentence reads correctly. No other text differs
             from the upstream 2.1 markdown.

llama.cpp licence, shipped with the sidecar
  File:      src-tauri/licenses/llama.cpp-LICENSE.txt
  URL:       https://raw.githubusercontent.com/ggml-org/llama.cpp/5e6a37cb115dc1074e274ac004373f5661909695/LICENSE
  License:   MIT
  Retrieved: 2026-08-27
  SHA-256:   94f29bbed6a22c35b992c5c6ebf0e7c92f13b836b90f36f461c9cf2f0f1d010d
  Bytes:     1078
  Modified:  No.
  Note:      The LICENSE as it stands at the pinned commit b10639. Vendored
             because the binary release archives do not contain it, and MIT
             requires the notice to travel with any copy of the software. The
             installer is such a copy, so tools/fetch_llama.py puts this file
             in resources/llama/ beside the server it covers.

LLVM OpenMP licence, shipped with the sidecar
  File:      src-tauri/licenses/LICENSE-LLVM-OpenMP.txt
  URL:       In the llama.cpp release archives, as LICENSE-LLVM-OpenMP.
  License:   Apache License 2.0 with LLVM Exceptions
  Retrieved: 2026-08-27
  SHA-256:   fdad1758a9e1f9d5a81e18879b3406772115edc92c24bfa36b70c654f325e8e4
  Bytes:     19741
  Modified:  No, beyond this repository's line-ending normalisation.
  Note:      Copied out of the checksummed archive. It covers libomp.dll, which
             llama-server loads and which the installer therefore ships. Placed
             in resources/llama/ by tools/fetch_llama.py for the same reason as
             the file above.

Apache License, Version 2.0
  File:      LICENSE
  URL:       https://www.apache.org/licenses/LICENSE-2.0.txt
  License:   The Apache License text is itself distributable; see the appendix
             within it for terms of application.
  Retrieved: 2026-08-26
  SHA-256:   2adf983780c9149f459ca3c5a44d7ceaf8ec6018a8069676a9aeb707d4b47c5d
  Note:      Canonical text, with the appendix placeholder
             "Copyright [yyyy] [name of copyright owner]" replaced by
             "Copyright 2026 Jared". No other change.

## Text and study corpora

Vendored unmodified in data/sources/ as the archives exactly as downloaded.
Nothing in these archives is edited. The pipeline reads them in place and
derives index.db from them; the derived database is not committed.

World English Bible, Classic edition
  File:      data/sources/web/eng-web_usfm.zip
  URL:       https://ebible.org/Scriptures/eng-web_usfm.zip
  Details:   https://ebible.org/find/details.php?id=eng-web
  License:   Public domain. The archive's own copr.htm states:
             "The World English Bible is in the Public Domain."
  Trademark: "World English Bible" is a trademark of eBible.org. The condition
             attached to it is that anyone who changes the actual text must not
             call the result the World English Bible. This project ships a
             faithful unmodified copy of the text, so the name is used
             correctly. Verse text is reproduced unmodified: the words and
             punctuation of the translation are not altered, added to, or
             removed. USFM formatting markers are stripped, and the
             translators' footnotes and cross-references are omitted, both
             being apparatus around the text rather than the text itself.
  Retrieved: 2026-08-26
  SHA-256:   2403c879aa6b0c9e5e43a4db6f604f7dd1a1f8f32b959c08c8a1fc32c4833e00
  Note:      Selected as the Classic edition with the full ecumenical book set:
             eBible records its dialect as American and it uses "Yahweh", both
             of which this project verified in the downloaded text rather than
             taking on trust.

Treasury of Scripture Knowledge
  File:      data/sources/tsk/TSK.zip
  URL:       https://crosswire.org/ftpmirror/pub/sword/packages/rawzip/TSK.zip
  License:   Public domain. The module's own mods.d/tsk.conf declares
             "DistributionLicense=Public Domain".
  Retrieved: 2026-08-26
  SHA-256:   6784c7099465995a8e66f02ead82b0bca66603c1bdeaf8332949774b7bfd4293
  Note:      The CrossWire SWORD edition, version 1.4, of the cross-reference
             work compiled by Canne, Browne, Blayney, Scott and others. Not a
             curated or re-edited derivative: the references are the work's own.

Nave's Topical Bible
  File:      data/sources/naves/Nave.zip
  URL:       https://crosswire.org/ftpmirror/pub/sword/packages/rawzip/Nave.zip
  License:   Public domain. The module's own mods.d/nave.conf declares
             "DistributionLicense=Public Domain".
  Retrieved: 2026-08-26
  SHA-256:   52d9b7cde04c2abb5187ae804bcb97d93c7344a1358539f50ebc178ac0c945f0
  Note:      The CrossWire SWORD edition, version 3.0, of the topical index
             compiled by Orville J. Nave and published in the early 1900s.

## Build-time tools

Used only to build index.db on our machines. None of these reaches a user; the
shipped application contains no Python.

pysword                0.2.8    MIT    https://gitlab.com/mothsART/pysword
                       Used solely for its KJV versification table, which the
                       Treasury of Scripture Knowledge module is indexed
                       against and which is needed to address its entries.
pytest                 9.1.1    MIT    https://pytest.org
                       Runs the tests in tests/ against the built index.db.

## Application framework

Tauri
  URL:       https://tauri.app  |  https://github.com/tauri-apps/tauri
  License:   MIT OR Apache-2.0
  Retrieved: 2026-08-26
  Versions:  tauri 2.11.5, tauri-build 2.6.3 (Rust crates, per Cargo.lock)
             @tauri-apps/api 2.11.1, @tauri-apps/cli 2.11.4 (per package-lock.json)
  Note:      Not vendored. Fetched by cargo and npm from crates.io and the npm
             registry, and pinned by Cargo.lock and package-lock.json, both of
             which are committed.

## The spreadsheet export

Added 2026-08-27, when Settings gained a choice between a text file and a
workbook. Pure Rust with no native dependency and no C toolchain, which is the
condition it was chosen under; every part of it is permissively licensed.

rust_xlsxwriter        0.99.0   MIT or Apache-2.0
                                https://github.com/jmcnamara/rust_xlsxwriter

It brings four crates with it, all of them pure Rust:

zip                    8.6.0    MIT              the xlsx container format
typed-path             0.12.3   MIT or Apache-2.0   paths inside that container
zopfli                 0.8.3    Apache-2.0       deflate compression
zlib-rs                0.6.7    Zlib             a zlib implementation in Rust

calamine 0.36.1 (MIT) reads xlsx files and is a development dependency only: the
tests write the workbook with one library and read it back with another, so a
passing test cannot mean the two agreed on a format neither got right. It is not
compiled into the shipped program.

## Frontend dependencies

Declared directly by this project. Not vendored; pinned by package-lock.json.
Retrieved 2026-08-26, with the test runner added 2026-08-27.

React                  19.2.8   MIT              https://react.dev
React DOM              19.2.8   MIT              https://react.dev
Vite                   7.3.6    MIT              https://vite.dev
@vitejs/plugin-react   4.7.0    MIT              https://github.com/vitejs/vite-plugin-react
TypeScript             5.8.3    Apache-2.0       https://www.typescriptlang.org

Development only, never shipped: Vitest 3.2.7 (MIT), jsdom 26.1.0 (MIT),
@testing-library/react 16.3.2 and @testing-library/dom 10.4.1 (both MIT). They
render the window's components in a test, which is the only kind of test that
can tell whether a control a reader needs is actually on the screen.

## Transitive dependencies

The dependency graph as locked today is 472 Rust crates and 216 npm packages,
counted as the entries in the two lockfiles less this project's own. It was 429
and 132 on 2026-08-26; the spreadsheet writer added five crates and the frontend
test runner added most of the rest, and that runner is a development dependency
that no end user ever receives.
These are overwhelmingly MIT, Apache-2.0, BSD, or ISC, and none is vendored into
this repository: each is fetched from its own registry and pinned by the
committed lockfiles, which are the authoritative record of exactly what is used.

The set of crates that actually reaches an end user was fixed in P6, when
packaging settled. What ships is this project's own program, the Tauri runtime
compiled into it, and the llama.cpp sidecar; the Rust and JavaScript
dependencies above are compiled in rather than shipped as files, and every one
of them is permissive.

Third-party licence texts that must travel as files with the binaries they cover
are the sidecar's two, vendored above and installed in resources/llama/. This
project's own Apache-2.0 text is installed by the installer from LICENSE. The
lockfiles remain the authoritative record of the compiled-in set.

## Model files

Chosen in P3 and listed in full under "Pinned binaries and models" at the top of
this file: Qwen3-8B and Qwen3-1.7B, which write the answers, and
nomic-embed-text-v1.5, which does the searching. All three are Apache-2.0, as
the plan requires; no model under a community licence or an acceptable-use
policy is used, so no model's terms reach a reader.

They reach a machine two different ways, and the difference matters for
attribution. The search model is bundled inside the installer, so this project
distributes it and its licence obligations are ours. The answering models are
not distributed by this project: the app downloads the one that is chosen from
the model's own host on first run, by pinned checksum.

Corrected 2026-08-27: this section previously read "None. Models are chosen in
P3 ... are not distributed by this project", which was written before P3 chose
them and was left standing afterwards. It contradicted the top of this same
file, and it was wrong about the search model, which ships in the installer.
