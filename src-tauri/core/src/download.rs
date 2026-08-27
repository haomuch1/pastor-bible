//! The one place in this program that reaches the internet.
//!
//! PLAN section 1: the internet is used exactly twice, to download the
//! installer and to download the model file on first run. This module is the
//! second of those, and it is the only code here that opens a connection to
//! anything but 127.0.0.1.
//!
//! That is enforced rather than asserted. `MODELS` holds the exact URL of every
//! file this program may fetch, and `fetch` refuses anything else by
//! whole-string comparison. No path is built from user input, no host comes
//! from a config file, and there is no code path that downloads an arbitrary
//! URL. A caller that wanted to send a question somewhere would have to add a
//! URL to that list, which is a diff a reviewer would see. Loopback is also
//! allowed, because a request to 127.0.0.1 cannot carry anything off the
//! machine and it is what makes the resume and corruption paths testable.
//!
//! Tauri's capabilities gate what the *frontend* may do and cannot gate a Rust
//! HTTP client, so the guarantee here is the list above and the offline test in
//! CI, not a capability. PLAN 3.3 named the wrong mechanism; DECISIONS.md
//! records the correction.
//!
//! Every file is checked against a sha256 pinned in the source before it is
//! used. A download that does not match is deleted, not kept and not run.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A model file the app may fetch. Everything about it is pinned here.
#[derive(Clone, Copy, Debug)]
pub struct ModelSpec {
    /// The identifier settings uses.
    pub id: &'static str,
    /// The name the file has on disk once it is verified.
    pub file: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    /// Free graphics memory this model needs to run wholly on the card, in
    /// MiB, at the app's 8192-token context.
    ///
    /// Measured on 2026-08-27 on an RTX 3080 as the rise in whole-GPU used
    /// memory from just before llama-server started to its peak while it was
    /// loaded and answering a 1,097-token prompt, plus a tenth. Per-process
    /// graphics memory is not reportable under Windows' display driver model,
    /// so a delta against the desktop's own usage is the honest figure. Zero
    /// for a model that never runs on the card.
    pub vram_mib: u64,
    pub bytes: u64,
    /// Shown to the reader when they choose between models.
    pub label: &'static str,
    pub note: &'static str,
    /// Bundled by the installer rather than downloaded.
    pub bundled: bool,
}

/// The complete list. Verified against the host on 2026-08-26: no token is
/// needed, HTTP Range is honoured, and each size and checksum below was
/// confirmed against both the host and the file on the build machine.
pub const MODELS: &[ModelSpec] = &[
    ModelSpec {
        id: "standard",
        file: "Qwen3-8B-Q4_K_M.gguf",
        url: "https://huggingface.co/Qwen/Qwen3-8B-GGUF/resolve/main/Qwen3-8B-Q4_K_M.gguf",
        sha256: "d98cdcbd03e17ce47681435b5150e34c1417f50b5c0019dd560e4882c5745785",
        // measured 5,750 MiB
        vram_mib: 6325,
        bytes: 5_027_783_488,
        label: "Standard model",
        note: "The model The Pastor Bible is built and tested on.",
        bundled: false,
    },
    ModelSpec {
        id: "smaller",
        file: "Qwen3-1.7B-Q8_0.gguf",
        url: "https://huggingface.co/Qwen/Qwen3-1.7B-GGUF/resolve/main/Qwen3-1.7B-Q8_0.gguf",
        sha256: "061b54daade076b5d3362dac252678d17da8c68f07560be70818cace6590cb1a",
        // measured 2,722 MiB
        vram_mib: 2994,
        bytes: 1_834_426_016,
        label: "Smaller model",
        note: "Faster, needs less memory, gives list-style answers.",
        bundled: false,
    },
    ModelSpec {
        id: "embedding",
        file: "nomic-embed-text-v1.5-f16.gguf",
        url: "https://huggingface.co/nomic-ai/nomic-embed-text-v1.5-GGUF/resolve/main/nomic-embed-text-v1.5.f16.gguf",
        sha256: "f7af6f66802f4df86eda10fe9bbcfc75c39562bed48ef6ace719a251cf1c2fdb",
        // The search model always runs on the processor: it takes about a
        // second there, and every megabyte of the card is worth more to the
        // model that takes minutes.
        vram_mib: 0,
        bytes: 274_290_560,
        label: "Search model",
        note: "Bundled with the installer; never downloaded.",
        bundled: true,
    },
];

pub fn model(id: &str) -> Option<&'static ModelSpec> {
    MODELS.iter().find(|m| m.id == id)
}

