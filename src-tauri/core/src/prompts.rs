//! Prompts, loaded verbatim from data/prompts/.
//!
//! A prompt is part of the product's behaviour, so it is versioned and diffable
//! rather than buried in code. The version line is read at start-up and carried
//! into the output structure, so an answer in history says which prompt wrote
//! it.

use std::collections::HashMap;

/// The prompts this build expects, with the version each was written against.
/// A version that has moved is a warning, not a failure: the file is the
/// authority, and the check exists to make a change visible rather than to
/// forbid one.
pub const EXPECTED: &[(&str, &str)] =
    &[("synopsis", "1"), ("retry", "1"), ("rewrite", "1")];

pub struct Prompts {
    dir: String,
    bodies: HashMap<String, String>,
    versions: HashMap<String, String>,
}

impl Prompts {
    pub fn load(dir: &str) -> Result<Self, String> {
        let mut bodies = HashMap::new();
        let mut versions = HashMap::new();
        for (name, _) in EXPECTED {
            let path = format!("{}/{}.txt", dir, name);
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("cannot read prompt {}: {}", path, e))?;
            versions.insert(name.to_string(), version_line(&text));
            bodies.insert(name.to_string(), body(&text));
        }
        Ok(Prompts { dir: dir.to_string(), bodies, versions })
    }

    pub fn dir(&self) -> &str {
        &self.dir
    }

    pub fn body(&self, name: &str) -> &str {
        self.bodies.get(name).map(|s| s.as_str()).unwrap_or("")
    }

    pub fn version(&self, name: &str) -> &str {
        self.versions.get(name).map(|s| s.as_str()).unwrap_or("?")
    }

    pub fn versions(&self) -> Vec<(String, String)> {
        let mut v: Vec<(String, String)> =
            self.versions.iter().map(|(k, x)| (k.clone(), x.clone())).collect();
        v.sort();
        v
    }

    /// Prompt versions that differ from what this build was written against.
    pub fn drift(&self) -> Vec<String> {
        EXPECTED
            .iter()
            .filter(|(n, v)| self.version(n) != *v)
            .map(|(n, v)| format!("{} is version {}, this build expects {}", n, self.version(n), v))
            .collect()
    }
}

fn version_line(text: &str) -> String {
    text.lines()
        .next()
        .and_then(|l| l.split_once(':'))
        .map(|(_, v)| v.trim().to_string())
        .unwrap_or_else(|| "?".to_string())
}

/// Drop the `version:` and `purpose:` header; keep the instruction body.
fn body(text: &str) -> String {
    let mut lines = text.lines().peekable();
    let mut out: Vec<&str> = Vec::new();
    let mut in_header = true;
    while let Some(line) = lines.next() {
        if in_header {
            if line.starts_with("version:") || line.starts_with("purpose:") {
                continue;
            }
            // Continuation lines of the purpose block are indented nine spaces.
            if line.starts_with("         ") {
                continue;
            }
            if line.trim().is_empty() {
                continue;
            }
            in_header = false;
        }
        out.push(line);
    }
    out.join("\n").trim().to_string()
}

/// Fill `{name}` placeholders. Unknown placeholders are left alone so a typo in
/// a prompt shows up in the output rather than silently emptying a section.
pub fn fill(template: &str, values: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (k, v) in values {
        out = out.replace(&format!("{{{}}}", k), v);
    }
    out
}
