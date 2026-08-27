//! The history as a workbook, read back with a different library.
//!
//! Written with rust_xlsxwriter and read with calamine, so the test cannot pass
//! by agreeing with itself about a format neither of them got right. What it
//! checks is what a reader would check on opening the file: the tabs are there
//! and named after their questions, every passage is a row, the references are
//! spelled the way a reader writes them, and the verse text in the cell is the
//! verse text in index.db.

mod common;

use calamine::{open_workbook, Data, Reader, Xlsx};

use pastor_bible_core::api::*;
use pastor_bible_core::index::Index;
use pastor_bible_core::spreadsheet;
use pastor_bible_core::userdb::UserDb;

fn temp_dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join("pastor-bible-tests").join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn passage(token: &str, verse_ids: Vec<i64>, cited: bool) -> PassageOut {
    PassageOut {
        token: Some(token.to_string()),
        reference: String::new(), // rebuilt from the index on the way out
        verse_ids,
        verses: vec![],
        score: 0.5,
        origins: vec![],
        canon: "protestant".to_string(),
        cited,
        sent: true,
    }
}

fn answer(question: &str, synopsis: &str, passages: Vec<PassageOut>, index_version: &str) -> Answer {
    let cited: Vec<i64> =
        passages.iter().filter(|p| p.cited).flat_map(|p| p.verse_ids.clone()).collect();
    Answer {
        question: question.to_string(),
        canon_mode: "66".to_string(),
        crisis: false,
        crisis_note: None,
        synopsis_markdown: Some(synopsis.to_string()),
        fallback_markdown: None,
        verdict: "ok".to_string(),
        attempts: vec![],
        fallback_used: false,
        cited_tokens: vec![],
        cited_passage_ids: cited,
        deuterocanon_cited: false,
        deuterocanon_footer: None,
        sent_count: passages.len(),
        passages,
        topics: vec![],
        topic_groups: vec![],
        timings: Timings::default(),
        model_id: "Qwen3-8B-Q4_K_M".to_string(),
        embedding_model_id: "nomic-embed-text-v1.5".to_string(),
        index_version: index_version.to_string(),
        prompt_versions: vec![],
        sidecar_path: "concurrent".to_string(),
        peak_ram_mb: None,
        query_mode: "raw".to_string(),
    }
}

/// Three entries: an ordinary one, one whose question carries every character
/// Excel refuses in a sheet name, and one written against another index.
fn fixture(dir: &std::path::Path, index: &Index) -> UserDb {
    let db = UserDb::open(&dir.join("user.db").to_string_lossy()).unwrap();
    let now = index.index_version.clone();

    db.save_answer(&answer(
        "What does the Bible say about anxiety?",
        "## Trust\nDo not be anxious [P1]; the shepherd provides [P2].",
        vec![
            // Deliberately not in canonical order: the sheet must sort them.
            passage("[P1]", vec![55006025, 55006026], true),
            passage("[P2]", vec![19023001], true),
            passage("[P3]", vec![1050015], false),
        ],
        &now,
    ))
    .unwrap();

    db.save_answer(&answer(
        r#"Money: profit / loss [gain] * why? \ how"#,
        "## Money\nIt says so [P1].",
        vec![passage("[P1]", vec![20011002], true)],
        &now,
    ))
    .unwrap();

    db.save_answer(&answer(
        "An answer from an older index",
        "## Older\nIt said so [P1].",
        vec![passage("[P1]", vec![75005007], true)],
        "0.0.1-old",
    ))
    .unwrap();

    db
}

fn text_at(sheet: &calamine::Range<Data>, row: usize, col: usize) -> String {
    sheet.get_value((row as u32, col as u32)).map(|d| d.to_string()).unwrap_or_default()
}

fn all_text(sheet: &calamine::Range<Data>) -> String {
    sheet.rows().flat_map(|r| r.iter().map(|d| d.to_string())).collect::<Vec<_>>().join("\n")
}

