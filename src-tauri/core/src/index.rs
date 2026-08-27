//! Read-only access to index.db.
//!
//! The index is bundled with the installer and never written at run time, so it
//! is opened read-only and that is enforced by the URI rather than by
//! convention. FTS5 is required: the keyword half of retrieval is not optional,
//! and a SQLite built without it would silently return nothing rather than
//! fail, so its presence is asserted at open time.

use std::collections::HashMap;

use rusqlite::{Connection, OpenFlags};

use crate::api::{ChapterOut, ChapterRef, VerseOut};

#[derive(Debug)]
pub struct Book {
    pub book_id: i64,
    pub usfm_code: String,
    pub name: String,
    pub abbrev: String,
    pub canon: String,
}

pub struct Index {
    pub con: Connection,
    pub books: Vec<Book>,
    pub canon_of: HashMap<i64, String>,
    pub abbrev_of: HashMap<i64, String>,
    /// The name a reader recognises, for every book. See `DISPLAY_NAMES`.
    pub name_of: HashMap<i64, String>,
    pub index_version: String,
}

/// The name of each book as a reader writes it, by USFM code.
///
/// index.db has two columns and neither is this. `abbrev` is "1Ki", which is
/// what the TSK and the retrieval fixtures speak and what no reader wants on a
/// citation. `name` is the World English Bible's own running title, which for
/// the same book is "The First Book of Kings", and for one deuterocanonical
/// book is "The Prayer of Manasses King of Judah when He was Held Captive in
/// Babylon". Neither belongs on a chip beside a chapter and verse number.
///
/// So this table exists, keyed on the USFM code because that is the stable
/// identifier and book_id is an ordinal. It is the only book data in this
/// program that is not read from the index, and a test asserts that every book
/// index.db holds has an entry here, so a new book cannot appear without one.
/// Jared chose these forms on 2026-08-27; DECISIONS records why.
pub const DISPLAY_NAMES: &[(&str, &str)] = &[
    ("GEN", "Genesis"),
    ("EXO", "Exodus"),
    ("LEV", "Leviticus"),
    ("NUM", "Numbers"),
    ("DEU", "Deuteronomy"),
    ("JOS", "Joshua"),
    ("JDG", "Judges"),
    ("RUT", "Ruth"),
    ("1SA", "1 Samuel"),
    ("2SA", "2 Samuel"),
    ("1KI", "1 Kings"),
    ("2KI", "2 Kings"),
    ("1CH", "1 Chronicles"),
    ("2CH", "2 Chronicles"),
    ("EZR", "Ezra"),
    ("NEH", "Nehemiah"),
    ("EST", "Esther"),
    ("JOB", "Job"),
    ("PSA", "Psalms"),
    ("PRO", "Proverbs"),
    ("ECC", "Ecclesiastes"),
    ("SNG", "Song of Solomon"),
    ("ISA", "Isaiah"),
    ("JER", "Jeremiah"),
    ("LAM", "Lamentations"),
    ("EZK", "Ezekiel"),
    ("DAN", "Daniel"),
    ("HOS", "Hosea"),
    ("JOL", "Joel"),
    ("AMO", "Amos"),
    ("OBA", "Obadiah"),
    ("JON", "Jonah"),
    ("MIC", "Micah"),
    ("NAM", "Nahum"),
    ("HAB", "Habakkuk"),
    ("ZEP", "Zephaniah"),
    ("HAG", "Haggai"),
    ("ZEC", "Zechariah"),
    ("MAL", "Malachi"),
    // The Deuterocanon, in the WEB's ecumenical order.
    ("TOB", "Tobit"),
    ("JDT", "Judith"),
    ("ESG", "Esther (Greek)"),
    ("WIS", "Wisdom of Solomon"),
    ("SIR", "Sirach"),
    ("BAR", "Baruch"),
    ("1MA", "1 Maccabees"),
    ("2MA", "2 Maccabees"),
    ("1ES", "1 Esdras"),
    ("MAN", "Prayer of Manasseh"),
    ("PS2", "Psalm 151"),
    ("3MA", "3 Maccabees"),
    ("2ES", "2 Esdras"),
    ("4MA", "4 Maccabees"),
    ("DAG", "Daniel (Greek)"),
    ("MAT", "Matthew"),
    ("MRK", "Mark"),
    ("LUK", "Luke"),
    ("JHN", "John"),
    ("ACT", "Acts"),
    ("ROM", "Romans"),
    ("1CO", "1 Corinthians"),
    ("2CO", "2 Corinthians"),
    ("GAL", "Galatians"),
    ("EPH", "Ephesians"),
    ("PHP", "Philippians"),
    ("COL", "Colossians"),
    ("1TH", "1 Thessalonians"),
    ("2TH", "2 Thessalonians"),
    ("1TI", "1 Timothy"),
    ("2TI", "2 Timothy"),
    ("TIT", "Titus"),
    ("PHM", "Philemon"),
    ("HEB", "Hebrews"),
    ("JAS", "James"),
    ("1PE", "1 Peter"),
    ("2PE", "2 Peter"),
    ("1JN", "1 John"),
    ("2JN", "2 John"),
    ("3JN", "3 John"),
    ("JUD", "Jude"),
    ("REV", "Revelation"),
];

