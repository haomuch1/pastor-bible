//! The text compiled into the binary is the text on disk, and the text README
//! promises.
//!
//! P7's clean machine is why this file exists. Nine runtime files resolved
//! through `CARGO_MANIFEST_DIR`, an absolute path on the build machine, and the
//! installed app opened to "cannot read disclaimer.txt: The system cannot find
//! the path specified (os error 3)". They are compiled in now, and these tests
//! stand between the copy in the binary and the copy in the repository so that
//! "single source" keeps meaning something.
//!
//! The parity tests deliberately assert against `builtin::`, not against the
//! files. Asserting against the files would test the repository; asserting
//! against `builtin::` tests what a reader will actually be shown.

use pastor_bible_core::builtin;
use pastor_bible_core::crisis::CrisisMatcher;
use pastor_bible_core::prompts::{self, Prompts};
use pastor_bible_core::session::SELF_TEST_IDS;

fn repo(rel: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join(rel)
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(repo(rel)).unwrap_or_else(|e| panic!("read {}: {}", rel, e))
}

/// Every `>` quote in README, trimmed.
fn readme_quotes() -> Vec<String> {
    read("README.md")
        .lines()
        .filter_map(|l| l.strip_prefix("> "))
        .map(|l| l.trim().to_string())
        .collect()
}

#[test]
fn the_disclaimer_is_the_one_in_the_readme() {
    let shown = builtin::DISCLAIMER.trim();
    assert!(
        readme_quotes().iter().any(|q| q == shown),
        "the disclaimer the app shows is not one README promises. \
         data/disclaimer.txt is the single source for PLAN 9.2 and the two must \
         not drift.\nthe app shows: {}",
        shown
    );
    assert!(
        shown.contains("not a pastor, a counselor, or an authority"),
        "the disclaimer lost the sentence that says what it is not"
    );
}

#[test]
fn the_crisis_note_is_the_one_in_the_readme() {
    let shown = builtin::CRISIS_NOTE.trim();
    assert!(
        readme_quotes().iter().any(|q| q == shown),
        "the crisis note the app shows is not the one README promises. \
         data/crisis_note.txt is the single source for PLAN 9.3.\nthe app shows: {}",
        shown
    );
    assert!(shown.contains("988"), "the crisis note lost its phone number");
}

/// What is compiled in is what is committed. Without this, `include_str!` could
/// be pointed at the wrong file and every other test here would still pass.
#[test]
fn the_compiled_in_copies_are_the_files_on_disk() {
    assert_eq!(builtin::DISCLAIMER, read("data/disclaimer.txt"), "disclaimer.txt");
    assert_eq!(builtin::CRISIS_NOTE, read("data/crisis_note.txt"), "crisis_note.txt");
    assert_eq!(builtin::CRISIS_TERMS, read("data/crisis_terms.txt"), "crisis_terms.txt");
    assert_eq!(
        builtin::EVAL_QUESTIONS,
        read("data/eval/questions.json"),
        "eval/questions.json"
    );
    for (name, text) in builtin::PROMPTS {
        assert_eq!(*text, read(&format!("data/prompts/{}.txt", name)), "prompt {}", name);
    }
}

/// The built-in set and the expected set name the same prompts. If one grows
/// and the other does not, the app loads a prompt nothing checks or checks a
/// prompt nothing loads.
#[test]
fn the_builtin_prompts_are_the_expected_prompts() {
    let mut builtin_names: Vec<&str> = builtin::PROMPTS.iter().map(|(n, _)| *n).collect();
    let mut expected_names: Vec<&str> = prompts::EXPECTED.iter().map(|(n, _)| *n).collect();
    builtin_names.sort_unstable();
    expected_names.sort_unstable();
    assert_eq!(builtin_names, expected_names);
}

/// The built-in prompts load, and say the same thing as the files.
#[test]
fn the_builtin_prompts_load_and_match_the_files() {
    let from_binary = Prompts::builtin();
    let from_disk = Prompts::load(repo("data/prompts").to_str().unwrap())
        .expect("the prompt files load");
    for (name, _) in prompts::EXPECTED {
        assert_eq!(from_binary.body(name), from_disk.body(name), "prompt body {}", name);
        assert_eq!(
            from_binary.version(name),
            from_disk.version(name),
            "prompt version {}",
            name
        );
    }
}

/// The crisis matcher builds from the binary alone, and matches what it should.
/// An empty list would fail here exactly as it would from a file: PLAN 5.8
/// holds that under-triggering is unacceptable.
#[test]
fn the_builtin_crisis_matcher_works() {
    let m = CrisisMatcher::builtin().expect("the built-in crisis list builds");
    assert!(m.is_crisis("I want to kill myself"));
    assert!(!m.is_crisis("What does the Bible say about hope?"));
    assert!(m.note.contains("988"));
}

/// `resolve(None, None)` is what the shipped app calls.
#[test]
fn resolve_with_no_paths_uses_the_builtin_copies() {
    let m = CrisisMatcher::resolve(None, None).expect("resolve to built in");
    assert_eq!(m.note, CrisisMatcher::builtin().unwrap().note);
    let p = Prompts::resolve(None).expect("resolve to built in");
    assert_eq!(p.body("synopsis"), Prompts::builtin().body("synopsis"));
}

/// The self-test's three questions are in the compiled-in evaluation set. This
/// is the read that would have failed next, after the disclaimer.
#[test]
fn the_self_test_questions_are_in_the_builtin_eval_set() {
    let v: serde_json::Value =
        serde_json::from_str(builtin::EVAL_QUESTIONS).expect("the eval set parses");
    for want in SELF_TEST_IDS {
        let found = v["smoke"]
            .as_array()
            .and_then(|a| a.iter().find(|q| q["id"] == want))
            .and_then(|q| q["question"].as_str());
        assert!(found.is_some(), "self-test question {} is not in the eval set", want);
        assert!(!found.unwrap().trim().is_empty(), "self-test question {} is empty", want);
    }
}
