//! Reading a cited passage in its place.
//!
//! A passage is a run of verses inside a chapter, and a run of verses is not
//! always enough to judge what it says. The reading view is the rest of the
//! chapter, read from index.db like everything else the reader is shown, with
//! the chapters either side reachable and the book named the way a reader
//! writes it.

mod common;

use pastor_bible_core::index::Index;

fn book(index: &Index, code: &str) -> i64 {
    index.books.iter().find(|b| b.usfm_code == code).expect(code).book_id
}

#[test]
fn a_chapter_comes_back_whole_with_its_text_from_the_index() {
    let index = Index::open(&common::require_index()).unwrap();
    let psalms = book(&index, "PSA");
    let c = index.chapter(psalms, 23, "66").expect("Psalms 23 exists");

    assert_eq!(c.book_name, "Psalms");
    assert_eq!(c.reference, "Psalms 23");
    assert_eq!(c.chapter, 23);
    assert_eq!(c.canon, "protestant");
    assert_eq!(c.verses.len(), 6, "Psalm 23 has six verses");
    assert_eq!(c.verses[0].reference, "Psalms 23:1");
    assert!(
        c.verses[0].text.to_lowercase().contains("shepherd"),
        "the text is read from index.db: {:?}",
        c.verses[0].text
    );
    // In verse order, with no gaps invented.
    let ids: Vec<i64> = c.verses.iter().map(|v| v.verse_id).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted);
}

#[test]
fn a_chapter_that_does_not_exist_is_none_rather_than_empty() {
    let index = Index::open(&common::require_index()).unwrap();
    assert!(index.chapter(book(&index, "JUD"), 2, "66").is_none(), "Jude has one chapter");
    assert!(index.chapter(book(&index, "PSA"), 0, "66").is_none());
    assert!(index.chapter(9999, 1, "66").is_none());
}

#[test]
fn previous_and_next_walk_the_chapter_and_then_the_book() {
    let index = Index::open(&common::require_index()).unwrap();
    let psalms = book(&index, "PSA");

    let c = index.chapter(psalms, 23, "66").unwrap();
    assert_eq!(c.previous.as_ref().unwrap().reference, "Psalms 22");
    assert_eq!(c.next.as_ref().unwrap().reference, "Psalms 24");

    // The last chapter of a book goes on to the first of the next.
    let last = index.chapter_count(psalms);
    let c = index.chapter(psalms, last, "66").unwrap();
    assert_eq!(c.next.as_ref().unwrap().reference, "Proverbs 1");

    // And the first chapter of a book goes back to the last of the one before.
    let c = index.chapter(book(&index, "PRO"), 1, "66").unwrap();
    let malachi_back = c.previous.as_ref().unwrap();
    assert_eq!(malachi_back.book_id, psalms);
    assert_eq!(malachi_back.chapter, last);

    // Genesis 1 has nothing before it and Revelation's last chapter nothing
    // after it.
    assert!(index.chapter(book(&index, "GEN"), 1, "66").unwrap().previous.is_none());
    let rev = book(&index, "REV");
    assert!(index.chapter(rev, index.chapter_count(rev), "66").unwrap().next.is_none());
}

/// Next never walks a reader into books they did not ask for.
#[test]
fn in_66_book_mode_the_deuterocanon_is_stepped_over_and_not_into() {
    let index = Index::open(&common::require_index()).unwrap();
    let malachi = book(&index, "MAL");
    let last = index.chapter_count(malachi);

    // The Deuterocanon sits between Malachi and Matthew in the index's order.
    let in66 = index.chapter(malachi, last, "66").unwrap();
    assert_eq!(in66.next.as_ref().unwrap().reference, "Matthew 1");

    let both = index.chapter(malachi, last, "both").unwrap();
    assert_eq!(both.next.as_ref().unwrap().reference, "Tobit 1");
}

/// A citation the reader is following always opens, whatever the setting.
#[test]
fn a_deuterocanonical_chapter_opens_in_either_mode_and_carries_its_tag() {
    let index = Index::open(&common::require_index()).unwrap();
    let tobit = book(&index, "TOB");
    for mode in ["66", "both"] {
        let c = index.chapter(tobit, 4, mode).unwrap_or_else(|| panic!("Tobit 4 in {} mode", mode));
        assert_eq!(c.book_name, "Tobit");
        assert_eq!(c.canon, "deutero", "the reading view carries the tag from the index");
        assert!(!c.verses.is_empty());
    }
}
