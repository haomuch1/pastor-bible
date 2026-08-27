//! The text the program is made of, compiled into it.
//!
//! P7's clean machine found the reason this module exists. `paths::repo_root()`
//! is `env!("CARGO_MANIFEST_DIR")`, an absolute path on whoever built the
//! binary, and nine runtime files resolved through it and nothing else. On the
//! build machine they were all there; on a laptop that has never seen the
//! repository the very first one failed, and the app opened to "cannot read
//! disclaimer.txt: The system cannot find the path specified (os error 3)".
//! `strings` on the shipped binary shows the path it was looking in:
//! `...\Haomuch-Programs\The-Pastor-Bible\src-tauri\core`.
//!
//! These files are small, they never change while the program runs, and every
//! one of them is part of what the program *is* rather than data it operates
//! on: the wording the reader is shown, the terms that trigger the crisis note,
//! and the prompts. Compiling them in means there is no path to resolve, so
//! there is nothing about the machine that can make them missing.
//!
//! They stay files on disk. `include_str!` reads them at build time, so
//! data/prompts/ is still versioned and diffable exactly as the P3 decision
//! requires, and data/disclaimer.txt is still the single source README is
//! checked against. What changed is that the copy the reader sees now travels
//! inside the executable instead of being looked up beside it.
//!
//! The three large resources -- index.db, the search model and the sidecar --
//! are not here and must not be. They are bundle resources resolved through
//! Tauri's resource directory, which is the mechanism that already worked.

/// PLAN 9.2, the single source. README quotes it and a test asserts they match.
pub const DISCLAIMER: &str = include_str!("../../../data/disclaimer.txt");

/// PLAN 9.3, likewise.
pub const CRISIS_NOTE: &str = include_str!("../../../data/crisis_note.txt");

/// PLAN 5.8's phrase list. A list that matches nothing is worse than no crisis
/// feature, so `CrisisMatcher` refuses to start on an empty one -- which is
/// what the installed app would have done next, had it got past the disclaimer.
pub const CRISIS_TERMS: &str = include_str!("../../../data/crisis_terms.txt");

/// The prompts, each with its version line. Names match the file stems under
/// data/prompts/ and must match `prompts::EXPECTED`, which a test asserts.
///
/// Three, not the five files in that directory. `summarize_batch` and
/// `summarize_merge` belong to the summarize-the-whole-set mode that P2 planned
/// and nothing has yet wired in; no code loads them. They stay on disk, where
/// the decision that created them can still be seen, and they are not compiled
/// into a binary that would never read them. Wiring that mode in means adding
/// them here and to EXPECTED together.
pub const PROMPTS: [(&str, &str); 3] = [
    ("synopsis", include_str!("../../../data/prompts/synopsis.txt")),
    ("retry", include_str!("../../../data/prompts/retry.txt")),
    ("rewrite", include_str!("../../../data/prompts/rewrite.txt")),
];

/// The evaluation set, from which the self-test takes its three questions.
///
/// The whole file, not the three questions lifted out of it: the point of the
/// self-test is that it asks what a reader would ask rather than something
/// chosen to pass, and it keeps that property only by still being read from the
/// evaluation set. 148 KB inside a 445 MB installer is not worth a build step
/// to avoid.
pub const EVAL_QUESTIONS: &str = include_str!("../../../data/eval/questions.json");
