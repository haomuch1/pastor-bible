//! Read-only access to index.db.
//!
//! The index is bundled with the installer and never written at run time, so it
//! is opened read-only and that is enforced by the URI rather than by
//! convention. FTS5 is required: the keyword half of retrieval is not optional,
//! and a SQLite built without it would silently return nothing rather than
//! fail, so its presence is asserted at open time.

use std::collections::HashMap;

use rusqlite::{Connection, OpenFlags};

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
    pub index_version: String,
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
        let index_version: String = con
            .query_row("SELECT value FROM meta WHERE key = 'index_version'", [], |r| r.get(0))
            .unwrap_or_else(|_| "unknown".to_string());

        Ok(Index { con, books, canon_of, abbrev_of, index_version })
    }

    pub fn canon_of_verse(&self, verse_id: i64) -> &str {
        self.canon_of
            .get(&verse_book(verse_id))
            .map(|s| s.as_str())
            .unwrap_or("protestant")
    }

    pub fn abbrev(&self, book_id: i64) -> &str {
        self.abbrev_of.get(&book_id).map(|s| s.as_str()).unwrap_or("?")
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
