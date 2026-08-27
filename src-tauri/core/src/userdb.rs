//! user.db: question history and settings, per PLAN 3.2 and section 8.
//!
//! This is the only file this program ever writes. It lives in the app data
//! directory, survives upgrades and reinstalls, and nothing in it is ever
//! transmitted. index.db, by contrast, is read-only and replaced wholesale by
//! the installer.
//!
//! An entry records the passage ids the answer rested on rather than the
//! passage text, so opening a two-year-old answer renders its verses from the
//! index that is installed now. When that index is a different version from the
//! one the answer was written against, the entry says so rather than pretending
//! the two are the same.

use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::api::{Answer, PassageOut, VerseOut};
use crate::index::{verse_book, verse_chapter, verse_num, Index};

pub const SCHEMA_VERSION: i64 = 2;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS history (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    asked_at      TEXT    NOT NULL,   -- ISO 8601, UTC, seconds
    question      TEXT    NOT NULL,
    canon_mode    TEXT    NOT NULL,
    answer_md     TEXT    NOT NULL,   -- the verified synopsis, or the fallback
    passage_ids   TEXT    NOT NULL,   -- JSON: one array of verse ids per sent
                                     -- passage, in the order they were sent,
                                     -- so [P1]..[Pn] can be rebuilt exactly
    cited_ids     TEXT    NOT NULL,   -- JSON: the verse ids the answer cited
    model_id      TEXT    NOT NULL,
    index_version TEXT    NOT NULL,
    crisis_flag   INTEGER NOT NULL,
    timings       TEXT    NOT NULL,   -- JSON
    verdict       TEXT    NOT NULL,
    fallback_used INTEGER NOT NULL
);

CREATE VIRTUAL TABLE IF NOT EXISTS history_fts USING fts5 (
    question, answer_md,
    content='history', content_rowid='id',
    tokenize='porter unicode61'
);

-- The FTS index is kept in step by the database rather than by whoever
-- remembers to call it.
CREATE TRIGGER IF NOT EXISTS history_ai AFTER INSERT ON history BEGIN
    INSERT INTO history_fts(rowid, question, answer_md)
    VALUES (new.id, new.question, new.answer_md);
END;
CREATE TRIGGER IF NOT EXISTS history_ad AFTER DELETE ON history BEGIN
    INSERT INTO history_fts(history_fts, rowid, question, answer_md)
    VALUES ('delete', old.id, old.question, old.answer_md);
END;
CREATE TRIGGER IF NOT EXISTS history_au AFTER UPDATE ON history BEGIN
    INSERT INTO history_fts(history_fts, rowid, question, answer_md)
    VALUES ('delete', old.id, old.question, old.answer_md);
    INSERT INTO history_fts(rowid, question, answer_md)
    VALUES (new.id, new.question, new.answer_md);
END;

CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryRow {
    pub id: i64,
    pub asked_at: String,
    pub question: String,
    pub canon_mode: String,
    pub model_id: String,
    pub index_version: String,
    pub crisis_flag: bool,
    pub verdict: String,
    pub fallback_used: bool,
    /// The first line or so of the answer, for the sidebar.
    pub preview: String,
    pub cited_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryDetail {
    pub row: HistoryRow,
    pub answer_md: String,
    pub timings: serde_json::Value,
    /// Rendered now, from the index that is installed now.
    pub passages: Vec<PassageOut>,
    /// False for an entry stored before the [P#] numbering was kept. The
    /// answer's citation markers cannot be linked to passages, so they are not
    /// shown as though they could be.
    pub tokens_resolvable: bool,
    /// Set when the answer was written against a different index version.
    pub index_note: Option<String>,
}

pub struct UserDb {
    pub con: Connection,
    pub path: String,
}

/// Seconds since the Unix epoch as `YYYY-MM-DDTHH:MM:SSZ`.
///
/// Hand-rolled rather than pulling in a date library for one format string.
/// The civil-from-days conversion is Howard Hinnant's, which is exact for every
/// date this program will ever see.
pub fn iso8601(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, d, h, mi, s)
}

pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Quote a user's words for FTS5 so that punctuation cannot become syntax.
///
/// A reader typing `anxiety AND "worry` should get a search, not an error, and
/// certainly not a query that means something they did not ask for.
pub fn fts_query(text: &str) -> String {
    let terms: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{}\"", t))
        .collect();
    terms.join(" ")
}

impl UserDb {
    pub fn open(path: &str) -> Result<Self, String> {
        if let Some(dir) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("cannot create {}: {}", dir.display(), e))?;
        }
        let con = Connection::open(path).map_err(|e| format!("cannot open {}: {}", path, e))?;
        con.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| e.to_string())?;
        con.execute_batch(SCHEMA).map_err(|e| format!("cannot create user.db: {}", e))?;

        let found: Option<String> = con
            .query_row("SELECT value FROM meta WHERE key = 'schema_version'", [], |r| r.get(0))
            .optional()
            .map_err(|e| e.to_string())?;
        match found {
            None => {
                con.execute(
                    "INSERT INTO meta (key, value) VALUES ('schema_version', ?)",
                    params![SCHEMA_VERSION.to_string()],
                )
                .map_err(|e| e.to_string())?;
            }
            Some(v) => {
                let have: i64 = v.parse().unwrap_or(0);
                migrate(&con, have)?;
            }
        }
        Ok(UserDb { con, path: path.to_string() })
    }

    // ---- settings --------------------------------------------------------

    pub fn get_setting(&self, key: &str) -> Option<String> {
        self.con
            .query_row("SELECT value FROM settings WHERE key = ?", [key], |r| r.get(0))
            .optional()
            .ok()
            .flatten()
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), String> {
        self.con
            .execute(
                "INSERT INTO settings (key, value) VALUES (?, ?) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn all_settings(&self) -> HashMap<String, String> {
        let mut out = HashMap::new();
        if let Ok(mut st) = self.con.prepare("SELECT key, value FROM settings") {
            if let Ok(rows) = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            {
                for r in rows.flatten() {
                    out.insert(r.0, r.1);
                }
            }
        }
        out
    }

    // ---- history ---------------------------------------------------------

    pub fn save_answer(&self, a: &Answer) -> Result<i64, String> {
        // One entry per sent passage, in token order. A flat list of verse ids
        // would lose which verses were [P1] and which were [P2], and a reopened
        // answer would show the reader a bare "[P19]" it could not resolve.
        let sent: Vec<Vec<i64>> =
            a.passages.iter().filter(|p| p.sent).map(|p| p.verse_ids.clone()).collect();
        let answer_md = a
            .synopsis_markdown
            .clone()
            .or_else(|| a.fallback_markdown.clone())
            .unwrap_or_default();
        self.con
            .execute(
                "INSERT INTO history (asked_at, question, canon_mode, answer_md, passage_ids, \
                 cited_ids, model_id, index_version, crisis_flag, timings, verdict, fallback_used) \
                 VALUES (?,?,?,?,?,?,?,?,?,?,?,?)",
                params![
                    iso8601(now_secs()),
                    a.question,
                    a.canon_mode,
                    answer_md,
                    serde_json::to_string(&sent).unwrap_or_else(|_| "[]".into()),
                    serde_json::to_string(&a.cited_passage_ids).unwrap_or_else(|_| "[]".into()),
                    a.model_id,
                    a.index_version,
                    if a.crisis { 1 } else { 0 },
                    serde_json::to_string(&a.timings).unwrap_or_else(|_| "{}".into()),
                    a.verdict,
                    if a.fallback_used { 1 } else { 0 },
                ],
            )
            .map_err(|e| format!("cannot save the answer: {}", e))?;
        Ok(self.con.last_insert_rowid())
    }

    fn row_from(r: &rusqlite::Row<'_>) -> rusqlite::Result<HistoryRow> {
        let answer: String = r.get("answer_md")?;
        let cited: String = r.get("cited_ids")?;
        let n = serde_json::from_str::<Vec<i64>>(&cited).map(|v| v.len()).unwrap_or(0);
        Ok(HistoryRow {
            id: r.get("id")?,
            asked_at: r.get("asked_at")?,
            question: r.get("question")?,
            canon_mode: r.get("canon_mode")?,
            model_id: r.get("model_id")?,
            index_version: r.get("index_version")?,
            crisis_flag: r.get::<_, i64>("crisis_flag")? != 0,
            verdict: r.get("verdict")?,
            fallback_used: r.get::<_, i64>("fallback_used")? != 0,
            preview: preview_of(&answer),
            cited_count: n,
        })
    }

    /// Newest first, paged.
    pub fn list(&self, limit: usize, offset: usize) -> Result<Vec<HistoryRow>, String> {
        let mut st = self
            .con
            .prepare(
                "SELECT * FROM history ORDER BY id DESC LIMIT ? OFFSET ?",
            )
            .map_err(|e| e.to_string())?;
        let rows = st
            .query_map(params![limit as i64, offset as i64], Self::row_from)
            .map_err(|e| e.to_string())?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())
    }

    pub fn count(&self) -> i64 {
        self.con.query_row("SELECT COUNT(*) FROM history", [], |r| r.get(0)).unwrap_or(0)
    }

    /// FTS5 over question and answer. Newest first among the matches.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<HistoryRow>, String> {
        let q = fts_query(query);
        if q.is_empty() {
            return self.list(limit, 0);
        }
        let mut st = self
            .con
            .prepare(
                "SELECT h.* FROM history h JOIN history_fts f ON f.rowid = h.id \
                 WHERE history_fts MATCH ? ORDER BY h.id DESC LIMIT ?",
            )
            .map_err(|e| e.to_string())?;
        let rows =
            st.query_map(params![q, limit as i64], Self::row_from).map_err(|e| e.to_string())?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())
    }

    /// One entry, with its passages rendered from the index installed now.
    pub fn get(&self, id: i64, index: &Index) -> Result<Option<HistoryDetail>, String> {
        let mut st = self
            .con
            .prepare("SELECT * FROM history WHERE id = ?")
            .map_err(|e| e.to_string())?;
        let found = st
            .query_row([id], |r| {
                Ok((
                    Self::row_from(r)?,
                    r.get::<_, String>("answer_md")?,
                    r.get::<_, String>("passage_ids")?,
                    r.get::<_, String>("cited_ids")?,
                    r.get::<_, String>("timings")?,
                ))
            })
            .optional()
            .map_err(|e| e.to_string())?;
        let Some((row, answer_md, sent_json, cited_json, timings_json)) = found else {
            return Ok(None);
        };

        let (sent, tokens_resolvable) = parse_sent(&sent_json);
        let cited: std::collections::HashSet<i64> =
            serde_json::from_str::<Vec<i64>>(&cited_json).unwrap_or_default().into_iter().collect();

        let index_note = if row.index_version != index.index_version {
            Some(format!(
                "This answer was written against Bible index {}. The index installed now is {}, \
                 so a passage may have moved or may no longer be present. The verses below are \
                 read from the index installed now.",
                row.index_version, index.index_version
            ))
        } else {
            None
        };

        Ok(Some(HistoryDetail {
            row,
            answer_md,
            timings: serde_json::from_str(&timings_json).unwrap_or(serde_json::Value::Null),
            passages: render_passages(index, &sent, &cited, tokens_resolvable),
            tokens_resolvable,
            index_note,
        }))
    }

    pub fn delete(&self, id: i64) -> Result<bool, String> {
        let n = self
            .con
            .execute("DELETE FROM history WHERE id = ?", [id])
            .map_err(|e| e.to_string())?;
        Ok(n > 0)
    }

    pub fn delete_all(&self) -> Result<usize, String> {
        let n = self.con.execute("DELETE FROM history", []).map_err(|e| e.to_string())?;
        // AUTOINCREMENT keeps counting; reset it so a cleared history starts at 1.
        let _ = self.con.execute("DELETE FROM sqlite_sequence WHERE name = 'history'", []);
        Ok(n)
    }

    /// Every entry as one plain-text file. No JSON, no markup a reader has to
    /// decode: this is what someone keeps or prints.
    ///
    /// The index is needed because a stored answer cites `[P3]`, which means
    /// nothing on paper. Each marker is replaced by the reference it stood for,
    /// spelled the way a reader writes it, and the passages are listed
    /// underneath; the references are read from the index installed now,
    /// exactly as they are on screen.
    pub fn export_text(&self, index: &Index) -> Result<String, String> {
        let mut st = self
            .con
            .prepare("SELECT * FROM history ORDER BY id ASC")
            .map_err(|e| e.to_string())?;
        let rows = st
            .query_map([], |r| {
                Ok((
                    Self::row_from(r)?,
                    r.get::<_, String>("answer_md")?,
                    r.get::<_, String>("passage_ids")?,
                ))
            })
            .map_err(|e| e.to_string())?;

        let mut out = String::new();
        out.push_str("THE PASTOR BIBLE - question history\n");
        out.push_str(
            "\nEvery answer below cites only passages that were retrieved from the text.\n\
             Nothing here has ever left this computer.\n",
        );
        let mut n = 0;
        for row in rows {
            let (row, answer_md, sent_json) = row.map_err(|e| e.to_string())?;
            n += 1;
            out.push_str("\n");
            out.push_str(&"-".repeat(72));
            out.push_str(&format!("\n{}  ({})\n\n", row.asked_at, row.canon_mode_label()));
            out.push_str(&format!("QUESTION\n  {}\n\n", row.question));
            if row.crisis_flag {
                out.push_str("  [a crisis note was shown above this answer]\n\n");
            }
            let (sent, tokens_resolvable) = parse_sent(&sent_json);
            let refs: Vec<String> = sent.iter().map(|g| index.reference_of(g)).collect();

            out.push_str("ANSWER\n");
            for line in resolve_tokens(&answer_md, &refs, tokens_resolvable).lines() {
                out.push_str("  ");
                out.push_str(line);
                out.push('\n');
            }
            if !refs.is_empty() {
                out.push_str("\nPASSAGES\n");
                for r in &refs {
                    out.push_str(&format!("  {}\n", r));
                }
            }
            out.push_str(&format!(
                "\n  model {}, Bible index {}, {} passages found\n",
                row.model_id,
                row.index_version,
                refs.len()
            ));
        }
        if n == 0 {
            out.push_str("\nNo questions have been asked yet.\n");
        } else {
            out.push_str(&format!("\n{}\n{} question{}.\n", "-".repeat(72), n,
                                  if n == 1 { "" } else { "s" }));
        }
        Ok(out)
    }
}

