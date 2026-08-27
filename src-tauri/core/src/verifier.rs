//! The mechanical citation guarantee, specified in docs/VERIFIER.md.
//!
//! A port of pipeline/verifier.py. The 35 test vectors in that file are the
//! shared contract and this implementation runs them too; core/tests/ holds
//! them as a fixture so neither side can quietly diverge from the other.
//!
//! Errors here are asymmetric. Missing a fabricated reference puts a false
//! citation in front of a reader. Flagging a real phrase costs one retry. The
//! rules lean towards detection, and the non-detection list keeps that lean
//! from mangling ordinary prose.

use std::collections::{HashMap, HashSet};

use regex::Regex;

use crate::index::Index;
use crate::tsk_abbrev::TSK_ABBREV;

/// Book names that are also ordinary English words. Matched case-sensitively,
/// so "he acts 2 ways" is not read as a citation of Acts.
pub const AMBIGUOUS: &[&str] = &[
    "job",
    "mark",
    "acts",
    "numbers",
    "judges",
    "kings",
    "song",
    "songs",
    "revelation",
    "chronicles",
    "romans",
    "hebrews",
    "lamentations",
    "proverbs",
    "psalm",
    "psalms",
    "james",
    "philemon",
    "ruth",
    "wisdom",
];

/// A number followed by one of these is a count, not a chapter.
pub const UNIT_NOUNS: &[&str] = &[
    "times", "time", "days", "day", "years", "year", "months", "month", "weeks", "week", "hours",
    "hour", "people", "men", "women", "sons", "daughters", "tribes", "thousand", "hundred",
    "million", "percent", "degrees",
];

const ORDINAL_PREFIX: &[(&str, &str)] = &[
    ("first", "1"),
    ("1st", "1"),
    ("i", "1"),
    ("1", "1"),
    ("second", "2"),
    ("2nd", "2"),
    ("ii", "2"),
    ("2", "2"),
    ("third", "3"),
    ("3rd", "3"),
    ("iii", "3"),
    ("3", "3"),
];

/// Common English names of books that neither the WEB's own long titles nor the
/// TSK abbreviation table spells out. Without these, "Song of Solomon 3:1" and
/// every deuterocanonical reference are invisible to Rule B.
const ALIASES: &[(&str, &[&str])] = &[
    ("SNG", &["Song of Songs", "Song of Solomon", "Canticles"]),
    ("ACT", &["Acts of the Apostles"]),
    ("WIS", &["Wisdom of Solomon", "Wisdom"]),
    ("SIR", &["Ecclesiasticus", "Sirach", "Ben Sira"]),
    ("MAN", &["Prayer of Manasseh", "Prayer of Manasses"]),
    ("ESG", &["Greek Esther"]),
    ("DAG", &["Greek Daniel"]),
    ("1ES", &["1 Esdras"]),
    ("2ES", &["2 Esdras"]),
    ("1MA", &["1 Maccabees"]),
    ("2MA", &["2 Maccabees"]),
    ("3MA", &["3 Maccabees"]),
    ("4MA", &["4 Maccabees"]),
];

pub fn norm(tok: &str) -> String {
    tok.chars().filter(|c| !c.is_whitespace() && *c != '.').flat_map(|c| c.to_lowercase()).collect()
}

fn is_ambiguous(key: &str) -> bool {
    AMBIGUOUS.contains(&key)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Token,
    Reference,
}

impl Kind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Kind::Token => "token",
            Kind::Reference => "reference",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Violation {
    pub kind: Kind,
    pub text: String,
    pub reason: String,
    /// Byte offsets, which is what `strip` needs.
    pub span: (usize, usize),
    /// Character offsets, which is what Python's re reports. The two differ
    /// the moment an answer contains an en dash or a curly quote, and the
    /// parity check against the Python records compares these.
    pub char_span: (usize, usize),
}

fn char_span(text: &str, span: (usize, usize)) -> (usize, usize) {
    let start = text[..span.0].chars().count();
    (start, start + text[span.0..span.1].chars().count())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Ok,
    Violation,
}

impl Verdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Verdict::Ok => "ok",
            Verdict::Violation => "violation",
        }
    }
}

