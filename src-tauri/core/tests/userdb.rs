//! user.db: the only file this program writes.
//!
//! History is the reader's, so the tests are about not losing it and not
//! silently changing it: a saved answer comes back the same, search finds it,
//! deleting removes exactly one, and the export holds what the entries hold.

mod common;

use pastor_bible_core::api::*;
use pastor_bible_core::index::Index;
use pastor_bible_core::userdb::{fts_query, iso8601, UserDb, SCHEMA_VERSION};

fn temp_dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join("pastor-bible-tests").join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn passage(token: &str, reference: &str, verse_ids: Vec<i64>, cited: bool) -> PassageOut {
    PassageOut {
        token: Some(token.to_string()),
        reference: reference.to_string(),
        verse_ids,
        verses: vec![],
        score: 0.5,
        origins: vec![],
        canon: "protestant".to_string(),
        cited,
        sent: true,
    }
}

fn answer(question: &str, verse_ids: Vec<i64>, synopsis: &str) -> Answer {
    let cited = verse_ids.clone();
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
        cited_tokens: vec!["[P1]".to_string()],
        cited_passage_ids: cited,
        deuterocanon_cited: false,
        deuterocanon_footer: None,
        passages: vec![PassageOut {
            token: Some("[P1]".to_string()),
            reference: "Mat 6:25-26".to_string(),
            verse_ids: verse_ids.clone(),
            verses: vec![],
            score: 0.5,
            origins: vec!["fts".to_string()],
            canon: "protestant".to_string(),
            cited: true,
            sent: true,
        }],
        sent_count: 1,
        topics: vec![],
        topic_groups: vec![],
        timings: Timings { total_seconds: 12.5, ..Default::default() },
        model_id: "Qwen3-8B-Q4_K_M".to_string(),
        embedding_model_id: "nomic-embed-text-v1.5".to_string(),
        index_version: "0.2.0".to_string(),
        prompt_versions: vec![],
        sidecar_path: "concurrent".to_string(),
        peak_ram_mb: Some(9001.0),
        query_mode: "raw".to_string(),
    }
}