impl HistoryRow {
    pub fn canon_mode_label(&self) -> &'static str {
        if self.canon_mode == "both" {
            "66 books and the Deuterocanon"
        } else {
            "66 books"
        }
    }
}

fn preview_of(answer: &str) -> String {
    let first = answer
        .lines()
        .map(|l| l.trim_start_matches('#').trim())
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let mut s: String = first.chars().take(90).collect();
    if first.chars().count() > 90 {
        s.push('.');
        s.push('.');
        s.push('.');
    }
    s
}

/// The sent passages, however they were stored, and whether their [P#]
/// numbering can be trusted.
///
/// The current form is one array of verse ids per passage, in the order they
/// were sent, so [P1]..[Pn] rebuild exactly. Entries written by the very first
/// build of this file hold a flat list of verse ids; those can be regrouped by
/// adjacency, which recovers the passages but NOT which one was [P1]. Numbering
/// them anyway would put a reference in front of a reader that the answer never
/// made, which is the one thing this program must not do, so such an entry
/// reports false and its tokens are left unresolved.
/// `[P3]` on paper means nothing, so it is replaced by what it stood for.
///
/// An entry whose passage numbering was not stored cannot resolve its markers,
/// and they are removed rather than pointed at the wrong passages, which is what
/// the window does with the same entry on screen.
fn resolve_tokens(answer: &str, refs: &[String], resolvable: bool) -> String {
    let re = regex::Regex::new(r"\s*\[P(\d+)\]").expect("token pattern");
    re.replace_all(answer, |c: &regex::Captures| {
        if !resolvable {
            return String::new();
        }
        match c[1].parse::<usize>().ok().and_then(|n| refs.get(n.wrapping_sub(1))) {
            Some(r) => format!(" ({})", r),
            None => c[0].to_string(),
        }
    })
    .into_owned()
}