/// One passage as it was sent to the model.
#[derive(Clone, Debug)]
pub struct Sent {
    pub token: String,
    pub reference: String,
    pub verse_ids: Vec<i64>,
}

struct BookNames {
    by_name: HashMap<String, i64>,
    /// The written form each normalised key came from, so the reference pattern
    /// can be rebuilt with its spaces intact.
    written_of: HashMap<String, String>,
}

impl BookNames {
    fn new(index: &Index) -> Self {
        let mut by_name: HashMap<String, i64> = HashMap::new();
        let mut written_of: HashMap<String, String> = HashMap::new();
        let mut insert = |key: String, written: String, book_id: i64| {
            if !by_name.contains_key(&key) {
                by_name.insert(key.clone(), book_id);
                written_of.insert(key, written);
            }
        };
        for b in &index.books {
            for variant in variants(&b.usfm_code, &b.name, &b.abbrev) {
                insert(norm(&variant), variant, b.book_id);
            }
        }
        let by_code: HashMap<&str, i64> =
            index.books.iter().map(|b| (b.usfm_code.as_str(), b.book_id)).collect();
        for (abbr, code) in TSK_ABBREV {
            if let Some(&bid) = by_code.get(code) {
                insert(norm(abbr), abbr.to_string(), bid);
            }
        }
        BookNames { by_name, written_of }
    }

    fn lookup(&self, written: &str) -> Option<i64> {
        let key = norm(written);
        if let Some(&b) = self.by_name.get(&key) {
            return Some(b);
        }
        // "First Corinthians", "I Corinthians", "1st Corinthians" all mean
        // 1 Corinthians. Rewrite the prefix to its digit and try again. The
        // rewrite is accepted only when the result names a real book, so
        // "Isaiah" is not mangled into "1saiah".
        let mut prefixes: Vec<&(&str, &str)> = ORDINAL_PREFIX.iter().collect();
        prefixes.sort_by_key(|(w, _)| std::cmp::Reverse(w.len()));
        for (word, digit) in prefixes {
            if key.starts_with(word) && key.len() > word.len() {
                let rewritten = format!("{}{}", digit, &key[word.len()..]);
                if let Some(&b) = self.by_name.get(&rewritten) {
                    return Some(b);
                }
            }
        }
        None
    }
}

fn variants(code: &str, name: &str, abbrev: &str) -> Vec<String> {
    let mut out = vec![code.to_string(), abbrev.to_string()];
    // The long name is "The First Book of Moses, Commonly Called Genesis"; the
    // useful part is the last word or two.
    let tail = match name.find("Called ") {
        Some(i) => name[i + "Called ".len()..].trim().to_string(),
        None => name.trim().to_string(),
    };
    out.push(tail.clone());
    out.push(name.to_string());
    // "The Song of Solomon" is how the WEB titles the book; "Song of Solomon"
    // is how anyone writes it.
    for v in [&tail, &name.to_string()] {
        if v.len() > 4 && v[..4].eq_ignore_ascii_case("the ") {
            out.push(v[4..].to_string());
        }
    }
    for (c, names) in ALIASES {
        if *c == code {
            out.extend(names.iter().map(|s| s.to_string()));
        }
    }
    // Numeric-prefixed books: 1SA -> "1Samuel", "1 Samuel", "first Samuel",
    // "i Samuel", all normalising to the same key.
    let first = code.chars().next().unwrap_or(' ');
    if ('1'..='3').contains(&first) && !tail.is_empty() {
        let base = tail.trim_start_matches(['1', '2', '3']).trim_start().to_string();
        let words: [&str; 2] = match first {
            '1' => ["first", "i"],
            '2' => ["second", "ii"],
            _ => ["third", "iii"],
        };
        let mut prefixes = vec![first.to_string(), format!("{} ", first)];
        prefixes.extend(words.iter().map(|w| format!("{} ", w)));
        for p in prefixes {
            out.push(format!("{}{}", p, base));
        }
    }
    out.into_iter().filter(|v| !v.is_empty()).collect()
}

const ORDINAL_ALT: &[(&str, &str)] =
    &[("1", "First|1st|I"), ("2", "Second|2nd|II"), ("3", "Third|3rd|III")];

