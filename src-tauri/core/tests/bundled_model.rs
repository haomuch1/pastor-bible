//! The bundled search model is where the app looks, and is the file we pinned.
//!
//! P5.1 opened with a reader being told `The system cannot find the path
//! specified (os error 3)` because the app resolved the embedding model inside
//! the application data directory, which nothing ever writes it to. It is a
//! resource of the application, and this test is the standing check that the
//! path the app resolves has the pinned bytes behind it. A size check would
//! pass a substituted file of the same length, so the sha256 is read in full;
//! it is a quarter of a gigabyte and takes about a second.

use std::path::Path;

use pastor_bible_core::download;
use pastor_bible_core::paths;
use pastor_bible_core::pipeline::EMBED_GGUF;

#[test]
fn the_bundled_search_model_is_present_and_matches_its_pinned_sha256() {
    let spec = download::model("embedding").expect("the embedding model is pinned in download.rs");
    assert_eq!(spec.file, EMBED_GGUF, "the pinned file name and the constant must agree");
    assert!(spec.bundled, "the embedding model ships with the app; it is never downloaded");

    let path = paths::embed_model();
    assert!(
        Path::new(&path).exists(),
        "the search model was not found at {}.\n\
         It is a bundled resource: run `python tools/fetch_model.py` to place it \
         in src-tauri/resources/, which is what the installer ships and what \
         `tauri dev` copies beside the binary.",
        path
    );

    let size = std::fs::metadata(&path).expect("stat the search model").len();
    assert_eq!(size, spec.bytes, "{} is {} bytes; {} were pinned", path, size, spec.bytes);

    let got = download::sha256_file(Path::new(&path), |_, _| {}).expect("read the search model");
    assert_eq!(
        got, spec.sha256,
        "{} is not the file that was pinned. A model file we cannot identify is \
         the one thing that must never be loaded.",
        path
    );
}

/// The resource directory, not the application data directory.
///
/// This is the defect itself, written down: if the resolved path ever falls
/// back into `models/` under application data, the reader is one fresh install
/// away from the error P5.1 fixed.
#[test]
fn the_search_model_resolves_to_the_resource_directory() {
    let resolved = paths::embed_model();
    let bundled = paths::resource_file(EMBED_GGUF);
    if std::env::var("TPB_EMBED_MODEL").is_ok() {
        return; // an explicit override is the one thing allowed to win
    }
    assert_eq!(
        resolved, bundled,
        "the search model must resolve to the bundled resource, not to {}",
        resolved
    );
}
