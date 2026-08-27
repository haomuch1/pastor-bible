//! README's "Sources and credits" section must say exactly what the About
//! screen says.
//!
//! P7-prep's pre-release check asked whether the two matched and found there
//! was nothing to match: About carried the full list of sources and the README
//! section was still "Not yet available; filled in at P3." This test is what
//! makes the answer checkable from now on, rather than something a person has
//! to notice.
//!
//! It reads the committed README rather than any generated file. If the two
//! ever drift, this fails in CI's offline gate before an installer is built.

use pastor_bible_core::credits;

fn readme() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../README.md");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", path.display(), e))
}

/// The lines of one `## ` section, up to the next one.
fn section<'a>(text: &'a str, heading: &str) -> &'a str {
    let start = text
        .find(&format!("\n## {}\n", heading))
        .unwrap_or_else(|| panic!("README has no \"## {}\" section", heading));
    let rest = &text[start + 1..];
    let body = &rest[rest.find('\n').unwrap() + 1..];
    match body.find("\n## ") {
        Some(end) => &body[..end],
        None => body,
    }
}

/// The indented block of `name  licence` lines, parsed back into pairs.
///
/// Two or more spaces separate the columns, which is what makes the block line
/// up for a reader; no source name contains a double space.
fn listed_sources(body: &str) -> Vec<(String, String)> {
    body.lines()
        .filter(|l| l.starts_with("    ") && !l.trim().is_empty())
        .filter_map(|l| {
            let l = l.trim();
            let idx = l.find("  ")?;
            Some((
                l[..idx].trim().to_string(),
                l[idx..].trim().to_string(),
            ))
        })
        .collect()
}

#[test]
fn readme_sources_match_the_about_screen() {
    let text = readme();
    let body = section(&text, "Sources and credits");
    let listed = listed_sources(body);

    let expected: Vec<(String, String)> = credits::SOURCES
        .iter()
        .map(|(n, l)| (n.to_string(), l.to_string()))
        .collect();

    assert_eq!(
        listed, expected,
        "\nREADME's \"Sources and credits\" and the About screen disagree.\n\
         The About screen shows pastor_bible_core::credits::SOURCES; the README \
         block is what a reader sees on the page. Change both or neither.\n\
         README lists:  {:#?}\n\
         About shows:   {:#?}\n",
        listed, expected
    );
}

#[test]
fn readme_credits_the_same_authors_the_about_screen_does() {
    let text = readme();
    let body = section(&text, "Sources and credits");

    // The exact string the About screen puts under "Made by".
    let made_by = credits::made_by();
    assert!(
        body.contains(&made_by),
        "README's \"Sources and credits\" section does not contain the About \
         screen's \"Made by\" line, which reads: {}",
        made_by
    );
    assert!(
        body.contains(credits::LICENSE),
        "README's \"Sources and credits\" section does not name the licence the \
         About screen shows, which is {}",
        credits::LICENSE
    );

    // PLAN section 1 requires both names, and requires them in the README's
    // own Authors section too, which is a different section from this one.
    let authors = section(&text, "Authors");
    for name in credits::AUTHORS {
        let bare = name.split(" (").next().unwrap();
        assert!(
            authors.contains(bare),
            "README's Authors section does not credit {}",
            bare
        );
    }
}

/// The sidecar's licences are shipped, not merely listed.
///
/// NOTICE.md claimed the MIT text travelled with the binaries in the archive.
/// It did not: the archive carries only the LLVM licence, for libomp, and
/// llama.cpp's own LICENSE is not in it at all. Both are now vendored and
/// copied into the bundle by tools/fetch_llama.py. This asserts the vendored
/// files are present and are the licences they claim to be, so the claim in
/// NOTICE cannot go false again without a test failing.
#[test]
fn the_sidecar_licence_texts_are_in_the_repository() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../licenses");

    let llama = std::fs::read_to_string(dir.join("llama.cpp-LICENSE.txt"))
        .expect("src-tauri/licenses/llama.cpp-LICENSE.txt is missing; llama.cpp is MIT and its notice ships with the binaries");
    assert!(llama.contains("MIT License"), "that is not the MIT licence");
    assert!(
        llama.contains("The ggml authors"),
        "that is not llama.cpp's copyright line"
    );

    let omp = std::fs::read_to_string(dir.join("LICENSE-LLVM-OpenMP.txt"))
        .expect("src-tauri/licenses/LICENSE-LLVM-OpenMP.txt is missing; libomp.dll ships in the sidecar and carries it");
    assert!(
        omp.contains("LLVM Exceptions"),
        "that is not the LLVM licence"
    );
}