fn parse_sent(json: &str) -> (Vec<Vec<i64>>, bool) {
    if let Ok(v) = serde_json::from_str::<Vec<Vec<i64>>>(json) {
        return (v, true);
    }
    let mut ids: Vec<i64> = serde_json::from_str::<Vec<i64>>(json).unwrap_or_default();
    ids.sort();
    ids.dedup();
    let mut groups: Vec<Vec<i64>> = Vec::new();
    for id in ids {
        let extend = groups.last().map(|g: &Vec<i64>| {
            let last = *g.last().unwrap();
            verse_book(last) == verse_book(id)
                && verse_chapter(last) == verse_chapter(id)
                && verse_num(id) - verse_num(last) <= 1
        });
        match extend {
            Some(true) => groups.last_mut().unwrap().push(id),
            _ => groups.push(vec![id]),
        }
    }
    (groups, false)
}

/// The sent passages, with their text read from the index installed now and
/// their [P#] tokens rebuilt so that a reopened answer reads like a new one.
fn render_passages(
    index: &Index,
    sent: &[Vec<i64>],
    cited: &std::collections::HashSet<i64>,
    tokens_resolvable: bool,
) -> Vec<PassageOut> {
    sent.iter()
        .enumerate()
        .filter(|(_, g)| !g.is_empty())
        .map(|(i, g)| {
            let g = g.clone();
            let reference = index.reference_of(&g);
            let verses: Vec<VerseOut> = index
                .text_of(&g)
                .into_iter()
                .map(|(vid, text)| VerseOut { verse_id: vid, reference: index.verse_reference(vid), text })
                .collect();
            PassageOut {
                cited: g.iter().any(|v| cited.contains(v)),
                sent: true,
                token: if tokens_resolvable { Some(format!("[P{}]", i + 1)) } else { None },
                reference,
                canon: index.canon_of_verse(g[0]).to_string(),
                verse_ids: g,
                verses,
                score: 0.0,
                origins: Vec::new(),
            }
        })
        .collect()
}

