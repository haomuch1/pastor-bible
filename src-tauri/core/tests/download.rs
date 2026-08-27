//! The downloader: the one thing here that reaches the internet.
//!
//! Three things have to be true. It must not fetch what is already correct,
//! because that is what makes a reinstall not cost five gigabytes. It must
//! resume, because a five-gigabyte download on a domestic connection will be
//! interrupted. And it must refuse a file whose checksum is wrong, because the
//! alternative is running a model file we cannot identify.
//!
//! The resume and corruption cases run against a server inside this test
//! process. The real host is checked separately, by a test that only reads
//! headers, so the suite does not depend on the internet to pass.

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use pastor_bible_core::download::{ensure_model, model, sha256_file, status, ModelSpec, Progress, MODELS};

/// A file small enough to serve from memory and large enough to resume in the
/// middle of.
const BODY_LEN: usize = 300_000;

fn body() -> Vec<u8> {
    (0..BODY_LEN).map(|i| (i % 251) as u8).collect()
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join("pastor-bible-download-tests").join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn sha256_of(bytes: &[u8]) -> String {
    pastor_bible_core::download::sha256_bytes(bytes)
}

/// A one-file HTTP server that honours Range, and can be told to hang up part
/// way through so the resume path has something to resume from.
struct Server {
    pub port: u16,
    stop: Arc<AtomicBool>,
    /// Bytes to send before hanging up. 0 means send everything.
    cut_after: Arc<AtomicUsize>,
    pub requests: Arc<AtomicUsize>,
}

impl Server {
    fn start(payload: Vec<u8>) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let stop = Arc::new(AtomicBool::new(false));
        let cut_after = Arc::new(AtomicUsize::new(0));
        let requests = Arc::new(AtomicUsize::new(0));
        let (s2, c2, r2) = (stop.clone(), cut_after.clone(), requests.clone());
        std::thread::spawn(move || {
            for conn in listener.incoming() {
                if s2.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(conn) = conn else { continue };
                r2.fetch_add(1, Ordering::SeqCst);
                let _ = serve(conn, &payload, c2.load(Ordering::SeqCst));
            }
        });
        Server { port, stop, cut_after, requests }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}/model.gguf", self.port)
    }

    fn cut_after(&self, n: usize) {
        self.cut_after.store(n, Ordering::SeqCst);
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
    }
}

fn serve(mut conn: TcpStream, payload: &[u8], cut_after: usize) -> std::io::Result<()> {
    let mut reader = BufReader::new(conn.try_clone()?);
    let mut start = 0usize;
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 || line.trim().is_empty() {
            break;
        }
        if let Some(rest) = line.to_ascii_lowercase().strip_prefix("range: bytes=") {
            if let Some(from) = rest.split('-').next() {
                start = from.trim().parse().unwrap_or(0);
            }
        }
    }
    let slice = &payload[start.min(payload.len())..];
    let head = if start > 0 {
        format!(
            "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {}-{}/{}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
            slice.len(), start, payload.len() - 1, payload.len()
        )
    } else {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
            slice.len()
        )
    };
    conn.write_all(head.as_bytes())?;
    let send = if cut_after > 0 { cut_after.min(slice.len()) } else { slice.len() };
    conn.write_all(&slice[..send])?;
    conn.flush()?;
    Ok(())
}