/// A permissive pattern for one written book name.
///
/// The name is matched word by word so that a multi-word title keeps its
/// spaces. Building the pattern from the normalised key instead, which has the
/// spaces stripped, is what let "Song of Solomon 3:1" and every
/// deuterocanonical reference slip past Rule B before 2026-08-26.
fn book_alternative(written: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (i, tok) in written.split_whitespace().enumerate() {
        let first = tok.chars().next().unwrap_or(' ');
        if i == 0 && ('1'..='3').contains(&first) {
            let rest = &tok[first.len_utf8()..];
            let alt = ORDINAL_ALT.iter().find(|(d, _)| d.chars().next() == Some(first)).unwrap().1;
            let head = format!("(?:{}|{})", first, alt);
            if rest.is_empty() {
                parts.push(head);
            } else {
                parts.push(format!("{}\\s*\\.?\\s*{}", head, regex::escape(rest)));
            }
        } else {
            parts.push(regex::escape(tok));
        }
    }
    // A full stop may follow any word of an abbreviated name.
    parts.join("\\.?\\s+")
}

fn build_reference_re(books: &BookNames) -> Regex {
    let mut keys: Vec<&String> = books.by_name.keys().collect();
    // Longest first, and by the key itself for equal lengths so the alternation
    // is built in one order on every run.
    keys.sort_by(|a, b| b.len().cmp(&a.len()).then(a.cmp(b)));
    let alts: Vec<String> = keys
        .into_iter()
        .map(|k| book_alternative(books.written_of.get(k).map(|s| s.as_str()).unwrap_or(k)))
        .collect();
    let body = alts.join("|");
    let pattern = format!(
        r"(?i)\b(?P<book>{})\s*\.?\s+(?P<chapter>\d{{1,3}})(?:\s*[:.]\s*(?P<verse>\d{{1,3}})(?:\s*[-\u{{2013}}\u{{2014}}]\s*(?P<verse_end>\d{{1,3}}))?)?",
        body
    );
    regex::RegexBuilder::new(&pattern)
        .size_limit(64 * 1024 * 1024)
        .dfa_size_limit(64 * 1024 * 1024)
        .build()
        .expect("reference pattern")
}

pub struct Verifier {
    books: BookNames,
    ref_re: Regex,
    token_re: Regex,
    after_re: Regex,
}

impl Verifier {
    pub fn new(index: &Index) -> Self {
        let books = BookNames::new(index);
        let ref_re = build_reference_re(&books);
        Verifier {
            books,
            ref_re,
            token_re: Regex::new(r"\[P(\d+)\]").unwrap(),
            after_re: Regex::new(r"^\s+([A-Za-z]+)").unwrap(),
        }
    }

