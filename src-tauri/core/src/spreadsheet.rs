//! The question history as a workbook.
//!
//! The text export is the copy a reader keeps or prints. This is the copy they
//! sort, filter and hand to someone else: one sheet listing every question, and
//! one sheet per question holding its answer and every passage it rested on,
//! with the verse text in a column beside the reference.
//!
//! The two rules that shape everything else in this program shape this too.
//! Verse text is read from index.db at the moment the file is written, never
//! from anything a model produced. And a reference is spelled the way a reader
//! writes it, so a cell says "1 Kings 3:9" and never "1Ki 3:9".
//!
//! There is a third rule that belongs to this file alone. An entry written
//! against a different version of the Bible index is not rendered against the
//! one installed now: the verse a reference pointed at then may not be the
//! verse it points at now, and a spreadsheet is exactly the artefact somebody
//! will trust without reading the caveat. Such a sheet says so in its first
//! line and lists its references without text.

use std::collections::HashSet;

use rust_xlsxwriter::{Format, FormatAlign, Workbook, Worksheet};

use crate::index::Index;
use crate::userdb::{Entry, UserDb};

/// Excel's own limit. A sheet name longer than this is refused by Excel, not
/// by us, so it is truncated before it is offered.
const SHEET_NAME_MAX: usize = 31;

/// The characters Excel will not accept in a sheet name.
const FORBIDDEN: &[char] = &['[', ']', ':', '*', '?', '/', '\\'];

pub fn write(db: &UserDb, index: &Index, path: &str) -> Result<(), String> {
    let entries = db.entries()?;

    let mut book = Workbook::new();
    let bold = Format::new().set_bold();
    let header = Format::new().set_bold().set_align(FormatAlign::Left);
    let wrap = Format::new().set_text_wrap().set_align(FormatAlign::Top);
    let top = Format::new().set_align(FormatAlign::Top);

    // ---- the index sheet ------------------------------------------------
    let names = sheet_names(&entries);
    {
        let s = book.add_worksheet();
        s.set_name("Questions").map_err(x)?;
        s.write_with_format(0, 0, "THE PASTOR BIBLE — question history", &bold).map_err(x)?;
        s.write(1, 0, "Every answer below cites only passages retrieved from the text.")
            .map_err(x)?;
        s.write(2, 0, "Nothing here has ever left this computer.").map_err(x)?;

        const COLS: [(&str, f64); 9] = [
            ("Asked", 20.0),
            ("Question", 52.0),
            ("Books", 26.0),
            ("Model", 20.0),
            ("Bible index", 12.0),
            ("Passages found", 15.0),
            ("Passages cited", 15.0),
            ("Crisis note shown", 17.0),
            ("Sheet", 34.0),
        ];
        for (c, (title, width)) in COLS.iter().enumerate() {
            let c = c as u16;
            s.write_with_format(4, c, *title, &header).map_err(x)?;
            s.set_column_width(c, *width).map_err(x)?;
        }
        // The header stays put while the reader scrolls their questions.
        s.set_freeze_panes(5, 0).map_err(x)?;

        for (i, e) in entries.iter().enumerate() {
            let r = 5 + i as u32;
            s.write(r, 0, &e.row.asked_at).map_err(x)?;
            s.write(r, 1, &e.row.question).map_err(x)?;
            s.write(r, 2, e.row.canon_mode_label()).map_err(x)?;
            s.write(r, 3, &e.row.model_id).map_err(x)?;
            s.write(r, 4, &e.row.index_version).map_err(x)?;
            s.write(r, 5, e.passages.len() as u32).map_err(x)?;
            s.write(r, 6, e.cited_count() as u32).map_err(x)?;
            s.write(r, 7, yes_no(e.row.crisis_flag)).map_err(x)?;
            // A note rather than a hyperlink: a link into a sheet whose name
            // Excel has truncated or rewritten is a link that breaks quietly,
            // and the name is what the reader needs to find the tab anyway.
            s.write(r, 8, &names[i]).map_err(x)?;
        }
        if entries.is_empty() {
            s.write(5, 0, "No questions have been asked yet.").map_err(x)?;
        }
    }

    // ---- one sheet per entry ---------------------------------------------
    for (i, e) in entries.iter().enumerate() {
        let s = book.add_worksheet();
        s.set_name(&names[i]).map_err(x)?;
        write_entry(s, e, index, &bold, &header, &wrap, &top)?;
    }

    book.save(path).map_err(|err| format!("cannot write {}: {}", path, err))?;
    Ok(())
}