fn spec_for(url: &'static str, sha: &'static str, bytes: u64) -> ModelSpec {
    ModelSpec {
        id: "test",
        file: "model.gguf",
        url,
        sha256: sha,
        bytes,
        label: "Test model",
        note: "",
        bundled: false,
        vram_mib: 0,
    }
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn collect(dir: &std::path::Path, spec: &ModelSpec) -> (Result<std::path::PathBuf, String>, Vec<Progress>) {
    let mut seen = Vec::new();
    let out = ensure_model(spec, dir, Arc::new(AtomicBool::new(false)), |p| seen.push(p));
    (out, seen)
}

#[test]
fn a_clean_download_verifies_and_is_renamed_into_place() {
    let payload = body();
    let sha = sha256_of(&payload);
    let server = Server::start(payload.clone());
    let spec = spec_for(leak(server.url()), leak(sha), payload.len() as u64);
    let dir = temp_dir("clean");

    let (out, seen) = collect(&dir, &spec);
    let path = out.expect("download");
    assert_eq!(std::fs::read(&path).unwrap(), payload);
    assert!(!dir.join("model.gguf.part").exists(), "the partial file must be gone");
    assert!(
        matches!(seen.last(), Some(Progress::Done { skipped: false, .. })),
        "last event was {:?}",
        seen.last()
    );
    assert!(
        seen.iter().any(|p| matches!(p, Progress::Downloading { .. })),
        "progress must be reported while downloading"
    );
    assert!(seen.iter().any(|p| matches!(p, Progress::Verifying { .. })));
}

#[test]
fn a_file_that_is_already_correct_is_not_fetched_again() {
    let payload = body();
    let sha = sha256_of(&payload);
    let server = Server::start(payload.clone());
    let spec = spec_for(leak(server.url()), leak(sha), payload.len() as u64);
    let dir = temp_dir("skip");
    std::fs::write(dir.join("model.gguf"), &payload).unwrap();

    let (out, seen) = collect(&dir, &spec);
    out.expect("the existing file is accepted");
    assert_eq!(server.requests.load(Ordering::SeqCst), 0, "the host was contacted anyway");
    assert!(matches!(seen.last(), Some(Progress::Done { skipped: true, .. })));
}

#[test]
fn a_truncated_partial_is_resumed_rather_than_restarted() {
    let payload = body();
    let sha = sha256_of(&payload);
    let server = Server::start(payload.clone());
    let spec = spec_for(leak(server.url()), leak(sha), payload.len() as u64);
    let dir = temp_dir("resume");

    // First attempt: the server hangs up after 100 kB.
    server.cut_after(100_000);
    let (first, _) = collect(&dir, &spec);
    assert!(first.is_err(), "a truncated download must not be accepted");
    let part = dir.join("model.gguf.part");
    let have = std::fs::metadata(&part).unwrap().len();
    assert!(have > 0 && have < payload.len() as u64, "kept {} bytes", have);
    assert_eq!(status(&spec, &dir).partial_bytes, have);
    assert!(!dir.join("model.gguf").exists(), "nothing is in place yet");

    // Second attempt: the server behaves, and the download carries on.
    server.cut_after(0);
    let (second, seen) = collect(&dir, &spec);
    let path = second.expect("resume");
    assert_eq!(std::fs::read(&path).unwrap(), payload, "the resumed file is byte-identical");

    let resumed_from = seen
        .iter()
        .find_map(|p| match p {
            Progress::Downloading { resumed_from, .. } => Some(*resumed_from),
            _ => None,
        })
        .unwrap();
    assert_eq!(resumed_from, have, "it started again from the beginning");
}

#[test]
fn a_corrupt_download_is_rejected_and_deleted() {
    let payload = body();
    let server = Server::start(payload.clone());
    // The pinned checksum is of something else, so what arrives is wrong.
    let wrong = "0".repeat(64);
    let spec = spec_for(leak(server.url()), leak(wrong), payload.len() as u64);
    let dir = temp_dir("corrupt");

    let (out, seen) = collect(&dir, &spec);
    let err = out.unwrap_err();
    assert!(err.contains("checksum"), "{}", err);
    assert!(!dir.join("model.gguf").exists(), "a bad file must never be put in place");
    assert!(!dir.join("model.gguf.part").exists(), "a bad file must be deleted, not kept");
    assert!(matches!(seen.last(), Some(Progress::Failed { .. })));
}

#[test]
fn a_present_but_wrong_file_is_replaced_rather_than_trusted() {
    let payload = body();
    let sha = sha256_of(&payload);
    let server = Server::start(payload.clone());
    let spec = spec_for(leak(server.url()), leak(sha), payload.len() as u64);
    let dir = temp_dir("wrongfile");
    // Right size, wrong contents: the size check alone would pass this.
    std::fs::write(dir.join("model.gguf"), vec![7u8; payload.len()]).unwrap();

    let (out, _) = collect(&dir, &spec);
    let path = out.expect("it is fetched again");
    assert_eq!(std::fs::read(&path).unwrap(), payload);
    assert!(server.requests.load(Ordering::SeqCst) > 0, "it must have been fetched");
}

#[test]
fn a_host_offering_a_different_size_is_refused() {
    let payload = body();
    let sha = sha256_of(&payload);
    let server = Server::start(payload.clone());
    // This build expects a different file from the one the host has.
    let spec = spec_for(leak(server.url()), leak(sha), payload.len() as u64 + 1);
    let dir = temp_dir("wrongsize");

    let (out, _) = collect(&dir, &spec);
    let err = out.unwrap_err();
    assert!(err.contains("Refusing"), "{}", err);
    assert!(!dir.join("model.gguf").exists());
}

#[test]
fn cancelling_keeps_what_was_already_fetched() {
    let payload = body();
    let sha = sha256_of(&payload);
    let server = Server::start(payload.clone());
    let spec = spec_for(leak(server.url()), leak(sha), payload.len() as u64);
    let dir = temp_dir("cancel");

    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    let out = ensure_model(&spec, &dir, cancel, move |p| {
        if let Progress::Downloading { done, .. } = p {
            if done > 0 {
                flag.store(true, Ordering::SeqCst);
            }
        }
    });
    assert_eq!(out.unwrap_err(), "cancelled");
    // Cancelling a download must not throw away what the reader already waited
    // for; it is resumed next time.
    assert!(dir.join("model.gguf.part").exists());
    assert!(!dir.join("model.gguf").exists());
}

#[test]
fn only_the_pinned_models_and_loopback_may_be_fetched() {
    let dir = temp_dir("allowlist");
    for bad in [
        "https://example.com/evil.gguf",
        "https://huggingface.co/Qwen/Qwen3-8B-GGUF/resolve/main/other.gguf",
        "http://10.0.0.1:8080/x",
    ] {
        let spec = spec_for(leak(bad.to_string()), leak("0".repeat(64)), 10);
        let (out, _) = collect(&dir, &spec);
        let err = out.unwrap_err();
        assert!(
            err.contains("not a pinned model URL") || err.contains("cannot reach"),
            "{} gave {:?}",
            bad,
            err
        );
        assert!(
            err.contains("not a pinned model URL"),
            "{} was not refused by the allow-list; it gave {:?}",
            bad,
            err
        );
    }
}

#[test]
fn the_pinned_list_is_the_three_models_this_build_uses() {
    assert_eq!(MODELS.len(), 3);
    let standard = model("standard").expect("a standard model");
    assert_eq!(standard.file, "Qwen3-8B-Q4_K_M.gguf");
    assert_eq!(standard.bytes, 5_027_783_488);
    assert!(!standard.bundled);
    let smaller = model("smaller").expect("a smaller model");
    assert!(smaller.note.contains("list-style"), "the caveat must travel with the choice");
    let embed = model("embedding").expect("the search model");
    assert!(embed.bundled, "the search model ships with the installer");
    for m in MODELS {
        assert_eq!(m.sha256.len(), 64, "{} has no checksum", m.file);
        assert!(m.url.starts_with("https://"), "{} is not fetched over TLS", m.file);
        assert!(m.bytes > 0);
    }
    assert!(model("nonsense").is_none());
}

/// The models on this machine are the ones the pinned checksums name. This is
/// the check that would catch a wrong file having been used for every
/// measurement since P3.
///
/// Ignored by default: it reads seven gigabytes, which takes over two minutes
/// in a debug build and is not something to pay for on every `cargo test`. Run
/// it with `--ignored` when the model files change.
#[test]
#[ignore]
fn the_model_files_on_this_machine_match_their_pinned_checksums() {
    let dir = std::path::PathBuf::from(pastor_bible_core::paths::model_dir());
    let mut checked = 0;
    for m in MODELS {
        let p = dir.join(m.file);
        if !p.exists() {
            continue;
        }
        let got = sha256_file(&p, |_, _| {}).unwrap();
        assert_eq!(got, m.sha256, "{} on this machine is not the pinned file", m.file);
        assert_eq!(std::fs::metadata(&p).unwrap().len(), m.bytes, "{} is the wrong size", m.file);
        checked += 1;
    }
    assert!(checked > 0, "no model files found in {:?}; nothing was checked", dir);
}

/// Reads headers from the real host. Ignored by default so the suite passes
/// offline, which is the whole point of this program.
#[test]
#[ignore]
fn the_real_host_needs_no_token_and_honours_range() {
    for m in MODELS {
        let resp = ureq::get(m.url)
            .set("Range", "bytes=0-99")
            .set("User-Agent", "pastor-bible/0.0.1")
            .call()
            .unwrap_or_else(|e| panic!("{}: {}", m.file, e));
        assert_eq!(resp.status(), 206, "{} does not honour Range", m.file);
        let cr = resp.header("Content-Range").expect("Content-Range").to_string();
        let total: u64 = cr.rsplit('/').next().unwrap().trim().parse().unwrap();
        assert_eq!(total, m.bytes, "{} is {} bytes on the host", m.file, total);
        let mut buf = Vec::new();
        resp.into_reader().read_to_end(&mut buf).unwrap();
        assert_eq!(buf.len(), 100);
    }
}