#[test]
fn the_workbook_has_a_sheet_for_the_list_and_one_for_every_question() {
    let index = Index::open(&common::require_index()).unwrap();
    let dir = temp_dir("xlsx-sheets");
    let db = fixture(&dir, &index);
    let path = dir.join("history.xlsx").to_string_lossy().into_owned();
    spreadsheet::write(&db, &index, &path).unwrap();

    let book: Xlsx<_> = open_workbook(&path).expect("the file opens as a workbook");
    let names = book.sheet_names().to_vec();
    assert_eq!(names.len(), 4, "one index sheet and three questions: {:?}", names);
    assert_eq!(names[0], "Questions");

    assert!(names[1].starts_with("1. What does the Bible say"), "{:?}", names[1]);
    // Every character Excel refuses is gone, and the name still fits.
    for n in &names {
        assert!(n.len() <= 31, "{:?} is {} characters", n, n.len());
        assert!(!n.is_empty());
        for bad in ['[', ']', ':', '*', '?', '/', '\\'] {
            assert!(!n.contains(bad), "{:?} still has {:?} in it", n, bad);
        }
    }
    assert!(names[2].starts_with("2. Money"), "{:?}", names[2]);
    assert!(names[3].starts_with("3. An answer from an older"), "{:?}", names[3]);
}

#[test]
fn the_questions_sheet_has_one_row_per_entry_with_its_counts() {
    let index = Index::open(&common::require_index()).unwrap();
    let dir = temp_dir("xlsx-index");
    let db = fixture(&dir, &index);
    let path = dir.join("history.xlsx").to_string_lossy().into_owned();
    spreadsheet::write(&db, &index, &path).unwrap();

    let mut book: Xlsx<_> = open_workbook(&path).unwrap();
    let s = book.worksheet_range("Questions").unwrap();

    // Row 4 is the header; the entries follow it, oldest first.
    let headers: Vec<String> = (0..9).map(|c| text_at(&s, 4, c)).collect();
    assert_eq!(
        headers,
        vec![
            "Asked",
            "Question",
            "Books",
            "Model",
            "Bible index",
            "Passages found",
            "Passages cited",
            "Crisis note shown",
            "Sheet",
        ]
    );

    assert_eq!(text_at(&s, 5, 1), "What does the Bible say about anxiety?");
    assert_eq!(text_at(&s, 5, 2), "66 books");
    assert_eq!(text_at(&s, 5, 5), "3", "three passages were sent");
    assert_eq!(text_at(&s, 5, 6), "2", "two of them were cited");
    assert_eq!(text_at(&s, 5, 7), "no", "no crisis note on this one");

    // The Sheet column names the tab the reader should go to, exactly.
    let names = book.sheet_names().to_vec();
    for (i, want) in names.iter().skip(1).enumerate() {
        assert_eq!(&text_at(&s, 5 + i as usize, 8), want);
    }
    assert_eq!(s.rows().count(), 8, "three header lines, a blank, a header row, three entries");
}

#[test]
fn a_question_sheet_lists_every_passage_in_canonical_order_with_its_text() {
    let index = Index::open(&common::require_index()).unwrap();
    let dir = temp_dir("xlsx-entry");
    let db = fixture(&dir, &index);
    let path = dir.join("history.xlsx").to_string_lossy().into_owned();
    spreadsheet::write(&db, &index, &path).unwrap();

    let mut book: Xlsx<_> = open_workbook(&path).unwrap();
    let name = book.sheet_names()[1].clone();
    let s = book.worksheet_range(&name).unwrap();
    let dump = all_text(&s);

    assert!(dump.contains("What does the Bible say about anxiety?"));
    // The answer's markers are resolved into references: "[P1]" is not a thing
    // a reader can look up.
    assert!(
        dump.contains("Do not be anxious (Matthew 6:25-26)"),
        "the answer kept an unresolved marker:\n{}",
        dump
    );
    assert!(!dump.contains("[P1]"), "an unresolved marker reached the sheet");
    // A cell is not markdown: the heading is a heading, not "## Trust".
    assert!(dump.contains("Trust"), "the theme heading is missing:\n{}", dump);
    assert!(!dump.contains("## "), "a markdown heading marker reached a cell");

    // Find the passage table by its header row.
    let head = s
        .rows()
        .position(|r| r.first().map(|d| d.to_string()) == Some("Reference".to_string()))
        .expect("a passage table");
    assert_eq!(
        (0..4).map(|c| text_at(&s, head, c)).collect::<Vec<_>>(),
        vec!["Reference", "Cited", "Deuterocanon", "Verse text"]
    );

    // Canonical order, not the order they were sent: Genesis, then Psalms,
    // then Matthew. They were saved as Matthew, Psalms, Genesis.
    let refs: Vec<String> = (1..=3).map(|i| text_at(&s, head + i, 0)).collect();
    assert_eq!(refs, vec!["Genesis 50:15", "Psalms 23:1", "Matthew 6:25-26"]);

    let cited: Vec<String> = (1..=3).map(|i| text_at(&s, head + i, 1)).collect();
    assert_eq!(cited, vec!["no", "yes", "yes"]);
    let deutero: Vec<String> = (1..=3).map(|i| text_at(&s, head + i, 2)).collect();
    assert_eq!(deutero, vec!["no", "no", "no"]);

    assert_eq!(s.rows().count(), head + 4, "three passages and nothing after them");

    // The verse text in the cell is the verse text in index.db, character for
    // character. Nothing a model wrote reaches this file.
    let want: String =
        index.text_of(&[19023001]).into_iter().map(|(_, t)| t).collect::<Vec<_>>().join(" ");
    assert!(!want.is_empty());
    assert_eq!(text_at(&s, head + 2, 3), want);
    assert!(want.to_lowercase().contains("shepherd"), "{:?}", want);
}

