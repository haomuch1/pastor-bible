//! PLAN 5.8's crisis matcher.
//!
//! Over-triggering is acceptable; under-triggering is not. The negatives below
//! are therefore chosen to be questions a reader would plausibly ask about
//! anger, grief, death or violence in scripture without being in danger
//! themselves, because those are the sentences a wide list is most likely to
//! catch by accident. The positives are the plain ways someone says it.

mod common;

use pastor_bible_core::crisis::{parse_terms, CrisisMatcher};
use pastor_bible_core::paths;

const POSITIVE: &[&str] = &[
    "I want to kill myself",
    "I have been thinking about suicide",
    "what does the bible say to someone who is suicidal",
    "I want to die and I don't know why",
    "everyone would be better off dead without me",
    "I have been hurting myself again",
    "is self harm a sin",
    "I am going to end my life tonight",
    "I don't want to live anymore",
    "there is nothing to live for",
    "I am so angry I could kill him",
    "I want to hurt someone",
    "what if I hurt my kids",
    "my husband hits me and I am afraid for my life",
    "I can't go on like this",
];

const NEGATIVE: &[&str] = &[
    "What does the Bible say about anxiety and worry?",
    "How should a Christian handle anger?",
    "What does scripture teach about forgiveness?",
    "Where does the Bible talk about grief and mourning?",
    "What happens after death according to the Bible?",
    "Why did Cain kill Abel?",
    "What does the Bible say about war?",
    "How did David defeat Goliath?",
    "What does the Bible say about money and debt?",
    "Is there a verse about patience?",
    "What does the Bible say about marriage?",
    "How many times should I forgive someone?",
    "What does the Bible say about work and rest?",
    "Who wrote the book of Hebrews?",
    "What does the Bible say about raising children?",
];

fn matcher() -> CrisisMatcher {
    CrisisMatcher::load(&paths::crisis_terms(), &paths::crisis_note()).expect("crisis list loads")
}

#[test]
fn the_list_is_populated() {
    let raw = std::fs::read_to_string(paths::crisis_terms()).expect("read crisis_terms.txt");
    let terms = parse_terms(&raw);
    assert!(
        terms.len() >= 40,
        "the crisis list holds {} terms; PLAN 5.8 needs a real list covering harm \
         to self and harm to others, and 40 is the floor",
        terms.len()
    );
    // Both halves must actually be there, not 120 ways of saying one of them.
    assert!(terms.iter().any(|t| t.contains("myself")), "no harm-to-self phrasing");
    assert!(
        terms.iter().any(|t| t.contains("someone") || t.contains("them") || t.contains("him")),
        "no harm-to-others phrasing"
    );
}

#[test]
fn fifteen_positives_and_fifteen_negatives() {
    let m = matcher();
    let mut wrong: Vec<String> = Vec::new();
    for s in POSITIVE {
        if !m.is_crisis(s) {
            wrong.push(format!("MISSED (this is the unacceptable direction): {:?}", s));
        }
    }
    for s in NEGATIVE {
        if let Some(term) = m.matches(s) {
            wrong.push(format!("false positive on {:?} via term {:?}", s, term));
        }
    }
    assert!(wrong.is_empty(), "{} of 30 sentences are wrong:\n{}", wrong.len(), wrong.join("\n"));
}

#[test]
fn a_list_with_no_terms_is_refused() {
    let dir = std::env::temp_dir().join("pastor-bible-crisis-test");
    std::fs::create_dir_all(&dir).unwrap();
    let note = dir.join("note.txt");
    std::fs::write(&note, "a note").unwrap();

    for (name, body) in [("empty.txt", ""), ("comments.txt", "# only a comment\n#\n\n")] {
        let p = dir.join(name);
        std::fs::write(&p, body).unwrap();
        let r = CrisisMatcher::load(&p.to_string_lossy(), &note.to_string_lossy());
        assert!(
            r.is_err(),
            "a crisis list with no terms was accepted from {}. It would match \
             nothing while looking like a working feature.",
            name
        );
    }
}

#[test]
fn the_note_is_the_one_in_the_readme() {
    let m = matcher();
    let readme = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("README.md"),
    )
    .expect("read README.md");
    let quoted: Vec<String> = readme
        .lines()
        .filter_map(|l| l.strip_prefix("> "))
        .map(|l| l.trim().to_string())
        .collect();
    assert!(
        quoted.iter().any(|q| q == &m.note),
        "the crisis note the app shows is not the one README promises. \
         data/crisis_note.txt is the single source and the two must not drift."
    );
    assert!(m.note.contains("988"), "the crisis note lost its phone number");
}

#[test]
fn matching_ignores_case_and_spacing() {
    let m = matcher();
    assert!(m.is_crisis("I  WANT   to\n\tKILL   MYSELF"));
    assert!(!m.is_crisis(""));
}
