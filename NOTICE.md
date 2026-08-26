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

Not yet present, and therefore deliberately absent from this file: the World
English Bible text, the Treasury of Scripture Knowledge, Nave's Topical Bible,
llama.cpp, and the chat, embedding and reranker models. Each is added in the
phase that vendors or ships it.

## Vendored files

Files copied into this repository unmodified. Checksums are of the committed
file and can be re-derived with sha256sum.

Contributor Covenant Code of Conduct, version 2.1
  File:      CODE_OF_CONDUCT.md
  URL:       https://www.contributor-covenant.org/version/2/1/code_of_conduct/code_of_conduct.md
  License:   Creative Commons Attribution 4.0 International (CC BY 4.0)
             https://creativecommons.org/licenses/by/4.0/
  Retrieved: 2026-08-26
  SHA-256:   977d781349351fd7c1f076e4c7dc7de2a05b40e12c773542c3815dd4ce7f37ba
  Note:      Shipped unmodified, including its "[INSERT CONTACT METHOD]"
             placeholder, which must be replaced with a real reporting address
             before this repository is made public.

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