/// The only URLs this program will ever request.
///
/// The pinned model files, and loopback. Loopback is here so that the resume
/// and corruption paths can be tested against a server in the test process;
/// a request to 127.0.0.1 cannot carry anything off this machine, so allowing
/// it costs nothing that the list exists to protect.
fn allowed(url: &str) -> bool {
    MODELS.iter().any(|m| m.url == url)
        || url.starts_with("http://127.0.0.1:")
        || url.starts_with("http://localhost:")
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case")]
pub enum Progress {
    /// Checking a file that is already on disk.
    Checking { file: String },
    Downloading {
        file: String,
        done: u64,
        total: u64,
        percent: f64,
        bytes_per_second: f64,
        eta_seconds: Option<f64>,
        resumed_from: u64,
    },
    /// Reading the finished file back to check its checksum.
    Verifying { file: String, done: u64, total: u64, percent: f64 },
    Done { file: String, bytes: u64, skipped: bool },
    Failed { file: String, message: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelStatus {
    pub id: String,
    pub file: String,
    pub label: String,
    pub note: String,
    pub bytes: u64,
    pub bundled: bool,
    pub present: bool,
    /// Bytes already fetched into the partial file, if there is one.
    pub partial_bytes: u64,
}

pub fn status(spec: &ModelSpec, dir: &Path) -> ModelStatus {
    let full = dir.join(spec.file);
    let part = part_path(dir, spec);
    ModelStatus {
        id: spec.id.to_string(),
        file: spec.file.to_string(),
        label: spec.label.to_string(),
        note: spec.note.to_string(),
        bytes: spec.bytes,
        bundled: spec.bundled,
        present: std::fs::metadata(&full).map(|m| m.len() == spec.bytes).unwrap_or(false),
        partial_bytes: std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0),
    }
}

fn part_path(dir: &Path, spec: &ModelSpec) -> PathBuf {
    dir.join(format!("{}.part", spec.file))
}

/// sha256 of a file, reported as it goes so a five-gigabyte check is not a
/// frozen window.
pub fn sha256_file(path: &Path, mut on: impl FnMut(u64, u64)) -> Result<String, String> {
    let mut fh = std::fs::File::open(path).map_err(|e| format!("cannot read {:?}: {}", path, e))?;
    let total = fh.metadata().map(|m| m.len()).unwrap_or(0);
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut done = 0u64;
    loop {
        let n = fh.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        done += n as u64;
        on(done, total);
    }
    Ok(hex(&hasher.finalize()))
}

/// sha256 of bytes already in memory.
pub fn sha256_bytes(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex(&h.finalize())
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// Make sure the model file is present and correct, downloading it if it is
/// not. Returns the path to the verified file.
///
/// A file that is already there and already correct costs one read and no
/// network access at all: this is what makes reinstalling and upgrading not
/// re-download five gigabytes.
pub fn ensure_model(
    spec: &ModelSpec,
    dir: &Path,
    cancel: Arc<AtomicBool>,
    mut on: impl FnMut(Progress),
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {:?}: {}", dir, e))?;
    let full = dir.join(spec.file);

    if full.exists() {
        on(Progress::Checking { file: spec.file.to_string() });
        let got = sha256_file(&full, |done, total| {
            on(Progress::Verifying {
                file: spec.file.to_string(),
                done,
                total,
                percent: pct(done, total),
            })
        })?;
        if got == spec.sha256 {
            let bytes = std::fs::metadata(&full).map(|m| m.len()).unwrap_or(0);
            on(Progress::Done { file: spec.file.to_string(), bytes, skipped: true });
            return Ok(full);
        }
        // Present but wrong. Say so and fetch it again rather than running a
        // model file we cannot identify.
        let _ = std::fs::remove_file(&full);
    }

    let part = part_path(dir, spec);
    let downloaded = fetch(spec, &part, cancel.clone(), &mut on)?;
    let _ = downloaded;

    on(Progress::Checking { file: spec.file.to_string() });
    let got = sha256_file(&part, |done, total| {
        on(Progress::Verifying { file: spec.file.to_string(), done, total, percent: pct(done, total) })
    })?;
    if got != spec.sha256 {
        let _ = std::fs::remove_file(&part);
        let msg = format!(
            "the downloaded file does not match its checksum and has been deleted. \
             Expected {}, got {}.",
            spec.sha256, got
        );
        on(Progress::Failed { file: spec.file.to_string(), message: msg.clone() });
        return Err(msg);
    }

    // Atomic rename: the full name never exists holding half a file.
    std::fs::rename(&part, &full)
        .map_err(|e| format!("cannot put the model in place: {}", e))?;
    let bytes = std::fs::metadata(&full).map(|m| m.len()).unwrap_or(0);
    on(Progress::Done { file: spec.file.to_string(), bytes, skipped: false });
    Ok(full)
}

fn pct(done: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        (done as f64) * 100.0 / (total as f64)
    }
}

/// Fetch into the partial file, resuming whatever is already there.
fn fetch(
    spec: &ModelSpec,
    part: &Path,
    cancel: Arc<AtomicBool>,
    on: &mut impl FnMut(Progress),
) -> Result<u64, String> {
    if !allowed(spec.url) {
        // Unreachable through the public API; here so that it stays unreachable.
        return Err(format!("refusing to fetch {}: not a pinned model URL", spec.url));
    }

    let have = std::fs::metadata(part).map(|m| m.len()).unwrap_or(0);
    if have > spec.bytes {
        // A partial larger than the whole file is not a partial.
        let _ = std::fs::remove_file(part);
        return fetch(spec, part, cancel, on);
    }
    if have == spec.bytes {
        return Ok(have);
    }

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(30))
        .timeout_read(Duration::from_secs(120))
        .user_agent("pastor-bible/0.0.1 (+https://github.com/haomuch1/pastor-bible)")
        .build();

    let mut req = agent.get(spec.url);
    if have > 0 {
        req = req.set("Range", &format!("bytes={}-", have));
    }
    let resp = req.call().map_err(|e| format!("cannot reach the model host: {}", e))?;

    // A server that ignores Range answers 200 and sends the whole file; then
    // the partial must be thrown away rather than appended to.
    let resumed = resp.status() == 206 && have > 0;
    let total = if resumed {
        content_range_total(resp.header("Content-Range")).unwrap_or(spec.bytes)
    } else {
        resp.header("Content-Length").and_then(|v| v.parse::<u64>().ok()).unwrap_or(spec.bytes)
    };
    if total != spec.bytes {
        return Err(format!(
            "the host offers {} bytes for {} and this build expects {}. \
             Refusing to download a file that is not the pinned one.",
            total, spec.file, spec.bytes
        ));
    }

    let mut fh = if resumed {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .open(part)
            .map_err(|e| format!("cannot open {:?}: {}", part, e))?;
        f.seek(SeekFrom::Start(have)).map_err(|e| e.to_string())?;
        f
    } else {
        std::fs::File::create(part).map_err(|e| format!("cannot create {:?}: {}", part, e))?
    };
    let start_at = if resumed { have } else { 0 };

    let mut reader = resp.into_reader();
    let mut buf = vec![0u8; 1 << 20];
    let mut done = start_at;
    let started = Instant::now();
    let mut last_report = Instant::now();
    on(Progress::Downloading {
        file: spec.file.to_string(),
        done,
        total: spec.bytes,
        percent: pct(done, spec.bytes),
        bytes_per_second: 0.0,
        eta_seconds: None,
        resumed_from: start_at,
    });

    loop {
        if cancel.load(Ordering::SeqCst) {
            let _ = fh.flush();
            // The partial stays on disk: cancelling a download should not throw
            // away four gigabytes the reader already waited for.
            return Err("cancelled".to_string());
        }
        let n = reader.read(&mut buf).map_err(|e| format!("the download broke off: {}", e))?;
        if n == 0 {
            break;
        }
        fh.write_all(&buf[..n]).map_err(|e| format!("cannot write the model file: {}", e))?;
        done += n as u64;
        let first_chunk = done == start_at + n as u64;
        if first_chunk || last_report.elapsed() >= Duration::from_millis(250) {
            last_report = Instant::now();
            let secs = started.elapsed().as_secs_f64().max(1e-3);
            let rate = (done - start_at) as f64 / secs;
            on(Progress::Downloading {
                file: spec.file.to_string(),
                done,
                total: spec.bytes,
                percent: pct(done, spec.bytes),
                bytes_per_second: rate,
                eta_seconds: if rate > 0.0 {
                    Some((spec.bytes.saturating_sub(done)) as f64 / rate)
                } else {
                    None
                },
                resumed_from: start_at,
            });
        }
    }
    fh.flush().map_err(|e| e.to_string())?;

    if done != spec.bytes {
        return Err(format!(
            "the download stopped at {} of {} bytes. Run it again and it will \
             carry on from there.",
            done, spec.bytes
        ));
    }
    Ok(done)
}

fn content_range_total(header: Option<&str>) -> Option<u64> {
    // "bytes 100-199/5027783488"
    header?.rsplit('/').next()?.trim().parse().ok()
}
