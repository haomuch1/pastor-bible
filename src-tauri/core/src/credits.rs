//! Who made this and what it is built from.
//!
//! One list, in one place. The About screen reads it, and a test asserts that
//! README's "Sources and credits" section says the same thing in the same
//! order. P7-prep found the two had never agreed: About carried the full list
//! and the README section was still a placeholder reading "filled in at P3".
//!
//! This is the same move the version took in P6. A fact the reader can see in
//! two places is a fact that will eventually disagree with itself unless one
//! of the two is derived from the other, or a test stands between them.

/// Both, always. PLAN section 1: "Credit: Jared and Claude, both, in README
/// and About screen." Claude cannot hold copyright and is credited as
/// co-author; the copyright is Jared's. NOTICE.md says so too.
pub const AUTHORS: [&str; 2] = ["Jared", "Claude (Anthropic)"];

/// This repository's own licence, not the sources'.
pub const LICENSE: &str = "Apache-2.0";

/// What The Pastor Bible is built from, with the licence each one carries.
///
/// The order is the order PLAN 9.1 item 12 gives: the text, then the two study
/// corpora, then the software, then the models. Not alphabetical — a reader
/// looking at this wants the Bible first.
pub const SOURCES: [(&str, &str); 7] = [
    ("World English Bible, Classic", "public domain"),
    ("Treasury of Scripture Knowledge", "public domain"),
    ("Nave's Topical Bible", "public domain"),
    ("llama.cpp", "MIT"),
    ("Tauri", "MIT or Apache-2.0"),
    ("Qwen3 (answering model)", "Apache-2.0"),
    ("nomic-embed-text-v1.5 (search model)", "Apache-2.0"),
];

/// The About screen's "Made by" line, so the README can be checked against the
/// exact string a reader sees rather than against its ingredients.
pub fn made_by() -> String {
    AUTHORS.join(" and ")
}
