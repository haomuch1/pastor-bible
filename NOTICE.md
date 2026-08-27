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
             llama-b10639-bin-ubuntu-x64.tar.gz
               sha256 3f928f12abc5aaec2b21e9c8116292910f9f5e76eb2605ae6a9578b0413de626
  Fetched by tools/fetch_llama.py, which refuses to unpack a mismatched
  checksum. The MIT licence text ships beside the binaries in the archive and
  is included in the installer.

Qwen3-8B, the default answering model
  Project:   https://huggingface.co/Qwen/Qwen3-8B-GGUF
  Licence:   Apache-2.0
  File:      Qwen3-8B-Q4_K_M.gguf
  sha256     d98cdcbd03e17ce47681435b5150e34c1417f50b5c0019dd560e4882c5745785

Qwen3-1.7B, the smaller answering model
  Project:   https://huggingface.co/Qwen/Qwen3-1.7B-GGUF
  Licence:   Apache-2.0
  File:      Qwen3-1.7B-Q8_0.gguf
  sha256     061b54daade076b5d3362dac252678d17da8c68f07560be70818cace6590cb1a

nomic-embed-text-v1.5, the search model
  Project:   https://huggingface.co/nomic-ai/nomic-embed-text-v1.5-GGUF
  Licence:   Apache-2.0
  File:      nomic-embed-text-v1.5.f16.gguf
  sha256     f7af6f66802f4df86eda10fe9bbcfc75c39562bed48ef6ace719a251cf1c2fdb

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

## Frontend dependencies

Declared directly by this project. Not vendored; pinned by package-lock.json.
Retrieved 2026-08-26.

React                  19.2.8   MIT              https://react.dev
React DOM              19.2.8   MIT              https://react.dev
Vite                   7.3.6    MIT              https://vite.dev
@vitejs/plugin-react   4.7.0    MIT              https://github.com/vitejs/vite-plugin-react
TypeScript             5.8.3    Apache-2.0       https://www.typescriptlang.org

## Transitive dependencies

The dependency graph as locked today is 429 Rust crates and 132 npm packages.
These are overwhelmingly MIT, Apache-2.0, BSD, or ISC, and none is vendored into
this repository: each is fetched from its own registry and pinned by the
committed lockfiles, which are the authoritative record of exactly what is used.

A full transitive licence audit, and the bundling of third-party licence texts
into the shipped installer, is done in P6 alongside packaging, when the set of
crates that actually reaches an end user is fixed. Until then the lockfiles
stand as the record.

## Model files

None. Models are chosen in P3, are required to be Apache-2.0 or MIT, are not
distributed by this project, and are downloaded by the user's machine from the
model's own host on first run. They are listed here once chosen.