#[test]
fn a_new_file_is_created_with_its_schema_version() {
    let dir = temp_dir("new");
    let path = dir.join("user.db").to_string_lossy().into_owned();
    let db = UserDb::open(&path).expect("create user.db");
    let v: String = db
        .con
        .query_row("SELECT value FROM meta WHERE key='schema_version'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(v, SCHEMA_VERSION.to_string());
    assert_eq!(db.count(), 0);
    // And opening it again is not a fresh file.
    drop(db);
    let db = UserDb::open(&path).expect("reopen");
    assert_eq!(db.count(), 0);
}

#[test]
fn a_user_db_from_a_newer_app_is_refused_rather_than_damaged() {
    let dir = temp_dir("newer");
    let path = dir.join("user.db").to_string_lossy().into_owned();
    {
        let db = UserDb::open(&path).unwrap();
        db.con
            .execute("UPDATE meta SET value = '99' WHERE key = 'schema_version'", [])
            .unwrap();
    }
    let err = match UserDb::open(&path) {
        Ok(_) => panic!("a newer schema version was accepted"),
        Err(e) => e,
    };
    assert!(err.contains("99"), "{}", err);
    assert!(err.contains("newer version"), "{}", err);
}

#[test]
fn an_answer_round_trips_and_its_passages_come_from_the_index_now() {
    let db_index = common::require_index();
    let index = Index::open(&db_index).unwrap();
    let dir = temp_dir("roundtrip");
    let db = UserDb::open(&dir.join("user.db").to_string_lossy()).unwrap();

    // Matthew 6:25-26.
    let ids = vec![55006025, 55006026];
    let a = answer("What does the Bible say about anxiety?", ids.clone(), "## Trust\nIt says so [P1].");
    let id = db.save_answer(&a).unwrap();
    assert!(id > 0);
    assert_eq!(db.count(), 1);

    let got = db.get(id, &index).unwrap().expect("the entry is there");
    assert_eq!(got.row.question, a.question);
    assert_eq!(got.answer_md, "## Trust\nIt says so [P1].");
    assert_eq!(got.row.canon_mode, "66");
    assert_eq!(got.row.model_id, "Qwen3-8B-Q4_K_M");
    assert_eq!(got.row.cited_count, 2);
    assert!(!got.row.crisis_flag);
    assert!(got.index_note.is_none(), "same index version, so no note");

    // The passages are rendered now, from the index, with real verse text.
    assert_eq!(got.passages.len(), 1, "one sent passage was stored");
    let p = &got.passages[0];
    assert_eq!(p.reference, "Mat 6:25-26");
    // The tokens are rebuilt, so a reopened answer's [P1] resolves to a chip
    // rather than being shown to the reader as "[P1]".
    assert_eq!(p.token.as_deref(), Some("[P1]"), "the [P#] token was not rebuilt");
    assert_eq!(p.verses.len(), 2);
    assert!(p.verses[0].text.len() > 20, "verse text must come from index.db");
    assert!(p.cited);

    // The timings survive as structured data, not as a string.
    assert_eq!(got.timings["total_seconds"].as_f64(), Some(12.5));
}

#[test]
fn an_answer_written_against_another_index_says_so() {
    let db_index = common::require_index();
    let index = Index::open(&db_index).unwrap();
    let dir = temp_dir("indexnote");
    let db = UserDb::open(&dir.join("user.db").to_string_lossy()).unwrap();

    let mut a = answer("Old question", vec![55006025], "## A\n[P1]");
    a.index_version = "0.1.0-old".to_string();
    let id = db.save_answer(&a).unwrap();

    let got = db.get(id, &index).unwrap().unwrap();
    let note = got.index_note.expect("a note about the index version");
    assert!(note.contains("0.1.0-old"), "{}", note);
    assert!(note.contains(&index.index_version), "{}", note);
    // The verses are still shown, from the index that is installed now.
    assert_eq!(got.passages.len(), 1);
    assert_eq!(got.passages[0].verses.len(), 1);
}

#[test]
fn history_lists_newest_first_and_pages() {
    let dir = temp_dir("list");
    let db = UserDb::open(&dir.join("user.db").to_string_lossy()).unwrap();
    for i in 0..5 {
        db.save_answer(&answer(&format!("question {}", i), vec![55006025], "## A\n[P1]")).unwrap();
    }
    let all = db.list(10, 0).unwrap();
    assert_eq!(all.len(), 5);
    assert_eq!(all[0].question, "question 4", "newest first");
    assert_eq!(all[4].question, "question 0");

    let page = db.list(2, 2).unwrap();
    assert_eq!(page.len(), 2);
    assert_eq!(page[0].question, "question 2");
}

#[test]
fn search_finds_by_question_and_by_answer() {
    let dir = temp_dir("search");
    let db = UserDb::open(&dir.join("user.db").to_string_lossy()).unwrap();
    db.save_answer(&answer("What does the Bible say about anxiety?", vec![55006025],
                           "## Trust\nThe text speaks of worry and care [P1].")).unwrap();
    db.save_answer(&answer("How should I forgive?", vec![55006025],
                           "## Mercy\nForgiveness is commanded [P1].")).unwrap();

    assert_eq!(db.search("anxiety", 10).unwrap().len(), 1);
    assert_eq!(db.search("forgive", 10).unwrap().len(), 1, "porter stemming finds forgiveness");
    assert_eq!(db.search("worry", 10).unwrap().len(), 1, "the answer body is indexed too");
    assert_eq!(db.search("nothingatall", 10).unwrap().len(), 0);
    // An empty search is the whole list, not an error.
    assert_eq!(db.search("   ", 10).unwrap().len(), 2);

    // Punctuation a reader might type must not become FTS syntax.
    for hostile in ["anxiety AND", "\"unclosed", "NEAR(", "*", "a OR b", "-"] {
        db.search(hostile, 10).unwrap_or_else(|e| panic!("{:?} errored: {}", hostile, e));
    }
}

#[test]
fn deleting_removes_exactly_one_and_search_forgets_it() {
    let dir = temp_dir("delete");
    let db = UserDb::open(&dir.join("user.db").to_string_lossy()).unwrap();
    let a = db.save_answer(&answer("about anxiety", vec![55006025], "## A\n[P1]")).unwrap();
    let _b = db.save_answer(&answer("about forgiveness", vec![55006025], "## B\n[P1]")).unwrap();

    assert!(db.delete(a).unwrap());
    assert!(!db.delete(a).unwrap(), "deleting twice is not an error, just false");
    assert_eq!(db.count(), 1);
    assert_eq!(db.search("anxiety", 10).unwrap().len(), 0, "the FTS index was not updated");
    assert_eq!(db.search("forgiveness", 10).unwrap().len(), 1);
}

#[test]
fn clearing_removes_everything_and_starts_again_at_one() {
    let dir = temp_dir("clear");
    let db = UserDb::open(&dir.join("user.db").to_string_lossy()).unwrap();
    for i in 0..3 {
        db.save_answer(&answer(&format!("q{}", i), vec![55006025], "## A\n[P1]")).unwrap();
    }
    assert_eq!(db.delete_all().unwrap(), 3);
    assert_eq!(db.count(), 0);
    assert_eq!(db.search("q1", 10).unwrap().len(), 0);
    let id = db.save_answer(&answer("after", vec![55006025], "## A\n[P1]")).unwrap();
    assert_eq!(id, 1, "a cleared history starts again at 1");
}

#[test]
fn the_export_holds_what_the_entries_hold() {
    let dir = temp_dir("export");
    let db = UserDb::open(&dir.join("user.db").to_string_lossy()).unwrap();
    let empty = db.export_text().unwrap();
    assert!(empty.contains("No questions have been asked yet"));

    let mut crisis = answer("I want to give up", vec![55006025], "## Hope\n[P1]");
    crisis.crisis = true;
    db.save_answer(&crisis).unwrap();
    db.save_answer(&answer("What does the Bible say about anxiety?", vec![55006025],
                           "## Trust\nDo not be anxious [P1].")).unwrap();

    let text = db.export_text().unwrap();
    assert!(text.contains("THE PASTOR BIBLE"));
    assert!(text.contains("I want to give up"));
    assert!(text.contains("What does the Bible say about anxiety?"));
    assert!(text.contains("Do not be anxious [P1]."));
    assert!(text.contains("a crisis note was shown"), "a crisis entry says so");
    assert!(text.contains("Qwen3-8B-Q4_K_M"));
    assert!(text.contains("66 books"));
    assert!(text.contains("2 questions."));
    // Plain text: no JSON blobs leaking through.
    assert!(!text.contains("passage_ids"));
    assert!(!text.contains("{\""));
}

#[test]
fn every_sent_passage_keeps_its_own_token() {
    let db_index = common::require_index();
    let index = Index::open(&db_index).unwrap();
    let dir = temp_dir("tokens");
    let db = UserDb::open(&dir.join("user.db").to_string_lossy()).unwrap();

    // Three passages, deliberately not adjacent, cited out of order.
    let mut a = answer("Which passages?", vec![55006025, 55006026], "## A
[P1] [P3]");
    a.passages = vec![
        passage("[P1]", "Mat 6:25-26", vec![55006025, 55006026], true),
        passage("[P2]", "Psa 23:1", vec![19023001], false),
        passage("[P3]", "1Pe 5:7", vec![75005007], true),
    ];
    a.sent_count = 3;
    a.cited_passage_ids = vec![55006025, 55006026, 75005007];
    let id = db.save_answer(&a).unwrap();

    let got = db.get(id, &index).unwrap().unwrap();
    let tokens: Vec<&str> = got.passages.iter().filter_map(|p| p.token.as_deref()).collect();
    assert_eq!(tokens, vec!["[P1]", "[P2]", "[P3]"], "tokens must keep their order");
    let refs: Vec<&str> = got.passages.iter().map(|p| p.reference.as_str()).collect();
    assert_eq!(refs, vec!["Mat 6:25-26", "Psa 23:1", "1Pe 5:7"]);
    assert_eq!(
        got.passages.iter().map(|p| p.cited).collect::<Vec<_>>(),
        vec![true, false, true],
        "the answer cited the first and the third"
    );
    for p in &got.passages {
        assert!(!p.verses.is_empty(), "{} has no text", p.reference);
    }
}

#[test]
fn an_entry_stored_before_the_numbering_was_kept_does_not_invent_one() {
    let db_index = common::require_index();
    let index = Index::open(&db_index).unwrap();
    let dir = temp_dir("legacy");
    let db = UserDb::open(&dir.join("user.db").to_string_lossy()).unwrap();

    let a = answer("Old entry", vec![55006025], "## A
It says so [P1] [P7].");
    let id = db.save_answer(&a).unwrap();
    // Rewrite it in the old flat form, which is what the first build wrote.
    db.con
        .execute(
            "UPDATE history SET passage_ids = ? WHERE id = ?",
            rusqlite::params!["[55006025,55006026,19023001]", id],
        )
        .unwrap();

    let got = db.get(id, &index).unwrap().unwrap();
    assert!(!got.tokens_resolvable, "a flat list cannot yield [P#] numbering");
    assert!(
        got.passages.iter().all(|p| p.token.is_none()),
        "a passage was numbered anyway, which would show the reader a citation the          answer never made"
    );
    // The passages themselves are still recovered and still have their text.
    assert_eq!(got.passages.len(), 2, "two runs of adjacent verses");
    assert!(got.passages.iter().all(|p| !p.verses.is_empty()));
}

#[test]
fn settings_round_trip_and_overwrite() {
    let dir = temp_dir("settings");
    let db = UserDb::open(&dir.join("user.db").to_string_lossy()).unwrap();
    assert!(db.get_setting("canon").is_none());
    db.set_setting("canon", "both").unwrap();
    db.set_setting("model", "standard").unwrap();
    assert_eq!(db.get_setting("canon").as_deref(), Some("both"));
    db.set_setting("canon", "66").unwrap();
    assert_eq!(db.get_setting("canon").as_deref(), Some("66"));
    let all = db.all_settings();
    assert_eq!(all.len(), 2);
    assert_eq!(all["model"], "standard");
}

#[test]
fn timestamps_are_iso8601_utc() {
    assert_eq!(iso8601(0), "1970-01-01T00:00:00Z");
    assert_eq!(iso8601(1_000_000_000), "2001-09-09T01:46:40Z");
    // A leap day, because the civil-from-days conversion is where this breaks.
    assert_eq!(iso8601(1_709_164_800), "2024-02-29T00:00:00Z");
    assert_eq!(iso8601(1_798_761_600), "2027-01-01T00:00:00Z");
    assert_eq!(iso8601(1_774_000_000), "2026-03-20T09:46:40Z");
}

#[test]
fn a_readers_words_become_a_search_and_never_syntax() {
    assert_eq!(fts_query("anxiety"), "\"anxiety\"");
    assert_eq!(fts_query("fear of Yahweh"), "\"fear\" \"of\" \"Yahweh\"");
    assert_eq!(fts_query("  "), "");
    assert_eq!(fts_query("a OR b"), "\"a\" \"OR\" \"b\"");
    assert!(!fts_query("NEAR(a b)").contains('('));
}