#[test]
fn no_abbreviation_reaches_any_cell_of_the_workbook() {
    let index = Index::open(&common::require_index()).unwrap();
    let dir = temp_dir("xlsx-names");
    let db = fixture(&dir, &index);
    let path = dir.join("history.xlsx").to_string_lossy().into_owned();
    spreadsheet::write(&db, &index, &path).unwrap();

    // Every abbreviation, as a whole word followed by a space, which is how it
    // would appear in a reference. Books whose name is their abbreviation, of
    // which Job is the only one, are not a failure.
    let needles: Vec<String> = index
        .books
        .iter()
        .filter(|b| index.name(b.book_id) != b.abbrev)
        .map(|b| format!("{} ", b.abbrev))
        .collect();

    let mut book: Xlsx<_> = open_workbook(&path).unwrap();
    for name in book.sheet_names().to_vec() {
        let dump = all_text(&book.worksheet_range(&name).unwrap());
        for n in &needles {
            assert!(
                !dump.contains(n.as_str()),
                "sheet {:?} shows the abbreviation {:?}",
                name,
                n.trim()
            );
        }
    }
}

/// An answer written against another index is not re-rendered against this one.
#[test]
fn an_entry_from_another_index_says_so_and_lists_references_only() {
    let index = Index::open(&common::require_index()).unwrap();
    let dir = temp_dir("xlsx-stale");
    let db = fixture(&dir, &index);
    let path = dir.join("history.xlsx").to_string_lossy().into_owned();
    spreadsheet::write(&db, &index, &path).unwrap();

    let mut book: Xlsx<_> = open_workbook(&path).unwrap();
    let name = book.sheet_names()[3].clone();
    let s = book.worksheet_range(&name).unwrap();
    let dump = all_text(&s);

    assert!(
        dump.contains("written against Bible index 0.0.1-old"),
        "the sheet does not say which index it came from:\n{}",
        dump
    );
    assert!(dump.contains("The verse text is not shown"));

    let head = s
        .rows()
        .position(|r| r.first().map(|d| d.to_string()) == Some("Reference".to_string()))
        .expect("a passage table");
    assert_eq!(text_at(&s, head + 1, 0), "1 Peter 5:7", "the reference is still listed");
    assert_eq!(text_at(&s, head + 1, 3), "", "and its text is not");
}

#[test]
fn an_empty_history_still_produces_a_workbook_that_opens() {
    let index = Index::open(&common::require_index()).unwrap();
    let dir = temp_dir("xlsx-empty");
    let db = UserDb::open(&dir.join("user.db").to_string_lossy()).unwrap();
    let path = dir.join("history.xlsx").to_string_lossy().into_owned();
    spreadsheet::write(&db, &index, &path).unwrap();

    let mut book: Xlsx<_> = open_workbook(&path).unwrap();
    assert_eq!(book.sheet_names(), &["Questions"]);
    assert!(all_text(&book.worksheet_range("Questions").unwrap())
        .contains("No questions have been asked yet"));
}
