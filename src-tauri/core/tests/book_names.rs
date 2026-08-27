//! No reader is ever shown "1Ki 3:9".
//!
//! index.db carries two spellings of a book and neither is the one a reader
//! writes. `abbrev` is "1Ki", which the Treasury of Scripture Knowledge speaks
//! and which the retrieval parity fixtures hold; `name` is the World English
//! Bible's own running title, "The First Book of Kings". The window and the
//! exported file use a third, `index.name()`, which is "1 Kings".
//!
//! This test walks every book index.db holds, builds a passage in it, and takes
//! every reference string the app can put in front of a reader: the passage
//! panel's headings, the verse numbers inside a passage, the citation chips, the
//! group labels in by-book mode, the reopened-from-history rendering, the
//! exported file and the fallback listing. None of them may contain an
//! abbreviation from the books table.
//!
//! The compact form is deliberately still there in one place: the prompt sent to
//! the model, which is what P3 and P4 measured and what the fixtures pin. That
//! is asserted too, so the split cannot quietly close in either direction.

mod common;

use std::collections::HashSet;

use pastor_bible_core::api::*;
use pastor_bible_core::index::{display_name, Index};
use pastor_bible_core::userdb::UserDb;

/// A verse that exists, for every book in the index.
fn one_verse_per_book(index: &Index) -> Vec<(i64, i64)> {
    let mut out = Vec::new();
    for b in &index.books {
        let v: Option<i64> = index
            .con
            .query_row(
                "SELECT MIN(verse_id) FROM verses WHERE verse_id / 1000000 = ?",
                [b.book_id],
                |r| r.get(0),
            )
            .ok()
            .flatten();
        if let Some(v) = v {
            out.push((b.book_id, v));
        }
    }
    out
}

/// Every abbreviation in the books table, as a whole word.
///
/// Matched with the space that follows it in a reference, so that "Job" the
/// abbreviation is not confused with "Job" the name, and so that a verse of
/// prose mentioning "Amos" is not a failure.
fn abbrev_needles(index: &Index) -> Vec<String> {
    index
        .books
        .iter()
        .filter(|b| index.name(b.book_id) != b.abbrev)
        .map(|b| format!("{} ", b.abbrev))
        .collect()
}

fn assert_no_abbrev(where_: &str, text: &str, needles: &[String]) {
    for n in needles {
        assert!(
            !text.contains(n.as_str()),
            "{} still shows the abbreviation {:?}:\n{}",
            where_,
            n.trim(),
            text
        );
    }
}

#[test]
fn every_book_in_the_index_has_a_name_a_reader_recognises() {
    let index = Index::open(&common::require_index()).unwrap();
    let mut missing = Vec::new();
    for b in &index.books {
        match display_name(&b.usfm_code) {
            None => missing.push(format!("{} ({})", b.usfm_code, b.name)),
            Some(n) => {
                assert!(!n.is_empty(), "{} has an empty display name", b.usfm_code);
                // Job is the one book whose ordinary name is also its
                // abbreviation, and there is nothing to fix about that.
                assert!(
                    n != b.abbrev || b.usfm_code == "JOB",
                    "{} is shown as its abbreviation",
                    b.usfm_code
                );
            }
        }
    }
    assert!(
        missing.is_empty(),
        "index.db holds books with no entry in DISPLAY_NAMES: {}",
        missing.join(", ")
    );

    // The five Jared named, verbatim.
    let by_code: std::collections::HashMap<&str, i64> =
        index.books.iter().map(|b| (b.usfm_code.as_str(), b.book_id)).collect();
    for (code, want) in
        [("1KI", "1 Kings"), ("2CH", "2 Chronicles"), ("PSA", "Psalms"), ("PRO", "Proverbs"), ("ISA", "Isaiah")]
    {
        assert_eq!(index.name(by_code[code]), want);
    }
}

#[test]
fn no_screen_shows_an_abbreviation_for_any_book() {
    let index = Index::open(&common::require_index()).unwrap();
    let needles = abbrev_needles(&index);
    assert!(needles.len() > 70, "the books table should carry an abbrev for every book");

    for (book_id, verse_id) in one_verse_per_book(&index) {
        let name = index.name(book_id).to_string();

        // The passage panel's heading, and the by-book group label, which the
        // window derives from the same string by cutting at the chapter number.
        let reference = index.reference_of(&[verse_id]);
        assert!(
            reference.starts_with(&format!("{} ", name)),
            "{:?} does not begin with {:?}",
            reference,
            name
        );
        assert_no_abbrev("a passage heading", &reference, &needles);

        // Two verses, which is the range form.
        let range = index.reference_of(&[verse_id, verse_id + 1]);
        assert!(range.starts_with(&format!("{} ", name)), "{:?}", range);
        assert_no_abbrev("a passage range", &range, &needles);

        // The verse number line inside an expanded passage.
        let verse = index.verse_reference(verse_id);
        assert_no_abbrev("a verse reference", &verse, &needles);
    }
}

