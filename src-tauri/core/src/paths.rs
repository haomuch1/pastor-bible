//! Where the pieces live.
//!
//! In the shipped app these are all inside the install directory or the app
//! data directory, and P5 and P6 set them. Until then everything resolves
//! relative to the repository, and every path can be overridden by an
//! environment variable so a test or a measurement can point somewhere else
//! without editing code.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    // core/ -> src-tauri/ -> repository root
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn from_env_or(var: &str, fallback: PathBuf) -> String {
    std::env::var(var).unwrap_or_else(|_| fallback.to_string_lossy().into_owned())
}

pub fn index_db() -> String {
    from_env_or("TPB_INDEX_DB", repo_root().join("src-tauri").join("resources").join("index.db"))
}

pub fn llama_server() -> String {
    let name = if cfg!(windows) { "llama-server.exe" } else { "llama-server" };
    from_env_or("TPB_LLAMA_SERVER", repo_root().join("tools").join("llama").join(name))
}

pub fn model_dir() -> String {
    from_env_or("TPB_MODEL_DIR", repo_root().join("models"))
}

pub fn model(file: &str) -> String {
    PathBuf::from(model_dir()).join(file).to_string_lossy().into_owned()
}

pub fn data_dir() -> String {
    from_env_or("TPB_DATA_DIR", repo_root().join("data"))
}

pub fn prompts_dir() -> String {
    from_env_or("TPB_PROMPTS_DIR", PathBuf::from(data_dir()).join("prompts"))
}

pub fn crisis_terms() -> String {
    from_env_or("TPB_CRISIS_TERMS", PathBuf::from(data_dir()).join("crisis_terms.txt"))
}

pub fn crisis_note() -> String {
    from_env_or("TPB_CRISIS_NOTE", PathBuf::from(data_dir()).join("crisis_note.txt"))
}

pub fn log_dir() -> String {
    from_env_or("TPB_LOG_DIR", repo_root().join("tools"))
}
