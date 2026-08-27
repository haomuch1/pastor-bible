//! Shared test scaffolding.
//!
//! index.db is a build artefact and is not in the repository, so a checkout
//! without it cannot run the parity tests. They say so and fail rather than
//! passing quietly: a parity test that silently skips is worse than no parity
//! test, because it reads as evidence.

use std::path::PathBuf;

pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures")
}

pub fn index_path() -> String {
    if let Ok(p) = std::env::var("TPB_INDEX_DB") {
        return p;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("resources")
        .join("index.db")
        .to_string_lossy()
        .into_owned()
}

/// True when the index is present. Tests that need it call `require_index`.
pub fn require_index() -> String {
    let p = index_path();
    assert!(
        std::path::Path::new(&p).exists(),
        "index.db not found at {}. It is built by the Python pipeline and is not \
         committed. Build it, or set TPB_INDEX_DB, before running the parity tests.",
        p
    );
    p
}

pub fn read_json(path: &std::path::Path) -> serde_json::Value {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {}", path.display(), e));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("cannot parse fixture {}: {}", path.display(), e))
}
