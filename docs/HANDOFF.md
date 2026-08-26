# HANDOFF

Session: P0 Scaffold
Date: 2026-08-26
Status: P0 COMPLETE

## State

The repository exists at D:\Haomuch-Programs\The-Pastor-Bible, git initialized on
branch main, pushed to https://github.com/haomuch1/pastor-bible, private.

51 files are tracked. The count is from git ls-files, not from a script's own
report. Untracked build output (node_modules, src-tauri/target, dist,
src-tauri/gen/schemas) is present on disk and correctly ignored.

At the root: README.md, LICENSE, NOTICE.md, CODE_OF_CONDUCT.md, .gitignore,
.gitattributes, and the Tauri project files (index.html, package.json,
package-lock.json, tsconfig.json, tsconfig.node.json, vite.config.ts).

docs/ holds PLAN.md, DECISIONS.md, EVAL.md, and this file.

PLAN.md is the approved plan, moved from TPB-Plan.md, unmodified. TPB-Plan.md no
longer exists at the root; it was moved, not copied.

src/ holds the frontend: main.tsx and App.tsx. App.tsx renders one h1 reading
"The Pastor Bible" and nothing else. vite-env.d.ts is the template's type
reference.

src-tauri/ holds the Rust backend: Cargo.toml, Cargo.lock, build.rs,
src/main.rs, src/lib.rs, tauri.conf.json, capabilities/default.json, and the
stock icon set. lib.rs builds the Tauri app and runs it. There is no command
handler and no plugin.

Empty directories carrying .gitkeep, to be populated in later phases:
data/sources, data/eval, pipeline, src-tauri/binaries, src-tauri/resources,
tests, .github/workflows.

data/crisis_terms.txt was deliberately not created; see Flags.

DECISIONS.md holds the 22 entries from plan section 1, seeded verbatim, followed
by the decisions made in this session.

## Verified

Prerequisites, all present on this machine:

    rustc 1.98.0, cargo 1.98.0, rustup 1.29.0
    active toolchain stable-x86_64-pc-windows-msvc, host x86_64-pc-windows-msvc
    cargo resolves on PATH natively, not only via a prepended path
    node v24.18.0, npm 11.16.0, git 2.55.0.windows.2
    gh 2.96.0, authenticated as haomuch1
    Visual Studio Build Tools 2022 17.14.35 with VC.Tools.x86.x64
    WebView2 runtime 151.0.4129.107, machine-wide

The Rust toolchain links, not merely resolves. A hello-world crate was created in
a temp directory, compiled, linked and run, printing "Hello, world!", then the
directory was deleted and its deletion confirmed. This is the check that could
not be run in the previous session, when Rust was absent.

PLAN.md is byte-identical to the approved plan, verified three times by SHA-256:
before the move, after the move, and by downloading the file back from GitHub
after the push and hashing what came down. All three read
f07a7354683ad38a4be0219651a5b3fca23ed6ad534dbc1fed60fcff7ad57239, 24924 bytes.

LICENSE is the canonical Apache-2.0 text fetched from apache.org. It was diffed
against the upstream file with the copyright substitution reversed; the diff was
empty, proving the only change is
"Copyright [yyyy] [name of copyright owner]" to "Copyright 2026 Jared".

CODE_OF_CONDUCT.md is byte-identical to the upstream Contributor Covenant 2.1
markdown, sha256 977d781349351fd7c1f076e4c7dc7de2a05b40e12c773542c3815dd4ce7f37ba.

README.md carries the plan's fourteen sections in the order given by plan 9.1.
The four verbatim blocks, 9.2 disclaimer, 9.3 crisis note, 9.4 Windows warning
and 9.5 stance, were extracted programmatically from PLAN.md rather than retyped,
and each was then confirmed present in README.md by exact whole-line match.

DECISIONS.md section 1 entries were extracted from PLAN.md by the same method and
diffed against the plan; the diff was empty across all 22.

NOTICE.md records only what exists in the repo now. Versions were read from
package-lock.json and Cargo.lock, not assumed: tauri 2.11.5, tauri-build 2.6.3,
@tauri-apps/api 2.11.1, @tauri-apps/cli 2.11.4, React 19.2.8, Vite 7.3.6,
TypeScript 5.8.3, @vitejs/plugin-react 4.7.0. Checksums for the two vendored
files were computed from the files as committed.

tauri.conf.json was verified by parsing the written file, not by trusting the
write: productName "The Pastor Bible", version 0.0.1, identifier
io.github.haomuch1.pastorbible, window title "The Pastor Bible", NSIS installMode
currentUser, bundle targets left at the template default of "all".

Builds:

    npm install    exit 0
    npm run build  exit 0, tsc clean, vite produced dist/
    cargo build    exit 0, zero errors and zero warnings, 1m15s

The built artifact was confirmed on disk: src-tauri/target/debug/pastor-bible.exe,
12,485,632 bytes. Tauri's generated capability schemas were produced in
src-tauri/gen/schemas.