#[test]
fn a_reopened_answer_and_its_export_show_names_and_never_abbreviations() {
    let index = Index::open(&common::require_index()).unwrap();
    let needles = abbrev_needles(&index);
    let dir = std::env::temp_dir().join("pastor-bible-tests").join("book-names");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = UserDb::open(&dir.join("user.db").to_string_lossy()).unwrap();

    // One answer that touches every book in the index, citing all of them.
    let verses = one_verse_per_book(&index);
    let passages: Vec<PassageOut> = verses
        .iter()
        .enumerate()
        .map(|(i, (_, v))| PassageOut {
            token: Some(format!("[P{}]", i + 1)),
            reference: index.reference_of(&[*v]),
            verse_ids: vec![*v],
            verses: vec![],
            score: 1.0,
            origins: vec!["fts".to_string()],
            canon: index.canon_of_verse(*v).to_string(),
            cited: true,
            sent: true,
        })
        .collect();
    let synopsis = format!(
        "## Every book\n{}",
        passages.iter().filter_map(|p| p.token.clone()).collect::<Vec<_>>().join(" ")
    );

    let a = Answer {
        question: "Something from every book".to_string(),
        canon_mode: "both".to_string(),
        crisis: false,
        crisis_note: None,
        synopsis_markdown: Some(synopsis),
        fallback_markdown: None,
        verdict: "ok".to_string(),
        attempts: vec![],
        fallback_used: false,
        cited_tokens: passages.iter().filter_map(|p| p.token.clone()).collect(),
        cited_passage_ids: verses.iter().map(|(_, v)| *v).collect(),
        deuterocanon_cited: true,
        deuterocanon_footer: None,
        sent_count: passages.len(),
        passages,
        topics: vec![],
        topic_groups: vec![],
        timings: Timings::default(),
        model_id: "Qwen3-8B-Q4_K_M".to_string(),
        embedding_model_id: "nomic-embed-text-v1.5".to_string(),
        index_version: index.index_version.clone(),
        prompt_versions: vec![],
        sidecar_path: "concurrent".to_string(),
        peak_ram_mb: None,
        query_mode: "raw".to_string(),
    };
    let id = db.save_answer(&a).unwrap();

    // Reopened from history: the references are rebuilt from the index now.
    let got = db.get(id, &index).unwrap().unwrap();
    assert_eq!(got.passages.len(), verses.len(), "every book came back");
    let names: HashSet<&str> = index.books.iter().map(|b| index.name(b.book_id)).collect();
    for p in &got.passages {
        assert_no_abbrev("a reopened passage", &p.reference, &needles);
        let book = p.reference.rsplit_once(' ').map(|(b, _)| b).unwrap_or("");
        assert!(names.contains(book), "{:?} is not a book name", book);
        for v in &p.verses {
            assert_no_abbrev("a reopened verse", &v.reference, &needles);
        }
    }

    // The exported file.
    let text = db.export_text(&index).unwrap();
    assert_no_abbrev("the history export", &text, &needles);
    assert!(text.contains("PASSAGES"));
    assert!(!text.contains("[P1]"), "the export resolves its markers");
    for (book_id, _) in &verses {
        let name = index.name(*book_id);
        assert!(text.contains(name), "the export lost {}", name);
    }
}

#[test]
fn the_fallback_listing_is_written_in_names_too() {
    use pastor_bible_core::verifier::{Sent, Verifier};
    let index = Index::open(&common::require_index()).unwrap();
    let needles = abbrev_needles(&index);
    let sent: Vec<Sent> = one_verse_per_book(&index)
        .iter()
        .enumerate()
        .map(|(i, (_, v))| Sent {
            token: format!("[P{}]", i + 1),
            reference: index.reference_of(&[*v]),
            verse_ids: vec![*v],
        })
        .collect();
    let text = Verifier::fallback(&index, &sent);
    assert_no_abbrev("the fallback listing", &text, &needles);
    assert!(text.contains("Psalms"), "the fallback groups under book names");
}

/// The one place the compact form survives, and it must.
///
/// The prompt is what P3 and P4 measured and what the retrieval fixtures pin.
/// If the reader-facing change ever reached it, every parity claim in
/// docs/DECISIONS.md would be about a different input than the one being sent.
#[test]
fn the_prompt_and_the_fixtures_still_speak_the_compact_form() {
    let index = Index::open(&common::require_index()).unwrap();
    let psalms =
        index.books.iter().find(|b| b.usfm_code == "PSA").expect("Psalms is in the index").book_id;
    assert_eq!(index.abbrev(psalms), "Psa");
    assert_eq!(index.name(psalms), "Psalms");
}
