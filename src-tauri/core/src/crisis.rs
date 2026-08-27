//! PLAN 5.8: crisis language in the question.
//!
//! The note is shown above the answer, never instead of it, and the answer
//! still runs. Over-triggering is acceptable; under-triggering is not, so the
//! phrases lean wide and the matching is deliberately blunt.
//!
//! The note text lives in data/crisis_note.txt and is PLAN 9.3 verbatim. README
//! quotes the same file, and a test asserts the two are identical, so the
//! wording a reader in trouble sees cannot drift from the wording the project
//! promised.

/// A phrase list, loaded once.
pub struct CrisisMatcher {
    pub terms: Vec<String>,
    pub note: String,
}

/// Lowercase, and collapse every run of whitespace to one space.
pub fn normalize(text: &str) -> String {
    let lower = text.to_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut in_space = false;
    for c in lower.chars() {
        if c.is_whitespace() {
            in_space = true;
        } else {
            if in_space && !out.is_empty() {
                out.push(' ');
            }
            in_space = false;
            out.push(c);
        }
    }
    out
}

pub fn parse_terms(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| normalize(l))
        .filter(|l| !l.is_empty())
        .collect()
}

impl CrisisMatcher {
    /// The list and the note compiled into this binary. What the shipped app
    /// uses. Built through the same checks `load` applies, so an empty list
    /// fails here exactly as it would from a file.
    pub fn builtin() -> Result<Self, String> {
        Self::from_text(
            crate::builtin::CRISIS_TERMS,
            crate::builtin::CRISIS_NOTE,
            "the built-in crisis list",
            "the built-in crisis note",
        )
    }

    /// `Some` paths read those files; `None` uses the built-in copies.
    pub fn resolve(terms: Option<&str>, note: Option<&str>) -> Result<Self, String> {
        match (terms, note) {
            (Some(t), Some(n)) => Self::load(t, n),
            _ => Self::builtin(),
        }
    }

    pub fn load(terms_path: &str, note_path: &str) -> Result<Self, String> {
        let raw = std::fs::read_to_string(terms_path)
            .map_err(|e| format!("cannot read {}: {}", terms_path, e))?;
        let note = std::fs::read_to_string(note_path)
            .map_err(|e| format!("cannot read {}: {}", note_path, e))?;
        Self::from_text(&raw, &note, terms_path, note_path)
    }

    fn from_text(
        raw: &str,
        note: &str,
        terms_name: &str,
        note_name: &str,
    ) -> Result<Self, String> {
        let terms = parse_terms(raw);
        if terms.is_empty() {
            // An empty list matches nothing, and PLAN 5.8 holds that
            // under-triggering is unacceptable. Refusing to start is the only
            // honest response to a crisis list with no terms in it.
            return Err(format!(
                "{} holds no terms. A crisis list that matches nothing is worse \
                 than no crisis feature, because it looks like one.",
                terms_name
            ));
        }
        let note = note.trim().to_string();
        if note.is_empty() {
            return Err(format!("{} is empty", note_name));
        }
        Ok(CrisisMatcher { terms, note })
    }

    /// Case-insensitive substring match after whitespace normalisation.
    pub fn matches(&self, question: &str) -> Option<&str> {
        let q = normalize(question);
        self.terms.iter().find(|t| q.contains(t.as_str())).map(|s| s.as_str())
    }

    pub fn is_crisis(&self, question: &str) -> bool {
        self.matches(question).is_some()
    }
}