fn write_entry(
    s: &mut Worksheet,
    e: &Entry,
    index: &Index,
    bold: &Format,
    header: &Format,
    wrap: &Format,
    top: &Format,
) -> Result<(), String> {
    // An answer written against another index is not re-rendered against this
    // one. See the note at the top of this file.
    let stale = e.row.index_version != index.index_version;

    s.set_column_width(0, 24.0).map_err(x)?;
    s.set_column_width(1, 9.0).map_err(x)?;
    s.set_column_width(2, 14.0).map_err(x)?;
    s.set_column_width(3, 96.0).map_err(x)?;

    s.write_with_format(0, 0, "Question", bold).map_err(x)?;
    s.write_with_format(0, 1, &e.row.question, wrap).map_err(x)?;
    s.write_with_format(1, 0, "Asked", bold).map_err(x)?;
    s.write(1, 1, &e.row.asked_at).map_err(x)?;
    s.write_with_format(2, 0, "Books", bold).map_err(x)?;
    s.write(2, 1, e.row.canon_mode_label()).map_err(x)?;

    let mut r = 4;
    if e.row.crisis_flag {
        s.write(r, 0, "A crisis note was shown above this answer.").map_err(x)?;
        r += 2;
    }

    s.write_with_format(r, 0, "Answer", bold).map_err(x)?;
    r += 1;
    // The markers are resolved into the references they stood for: "[P3]" means
    // nothing in a cell. One paragraph per row, so the text stays readable
    // without anyone having to widen anything.
    //
    // A theme heading arrives as "## Rest as a Weekly Practice", which is
    // markdown, and a cell is not markdown: the hashes come off and the row is
    // set in bold instead, which is what they were asking for.
    for para in e.answer_resolved(index).split('\n') {
        let para = para.trim();
        if para.is_empty() {
            continue;
        }
        if para.starts_with('#') {
            let heading = para.trim_start_matches('#').trim();
            s.write_with_format(r, 0, heading, bold).map_err(x)?;
        } else {
            s.write_with_format(r, 0, para, wrap).map_err(x)?;
        }
        r += 1;
    }

    r += 1;
    if stale {
        s.write_with_format(
            r,
            0,
            format!(
                "This answer was written against Bible index {}, and the index installed now \
                 is {}. The verse text is not shown, because a reference may no longer point \
                 at the same verse. The references are listed as the answer used them.",
                e.row.index_version, index.index_version
            ),
            wrap,
        )
        .map_err(x)?;
        r += 2;
    }

    let head_row = r;
    for (c, title) in ["Reference", "Cited", "Deuterocanon", "Verse text"].iter().enumerate() {
        s.write_with_format(head_row, c as u16, *title, header).map_err(x)?;
    }
    // Everything above the passages stays put while the reader scrolls them.
    s.set_freeze_panes(head_row + 1, 0).map_err(x)?;
    r += 1;

    // Canonical order: the verse ids ascend with the canon, so the first verse
    // of a passage orders it. The sent order is rank order and is not this.
    let mut passages: Vec<_> = e.passages.iter().collect();
    passages.sort_by_key(|p| p.verse_ids.first().copied().unwrap_or(0));

    for p in passages {
        let first = p.verse_ids.first().copied().unwrap_or(0);
        s.write_with_format(r, 0, index.reference_of(&p.verse_ids), top).map_err(x)?;
        s.write_with_format(r, 1, yes_no(p.cited), top).map_err(x)?;
        s.write_with_format(r, 2, yes_no(index.canon_of_verse(first) == "deutero"), top)
            .map_err(x)?;
        if !stale {
            let text: Vec<String> =
                index.text_of(&p.verse_ids).into_iter().map(|(_, t)| t).collect();
            s.write_with_format(r, 3, text.join(" "), wrap).map_err(x)?;
        }
        r += 1;
    }
    Ok(())
}

/// A tab name per entry: its number, then as much of the question as fits.
///
/// Excel refuses seven characters and anything over thirty-one, and refuses two
/// sheets with the same name. The number goes first because it is what makes
/// them different from each other; a counter is appended in the case where even
/// that is not enough, so the export cannot fail on a name.
fn sheet_names(entries: &[Entry]) -> Vec<String> {
    let mut used: HashSet<String> = HashSet::from(["Questions".to_string()]);
    let mut out = Vec::with_capacity(entries.len());
    for (i, e) in entries.iter().enumerate() {
        let cleaned: String =
            e.row.question.chars().filter(|c| !FORBIDDEN.contains(c) && *c != '\'').collect();
        let base = format!("{}. {}", i + 1, cleaned.trim());
        let mut name = truncate(&base, SHEET_NAME_MAX);
        let mut n = 2;
        while used.contains(&name) {
            let suffix = format!(" ({})", n);
            name = format!("{}{}", truncate(&base, SHEET_NAME_MAX - suffix.len()), suffix);
            n += 1;
        }
        used.insert(name.clone());
        out.push(name);
    }
    out
}

/// Truncate on a character boundary, because a question may be in any language
/// and half a character is not a name.
fn truncate(s: &str, max: usize) -> String {
    let t: String = s.chars().take(max).collect();
    t.trim_end().to_string()
}

fn yes_no(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

fn x(e: rust_xlsxwriter::XlsxError) -> String {
    format!("cannot build the spreadsheet: {}", e)
}