pub fn display_name(usfm_code: &str) -> Option<&'static str> {
    DISPLAY_NAMES.iter().find(|(c, _)| *c == usfm_code).map(|(_, n)| *n)
}

pub fn verse_book(verse_id: i64) -> i64 {
    verse_id / 1_000_000
}
pub fn verse_chapter(verse_id: i64) -> i64 {
    (verse_id % 1_000_000) / 1000
}
pub fn verse_num(verse_id: i64) -> i64 {
    verse_id % 1000
}

impl Index {
    pub fn open(path: &str) -> Result<Self, String> {
        let con = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )
        .map_err(|e| format!("cannot open index {}: {}", path, e))?;

        // A SQLite without FTS5 would make every keyword query return nothing
        // and no error. Fail loudly here instead.
        let fts5: i64 = con
            .query_row(
                "SELECT COUNT(*) FROM pragma_compile_options WHERE compile_options = 'ENABLE_FTS5'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if fts5 == 0 {
            return Err("this SQLite build has no FTS5; keyword retrieval would \
                        silently return nothing"
                .to_string());
        }
        // And prove it against the real table rather than trusting the pragma.
        con.query_row("SELECT COUNT(*) FROM verse_fts WHERE verse_fts MATCH 'god'", [], |r| {
            r.get::<_, i64>(0)
        })
        .map_err(|e| format!("verse_fts is not queryable: {}", e))?;

        let mut books = Vec::new();
        {
            let mut st = con
                .prepare("SELECT book_id, usfm_code, name, abbrev, canon FROM books ORDER BY book_id")
                .map_err(|e| e.to_string())?;
            let rows = st
                .query_map([], |r| {
                    Ok(Book {
                        book_id: r.get(0)?,
                        usfm_code: r.get(1)?,
                        name: r.get(2)?,
                        abbrev: r.get(3)?,
                        canon: r.get(4)?,
                    })
                })
                .map_err(|e| e.to_string())?;
            for b in rows {
                books.push(b.map_err(|e| e.to_string())?);
            }
        }
        let canon_of = books.iter().map(|b| (b.book_id, b.canon.clone())).collect();
        let abbrev_of = books.iter().map(|b| (b.book_id, b.abbrev.clone())).collect();
        // A book with no entry in the table falls back to the WEB's own title
        // rather than to an abbreviation: a long name is a nuisance, an
        // abbreviation on a citation is the thing this replaced.
        let name_of = books
            .iter()
            .map(|b| {
                (b.book_id, display_name(&b.usfm_code).map(|n| n.to_string()).unwrap_or_else(|| b.name.clone()))
            })
            .collect();
        let index_version: String = con
            .query_row("SELECT value FROM meta WHERE key = 'index_version'", [], |r| r.get(0))
            .unwrap_or_else(|_| "unknown".to_string());