npm run tauri dev was launched, and after 20 seconds the npm process was still
alive and the application process was running. Windows reported that process's
MainWindowTitle as "The Pastor Bible", which proves a real window exists and
carries the configured title. The dev log contained no line matching error, panic
or failed. The processes were then terminated and absence of orphans confirmed by
name for the app, by command line for node running tauri or vite, and by name for
cargo. All three were gone.

The push was confirmed against the remote rather than the local ref: local HEAD
and the commit returned by the GitHub API for branch main are both
df467d76cd244b09f3612ebc6cf246e4ea87d154. The API also confirms private true,
default branch main, and the intended description.

## Not verified

The window's appearance. I can read its title from the process table; I cannot
see it. Confirming that the window opens, is legible, and shows "The Pastor
Bible" is Jared's step. Run npm run tauri dev and look at it.

No installer was built. NSIS installMode currentUser is configured but has never
been executed. Whether it truly installs per-user with no UAC prompt is a P6
question, as is the choice between NSIS and MSI, WebView2 bootstrapping on a
clean machine, and in-place upgrade behaviour. The config value is a stated
intention, not a tested fact.

The release profile was never built. Only cargo's dev profile ran. Release builds
can surface problems debug builds do not.

Linux was not built or tested. There is no Linux machine in this session.

bundle targets is "all", inherited from the template and left alone as
instructed. It has not been exercised and may need narrowing in P6.

The transitive dependency licence position is not audited. 429 Rust crates and
132 npm packages are locked. None is vendored, and the lockfiles are committed,
but the full audit and the bundling of third-party licence texts into the
installer belong to P6.

npm reported that some install lifecycle scripts were not run, esbuild's among
them, under its newer allow-scripts gating. The frontend nonetheless builds and
runs, so nothing was approved and no project or machine configuration was
changed. If a future session sees esbuild misbehave, this is the first thing to
look at.

## Flags for Jared

Appearance is entirely placeholder. The page is a bare h1 in the browser's
default type, in an 800 by 600 window. There is no stylesheet at all; App.css was
deleted along with the template's demo content. Nothing here is a design
proposal, and no aesthetic choice was made on your behalf. It waits on you.

The application icon is still the stock Tauri logo, in every size, including the
Windows .ico that an installer would use. That is an appearance decision and
therefore yours. It should be replaced before anything is packaged in P6.

Per-user install. The NSIS installMode is currentUser, so installing needs no
administrator and writes under the user's AppData. The trade-off, recorded in
DECISIONS.md, is that installing once does not serve every account on a shared
machine. Confirmed or reversed in P6.

The Code of Conduct still contains the literal text "[INSERT CONTACT METHOD]" on
line 40. It was shipped unmodified, as instructed, but the document is not usable
for reporting until a real address is there. This must be filled before the repo
is made public at v1.0.0.

README section 9.4, which the plan requires verbatim, says "We have applied for
free open-source signing." That is not true yet. The SignPath application is P8.
The wording is harmless while the repo is private, but it must be either true or
reworded before the repo goes public. Flagging rather than editing, because the
text is a locked verbatim block and changing it is your call.

Your commit email. The first push was rejected by GitHub with GH007, because the
commit carried your private address and your account blocks pushes that would
publish it. Rather than turning that protection off, the commit was amended to
use your GitHub noreply address, 293447797+haomuch1@users.noreply.github.com. The
protection stays on and your real address stays out of a repository that becomes
public at v1.0.0. This was set locally for this repo only; no global git config
and no GitHub account setting was changed.

Contributor Covenant version. Your instruction said "current version" and also
said its licence is CC BY 4.0. Those no longer describe the same document: 3.0 is
current and is CC BY-SA 4.0, while CC BY 4.0 belongs to 2.1. You chose 2.1 when
asked. NOTICE.md records CC BY 4.0 accordingly.

data/crisis_terms.txt was not created, though plan section 12 lists it. An empty
crisis-term file matches nothing, and plan 5.8 states that under-triggering is
unacceptable while over-triggering is fine. A file that silently matches nothing
is worse than a file that is visibly absent. It is created with real content in
P4, where the matcher is built. Recorded in DECISIONS.md.

The repository is private. Making it public is a deliberate act at v1.0.0, per
the decision logged this session.

## Next session

P1 Ingestion, per plan section 13.

Scope: acquire the sources and verify their licences, parse USFM into the verses,
books and pericopes tables, check the counts, and load TSK and Nave's.
Deliverable: index.db without embeddings, with counts reported and re-derived
from the parsed rows.

Its VERIFY items, from plan section 16, all owned by P1:

  - The WEB Classic USA source URL, its format, and the exact Deuterocanon file
    set. Plan 4.2 lists the expected books including the Orthodox set, and marks
    the exact file set as unconfirmed.
  - The TSK and Nave's dataset sources and their public-domain statements.
  - Verse totals for the parsed WEB. Plan 4.2 names 31,102 for the 66 books as
    the figure to check against, and explicitly warns it may differ by
    versification. Re-derive from the parsed rows. Do not carry the figure over.

Each source acquired gets a NOTICE.md entry at the time it is vendored: name,
URL, licence, retrieval date, and checksum. Sources are structured text only.
No PDFs, per the locked decision.

Read PLAN.md, DECISIONS.md and this file before starting. Do not begin P2.