/// Bring an older user.db up to the current schema.
fn migrate(con: &Connection, have: i64) -> Result<(), String> {
    if have == SCHEMA_VERSION {
        return Ok(());
    }
    if have > SCHEMA_VERSION {
        return Err(format!(
            "this user.db is schema version {}, and this version of The Pastor Bible \
             understands version {}. It was written by a newer version of the app.",
            have, SCHEMA_VERSION
        ));
    }
    if have < 2 {
        drop_unnumbered_entries(con)?;
    }
    // Future migrations run here, in order, each bumping the recorded version.
    con.execute(
        "INSERT INTO meta (key, value) VALUES ('schema_version', ?) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![SCHEMA_VERSION.to_string()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Schema 2: delete history entries whose passage numbering was never stored.
///
/// `passage_ids` holds one array of verse ids per sent passage. A short-lived
/// build stored a flat list of verse ids instead, which loses which verses were
/// [P1] and which were [P2], so a reopened answer could only show its citation
/// markers with the numbers stripped out and a notice explaining why. No
/// released version ever wrote that form: the only entries in it are the test
/// questions asked while P5 was being built, on the machine it was built on.
/// They are deleted rather than carried forward, because an answer that cannot
/// show what it rests on is not worth keeping and the notice would outlive its
/// cause. The delete goes through the table so the FTS index follows it.
fn drop_unnumbered_entries(con: &Connection) -> Result<usize, String> {
    let mut doomed: Vec<i64> = Vec::new();
    {
        let mut st = con
            .prepare("SELECT id, passage_ids FROM history")
            .map_err(|e| e.to_string())?;
        let rows = st
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (id, json) = row.map_err(|e| e.to_string())?;
            if !parse_sent(&json).1 {
                doomed.push(id);
            }
        }
    }
    for id in &doomed {
        con.execute("DELETE FROM history WHERE id = ?", [id]).map_err(|e| e.to_string())?;
    }
    Ok(doomed.len())
}