        Ok(Index { con, books, canon_of, abbrev_of, name_of, index_version })
    }

    pub fn canon_of_verse(&self, verse_id: i64) -> &str {
        self.canon_of
            .get(&verse_book(verse_id))
            .map(|s| s.as_str())
            .unwrap_or("protestant")
    }

    /// The compact form: "1Ki". The TSK's spelling and the retrieval fixtures',
    /// and never what a reader is shown.
    pub fn abbrev(&self, book_id: i64) -> &str {
        self.abbrev_of.get(&book_id).map(|s| s.as_str()).unwrap_or("?")
    }

    /// The name a reader recognises: "1 Kings". Everything shown on a screen or
    /// written into an exported file uses this.
    pub fn name(&self, book_id: i64) -> &str {
        self.name_of.get(&book_id).map(|s| s.as_str()).unwrap_or("?")
    }

    /// One verse, as a reader would write it: "Psalms 23:1".
    pub fn verse_reference(&self, verse_id: i64) -> String {
        format!(
            "{} {}:{}",
            self.name(verse_book(verse_id)),
            verse_chapter(verse_id),
            verse_num(verse_id)
        )
    }

    /// A run of verses, as a reader would write it: "Psalms 23:1-6".
    ///
    /// The ids are a contiguous run inside one chapter, which is how retrieval
    /// builds a passage and how history stores one; the first and the last are
    /// what the reference needs, and nothing else is read.
    pub fn reference_of(&self, verse_ids: &[i64]) -> String {
        let (Some(&first), Some(&last)) = (verse_ids.first(), verse_ids.last()) else {
            return String::new();
        };
        let (name, chapter) = (self.name(verse_book(first)), verse_chapter(first));
        if verse_num(first) == verse_num(last) {
            format!("{} {}:{}", name, chapter, verse_num(first))
        } else {
            format!("{} {}:{}-{}", name, chapter, verse_num(first), verse_num(last))
        }
    }

    /// Verse text, in verse order. Always from the index, never from a model.
    pub fn text_of(&self, verse_ids: &[i64]) -> Vec<(i64, String)> {
        if verse_ids.is_empty() {
            return Vec::new();
        }
        let marks = vec!["?"; verse_ids.len()].join(",");
        let sql = format!(
            "SELECT verse_id, text FROM verses WHERE verse_id IN ({}) ORDER BY verse_id",
            marks
        );
        let mut st = self.con.prepare(&sql).expect("text_of prepare");
        let params: Vec<&dyn rusqlite::ToSql> =
            verse_ids.iter().map(|v| v as &dyn rusqlite::ToSql).collect();
        let rows = st
            .query_map(params.as_slice(), |r| Ok((r.get(0)?, r.get(1)?)))
            .expect("text_of query");
        rows.filter_map(|r| r.ok()).collect()
    }

    /// A whole chapter, with its verses and the chapters either side of it.
    ///
    /// `within_canon` is the reader's canon setting: in 66-book mode the
    /// previous and next chapters step over the Deuterocanon rather than into
    /// it, so a reader who has not asked for those books does not arrive in one
    /// by pressing Next. The chapter itself is returned whatever its canon,
    /// because a citation the reader is following always resolves.
    pub fn chapter(&self, book_id: i64, chapter: i64, within_canon: &str) -> Option<ChapterOut> {
        let name = self.name(book_id).to_string();
        let verses: Vec<VerseOut> = self
            .con
            .prepare_cached(
                "SELECT verse_id, text FROM verses \
                 WHERE verse_id / 1000000 = ? AND (verse_id % 1000000) / 1000 = ? \
                 ORDER BY verse_id",
            )
            .ok()?
            .query_map(rusqlite::params![book_id, chapter], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
            })
            .ok()?
            .filter_map(|r| r.ok())
            .map(|(verse_id, text)| VerseOut {
                verse_id,
                reference: self.verse_reference(verse_id),
                text,
            })
            .collect();
        if verses.is_empty() {
            return None;
        }
        Some(ChapterOut {
            book_id,
            book_name: name.clone(),
            chapter,
            reference: format!("{} {}", name, chapter),
            canon: self.canon_of.get(&book_id).cloned().unwrap_or_else(|| "protestant".into()),
            verses,
            previous: self.step_chapter(book_id, chapter, -1, within_canon),
            next: self.step_chapter(book_id, chapter, 1, within_canon),
        })
    }

    /// The chapter one step away, crossing into the next or previous book when
    /// this one runs out. `None` at the two ends of the canon being read.
    fn step_chapter(
        &self,
        book_id: i64,
        chapter: i64,
        step: i64,
        within_canon: &str,
    ) -> Option<ChapterRef> {
        let readable = |b: &Book| within_canon == "both" || b.canon == "protestant";
        let want = chapter + step;
        if want >= 1 && self.chapter_count(book_id) >= want {
            return Some(ChapterRef {
                book_id,
                chapter: want,
                reference: format!("{} {}", self.name(book_id), want),
            });
        }
        // Into the neighbouring book, skipping any the reader is not reading.
        let mut ids: Vec<i64> =
            self.books.iter().filter(|b| readable(b)).map(|b| b.book_id).collect();
        if step < 0 {
            ids.reverse();
        }
        let here = ids.iter().position(|id| *id == book_id)?;
        let next_book = *ids.get(here + 1)?;
        let n = self.chapter_count(next_book);
        if n == 0 {
            return None;
        }
        let c = if step > 0 { 1 } else { n };
        Some(ChapterRef {
            book_id: next_book,
            chapter: c,
            reference: format!("{} {}", self.name(next_book), c),
        })
    }

    /// How many chapters a book holds.
    pub fn chapter_count(&self, book_id: i64) -> i64 {
        self.con
            .query_row(
                "SELECT COALESCE(MAX((verse_id % 1000000) / 1000), 0) FROM verses \
                 WHERE verse_id / 1000000 = ?",
                [book_id],
                |r| r.get(0),
            )
            .unwrap_or(0)
    }

    /// The verse numbers a chapter actually holds. Used by the verifier: a
    /// chapter-only reference claims all of them.
    pub fn chapter_verses(&self, book_id: i64, chapter: i64) -> Vec<i64> {
        let mut st = self
            .con
            .prepare_cached("SELECT verse FROM verses WHERE book_id = ? AND chapter = ?")
            .expect("chapter_verses prepare");
        let rows = st
            .query_map([book_id, chapter], |r| r.get::<_, i64>(0))
            .expect("chapter_verses query");
        rows.filter_map(|r| r.ok()).collect()
    }
}