    /// Rule A and Rule B over one model output.
    pub fn check(&self, index: &Index, text: &str, passages: &[Sent]) -> (Verdict, Vec<Violation>) {
        let sent_tokens: HashSet<&str> = passages.iter().map(|p| p.token.as_str()).collect();
        let mut sent_verses: HashSet<i64> = HashSet::new();
        for p in passages {
            sent_verses.extend(p.verse_ids.iter().copied());
        }

        let mut violations: Vec<Violation> = Vec::new();

        // Rule A: every token must be one that was sent.
        for m in self.token_re.captures_iter(text) {
            let whole = m.get(0).unwrap();
            let tok = format!("[P{}]", &m[1]);
            if !sent_tokens.contains(tok.as_str()) {
                violations.push(Violation {
                    kind: Kind::Token,
                    text: tok,
                    reason: "token not in sent set".to_string(),
                    span: (whole.start(), whole.end()),
                    char_span: char_span(text, (whole.start(), whole.end())),
                });
            }
        }

        // Rule B: every free-text reference must resolve inside the sent set.
        for caps in self.ref_re.captures_iter(text) {
            let whole = caps.get(0).unwrap();
            let written = whole.as_str();
            let book_written = caps.name("book").unwrap().as_str();
            let book_id = match self.books.lookup(book_written) {
                Some(b) => b,
                None => continue,
            };
            // An ambiguous book name must be capitalised to count.
            if is_ambiguous(&norm(book_written))
                && !book_written.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
            {
                continue;
            }
            // A number followed by a unit noun is a count, not a chapter.
            // Python looks at the next 24 characters; a byte slice would cut
            // a multi-byte character in half.
            let after: String = text[whole.end()..].chars().take(24).collect();
            if caps.name("verse").is_none() {
                if let Some(nxt) = self.after_re.captures(&after) {
                    if UNIT_NOUNS.contains(&nxt[1].to_lowercase().as_str()) {
                        continue;
                    }
                }
            }

            let chapter: i64 = caps["chapter"].parse().unwrap_or(0);
            let (wanted, reason): (HashSet<i64>, &str) = match caps.name("verse") {
                None => {
                    let verses = index.chapter_verses(book_id, chapter);
                    let set: HashSet<i64> = verses
                        .iter()
                        .map(|v| book_id * 1_000_000 + chapter * 1000 + v)
                        .collect();
                    let reason = if set.is_empty() {
                        "chapter not in this text"
                    } else {
                        "whole chapter not in sent set"
                    };
                    (set, reason)
                }
                Some(v1) => {
                    let v1: i64 = v1.as_str().parse().unwrap_or(0);
                    let v2: i64 =
                        caps.name("verse_end").map(|m| m.as_str().parse().unwrap_or(v1)).unwrap_or(v1);
                    let set: HashSet<i64> =
                        (v1..=v2).map(|v| book_id * 1_000_000 + chapter * 1000 + v).collect();
                    (set, "verses not in sent set")
                }
            };
            if wanted.is_empty() || !wanted.is_subset(&sent_verses) {
                violations.push(Violation {
                    kind: Kind::Reference,
                    text: written.to_string(),
                    reason: reason.to_string(),
                    span: (whole.start(), whole.end()),
                    char_span: char_span(text, (whole.start(), whole.end())),
                });
            }
        }

        let verdict = if violations.is_empty() { Verdict::Ok } else { Verdict::Violation };
        (verdict, violations)
    }

    /// Remove offending spans, right to left so offsets stay valid.
    ///
    /// A stripped output is never shown to a reader. It exists so the retry
    /// prompt can quote what was wrong.
    pub fn strip(text: &str, violations: &[Violation]) -> String {
        let mut spans: Vec<(usize, usize)> = violations.iter().map(|v| v.span).collect();
        spans.sort_by(|a, b| b.0.cmp(&a.0));
        let mut out = text.to_string();
        for (a, b) in spans {
            if b <= out.len() {
                out.replace_range(a..b, "");
            }
        }
        // Collapse the runs of spaces that removal leaves behind.
        let squeeze = Regex::new(r"[ \t]{2,}").unwrap();
        squeeze.replace_all(&out, " ").into_owned()
    }

    /// The text handed back to the model on the retry.
    pub fn failure_note(violations: &[Violation]) -> String {
        violations
            .iter()
            .map(|v| format!("{} \"{}\" ({})", v.kind.as_str(), v.text, v.reason))
            .collect::<Vec<_>>()
            .join("; ")
    }

    /// PLAN 5.6's fallback: the retrieved passages grouped by book, with the
    /// one-line note. A normal outcome, not an error state.
    pub fn fallback(index: &Index, passages: &[Sent]) -> String {
        let mut by_book: HashMap<i64, Vec<&Sent>> = HashMap::new();
        for p in passages {
            if let Some(&first) = p.verse_ids.first() {
                by_book.entry(first / 1_000_000).or_default().push(p);
            }
        }
        let mut order: Vec<i64> = by_book.keys().copied().collect();
        order.sort();
        let mut lines = vec![
            "A synthesis could not be produced for this question. These are the \
             passages that were found."
                .to_string(),
        ];
        for bid in order {
            lines.push(String::new());
            lines.push(index.abbrev(bid).to_string());
            let mut ps = by_book.remove(&bid).unwrap();
            ps.sort_by_key(|p| p.verse_ids.first().copied().unwrap_or(0));
            for p in ps {
                let label = if p.reference.is_empty() { &p.token } else { &p.reference };
                lines.push(format!("  {}", label));
            }
        }
        lines.join("\n")
    }
}

/// The tokens an answer cites, sorted and deduplicated.
pub fn cited_tokens(text: &str) -> Vec<String> {
    let re = Regex::new(r"\[P\d+\]").unwrap();
    let mut out: Vec<String> = re.find_iter(text).map(|m| m.as_str().to_string()).collect();
    out.sort();
    out.dedup();
    out
}
