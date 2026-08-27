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

/// A file in the repository's resources directory.
///
/// In an installed app these are found through Tauri's resource path, which the
/// shell resolves first; this is the development answer, and the last resort
/// everywhere, so that a name can be shown to the reader even when the file is
/// not there.
pub fn resource_file(name: &str) -> String {
    repo_root().join("src-tauri").join("resources").join(name).to_string_lossy().into_owned()
}

pub fn index_db() -> String {
    from_env_or("TPB_INDEX_DB", repo_root().join("src-tauri").join("resources").join("index.db"))
}

/// The bundled embedding model, for the CLI harness and the tests.
///
/// The application shell resolves this through Tauri's resource path and only
/// falls back to here. `TPB_EMBED_MODEL` overrides both.
pub fn embed_model() -> String {
    match std::env::var("TPB_EMBED_MODEL") {
        Ok(p) => p,
        Err(_) => {
            let bundled = resource_file(crate::pipeline::EMBED_GGUF);
            if std::path::Path::new(&bundled).exists() {
                bundled
            } else {
                model(crate::pipeline::EMBED_GGUF)
            }
        }
    }
}

pub fn llama_server() -> String {
    let name = if cfg!(windows) { "llama-server.exe" } else { "llama-server" };
    // The assembled bundle first: it is what the installer ships and the only
    // copy with the Vulkan backend beside it. tools/llama is what
    // fetch_llama.py unpacks and is the fallback for a checkout that has not
    // run `--bundle` yet.
    let bundled = repo_root().join("src-tauri").join("resources").join("llama").join(name);
    if std::env::var("TPB_LLAMA_SERVER").is_err() && bundled.exists() {
        return bundled.to_string_lossy().into_owned();
    }
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

/// A text path for an installed build: only if something explicitly asked for
/// one, never the repository.
///
/// The functions above answer with a repository path when no variable is set,
/// which is right for the harness and the tests and was catastrophic for the
/// shipped app: `repo_root()` is `CARGO_MANIFEST_DIR`, an absolute path on
/// whoever ran the compiler. P7's laptop opened to "cannot read
/// disclaimer.txt: The system cannot find the path specified (os error 3)",
/// looking in a directory on a drive it does not have.
///
/// So the app asks this instead. `Some` only when the variable is set, or in a
/// debug build where the repository really is where the binary was built and
/// editing a prompt without recompiling is worth having. Otherwise `None`, and
/// the caller uses the copy compiled into the binary.
pub fn override_path(var: &str, fallback: impl FnOnce() -> String) -> Option<String> {
    if let Ok(p) = std::env::var(var) {
        return Some(p);
    }
    if cfg!(debug_assertions) {
        let p = fallback();
        if std::path::Path::new(&p).exists() {
            return Some(p);
        }
    }
    None
}

/// The prompts directory an installed build should read, or `None` for built in.
pub fn prompts_dir_override() -> Option<String> {
    override_path("TPB_PROMPTS_DIR", prompts_dir)
}

/// The crisis list and note an installed build should read, or `None` for both.
pub fn crisis_overrides() -> (Option<String>, Option<String>) {
    let terms = override_path("TPB_CRISIS_TERMS", crisis_terms);
    let note = override_path("TPB_CRISIS_NOTE", crisis_note);
    match (terms, note) {
        // Both or neither: a matcher built from one file and one built-in would
        // be a state nobody asked for and nobody could reason about.
        (Some(t), Some(n)) => (Some(t), Some(n)),
        _ => (None, None),
    }
}

pub fn log_dir() -> String {
    from_env_or("TPB_LOG_DIR", repo_root().join("tools"))
}
