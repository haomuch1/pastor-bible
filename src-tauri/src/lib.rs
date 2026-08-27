//! The Tauri shell: the window, and the commands it may call.
//!
//! Everything of substance is in `pastor-bible-core`. This file resolves where
//! things live on this machine, holds the open session and the open user.db,
//! and exposes a small set of commands. It contains no retrieval, no
//! verification and no prompt.
//!
//! Two rules from the plan show up here as code rather than as intention. The
//! frontend keeps nothing: there is no browser storage anywhere, and every
//! setting and every answer goes through user.db by way of a command. And both
//! sidecars are stopped when the window closes, by a handler and by the Job
//! Object underneath it, so nothing is left holding five gigabytes.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{Emitter, Manager, State};

use pastor_bible_core::api::Answer;
use pastor_bible_core::compute::{self, ComputeChoice};
use pastor_bible_core::download::{self, ModelStatus, Progress};
use pastor_bible_core::hardware::{self, Hardware};
use pastor_bible_core::pipeline::{Engine, QueryMode, Settings, DEFAULT_CHAT_GGUF, EMBED_GGUF};
use pastor_bible_core::retrieve::CanonMode;
use pastor_bible_core::session::{SelfTestResult, Session, Stage, SELF_TEST_IDS};
use pastor_bible_core::userdb::{HistoryDetail, HistoryRow, UserDb};
use pastor_bible_core::{paths, verifier};

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Where this installation keeps its things.
#[derive(Clone, Debug, Serialize)]
pub struct AppPaths {
    pub app_data: String,
    pub user_db: String,
    /// Where the chat model is downloaded to and looked for. Application data,
    /// because it is the one file the reader chooses and may replace.
    pub models: String,
    /// The embedding model, resolved as a resource of the application rather
    /// than as something in `models`. See `resolve_paths`.
    pub embed_model: String,
    pub index_db: String,
    pub llama_server: String,
    pub logs: String,
}

pub struct AppState {
    paths: AppPaths,
    session: Arc<Mutex<Option<Session>>>,
    db: Arc<Mutex<UserDb>>,
    /// Set while a question is running, so a second Ask is refused rather than
    /// queued behind the first.
    busy: Arc<AtomicBool>,
    /// True between Stop being pressed and the two-second deadline passing.
    cancelling: Arc<AtomicBool>,
    download_cancel: Arc<AtomicBool>,
    /// Written by the session as soon as retrieval is done, read by the window
    /// while the answer is still being written.
    retrieved: pastor_bible_core::session::RetrievedSlot,
    /// The answering model's process id, so Stop can stop it.
    chat_pid: pastor_bible_core::session::ChatPidSlot,
    /// The bundled search model, checked against its pinned sha256 in the
    /// background at startup. `None` while that is still running; `Some(Err)`
    /// carries a message the reader can act on.
    embed_checksum: Arc<Mutex<Option<Result<(), String>>>>,
    /// The last graphics-card decision, so the probe runs once rather than
    /// before every question. Cleared when the setting or the model changes.
    compute: Arc<Mutex<Option<ComputeChoice>>>,
}

fn resolve_paths(app: &tauri::AppHandle) -> Result<AppPaths, String> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("cannot find the application data directory: {}", e))?;
    std::fs::create_dir_all(&app_data).map_err(|e| e.to_string())?;

    // In a built app the index and the sidecar are resources beside the
    // program. In development they are where the repository keeps them, and
    // TPB_* overrides both, which is how the measurements point at a fresh
    // application data directory without moving five gigabytes of models.
    let resource = app.path().resource_dir().ok();
    let index_db = first_existing(&[
        std::env::var("TPB_INDEX_DB").ok(),
        resource.as_ref().map(|r| r.join("resources").join("index.db").to_string_lossy().into_owned()),
        resource.as_ref().map(|r| r.join("index.db").to_string_lossy().into_owned()),
        Some(paths::index_db()),
    ])
    .ok_or("index.db was not found. It ships with the installer.")?;

    // One server, in one directory, with its libraries beside it — the Vulkan
    // backend among them, which is the whole difference between llama.cpp's
    // "CPU build" and its "Vulkan build". See core/src/compute.rs. The
    // directory matters: Windows resolves a DLL from the executable's own
    // folder first, so the server and its libraries cannot be separated.
    let server_name = if cfg!(windows) { "llama-server.exe" } else { "llama-server" };
    let llama_server = first_existing(&[
        std::env::var("TPB_LLAMA_SERVER").ok(),
        resource
            .as_ref()
            .map(|r| r.join("resources").join("llama").join(server_name).to_string_lossy().into_owned()),
        resource.as_ref().map(|r| r.join("llama").join(server_name).to_string_lossy().into_owned()),
        Some(paths::llama_server()),
    ])
    .ok_or("the model server was not found. It ships with the installer.")?;

    // The chat model is the one file the reader chooses, so it lives in
    // application data where the downloader puts it. In a development build the
    // repository's own models/ directory is accepted as well, because that is
    // where five gigabytes already sit on a machine that builds this app and
    // nobody should have to copy them to run `tauri dev`. A release build never
    // looks there: the reader's copy is the reader's copy.
    let app_models = app_data.join("models");
    let models = match std::env::var("TPB_MODEL_DIR") {
        Ok(dir) => dir,
        Err(_) => {
            let repo = std::path::PathBuf::from(paths::model_dir());
            if cfg!(debug_assertions) && !app_models.join(DEFAULT_CHAT_GGUF).exists() && repo.join(DEFAULT_CHAT_GGUF).exists() {
                repo.to_string_lossy().into_owned()
            } else {
                app_models.to_string_lossy().into_owned()
            }
        }
    };

    // The embedding model is a bundled resource, not a download. It is read on
    // every question, it is 262 MB, and DECISIONS records it as shipping with
    // the installer; looking for it in application data is what produced the
    // defect P5.1 opened with, because nothing ever puts it there. The last
    // candidate is returned even when it does not exist, so that the check at
    // startup can name the file the reader is missing rather than fail here and
    // leave the window unable to open at all.
    let embed_model = first_existing(&[
        std::env::var("TPB_EMBED_MODEL").ok(),
        resource.as_ref().map(|r| r.join("resources").join(EMBED_GGUF).to_string_lossy().into_owned()),
        resource.as_ref().map(|r| r.join(EMBED_GGUF).to_string_lossy().into_owned()),
        Some(paths::resource_file(EMBED_GGUF)),
        Some(std::path::Path::new(&models).join(EMBED_GGUF).to_string_lossy().into_owned()),
    ])
    .unwrap_or_else(|| paths::resource_file(EMBED_GGUF));

    let logs = app_data.join("logs").to_string_lossy().into_owned();

    Ok(AppPaths {
        user_db: app_data.join("user.db").to_string_lossy().into_owned(),
        app_data: app_data.to_string_lossy().into_owned(),
        models,
        embed_model,
        index_db,
        llama_server,
        logs,
    })
}

fn first_existing(candidates: &[Option<String>]) -> Option<String> {
    candidates
        .iter()
        .flatten()
        .find(|p| std::path::Path::new(p).exists())
        .cloned()
}

// ------------------------------------------------------------ model files

/// Is every model file this app needs where it is expected to be?
///
/// The defect this replaced showed the reader `The system cannot find the path
/// specified (os error 3)` with a path in it, two and a half minutes after they
/// asked a question. A missing file is not an unexpected condition and it does
/// not deserve an error code: it deserves a sentence naming the file and saying
/// what puts it back. Checked at startup and again before a question runs, so
/// the message arrives before the wait rather than after it.
fn model_problem(
    paths: &AppPaths,
    settings: &AppSettings,
    checksum: &Option<Result<(), String>>,
) -> Option<String> {
    let embed = download::model("embedding").expect("the embedding model is pinned");
    let path = std::path::Path::new(&paths.embed_model);
    if !path.exists() {
        return Some(format!(
            "The search model is missing, so nothing can be looked up.\n\n\
             The file is {}, and The Pastor Bible expected it here:\n\
             {}\n\n\
             It ships with the application and is never downloaded. \
             Reinstalling puts it back.",
            embed.file,
            tidy(&paths.embed_model)
        ));
    }
    if let Some(Err(why)) = checksum {
        return Some(why.clone());
    }

    let spec = download::model(&settings.model)?;
    let chat = std::path::Path::new(&paths.models).join(spec.file);
    if !chat.exists() {
        return Some(format!(
            "The answering model has not been downloaded yet.\n\n\
             The file is {} ({}), and The Pastor Bible expected it here:\n\
             {}\n\n\
             Open Settings to download it. Nothing else is needed.",
            spec.file,
            human_bytes(spec.bytes),
            tidy(&chat.to_string_lossy())
        ));
    }
    None
}

/// A path a reader can read.
///
/// The development fallbacks are built by joining `..` onto a manifest
/// directory, which is correct and unreadable: the message that names a missing
/// file should not also make the reader parse `core\..\..\src-tauri`. The
/// components are resolved textually, because the file being named is the one
/// that is not there and cannot be canonicalised.
fn tidy(path: &str) -> String {
    let p = std::path::Path::new(path);
    let mut out = std::path::PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out.to_string_lossy().into_owned()
}

fn human_bytes(n: u64) -> String {
    if n >= 1 << 30 {
        format!("{:.1} GB", n as f64 / (1u64 << 30) as f64)
    } else {
        format!("{:.0} MB", n as f64 / (1u64 << 20) as f64)
    }
}

/// Check the bundled search model against the sha256 pinned in download.rs.
///
/// Off the startup path in a thread of its own: the file is 262 MB and the
/// window opens in under a second, which is worth keeping. A file that is
/// present but is not the file we pinned is the one case a size check would
/// pass and a reader would never find out about, so it is worth the read.
fn verify_embed_model(path: String, slot: Arc<Mutex<Option<Result<(), String>>>>) {
    let spec = download::model("embedding").expect("the embedding model is pinned");
    let result = (|| -> Result<(), String> {
        let p = std::path::Path::new(&path);
        if !p.exists() {
            return Ok(()); // `model_problem` already says so, more usefully.
        }
        let got = download::sha256_file(p, |_, _| {})?;
        if got == spec.sha256 {
            Ok(())
        } else {
            Err(format!(
                "The search model on this computer is not the file The Pastor Bible \
                 was built with, so it is not being used.\n\n\
                 The file is {}, and it is here:\n\
                 {}\n\n\
                 Reinstalling replaces it.",
                spec.file,
                tidy(&path)
            ))
        }
    })();
    if let Ok(mut g) = slot.lock() {
        *g = Some(result);
    }
}

// ---------------------------------------------------------------- settings

/// The settings the app understands, with their defaults. Read from user.db
/// every time rather than cached, because there is one reader and no contention
/// and a cache is one more thing that can disagree with the truth.
#[derive(Clone, Debug, Serialize)]
pub struct AppSettings {
    pub canon: String,
    pub model: String,
    pub compute: String,
}

fn read_settings(db: &UserDb) -> AppSettings {
    AppSettings {
        canon: db.get_setting("canon").unwrap_or_else(|| "66".into()),
        model: db.get_setting("model").unwrap_or_else(|| "standard".into()),
        compute: db.get_setting("compute").unwrap_or_else(|| "auto".into()),
    }
}

fn engine_settings(
    paths: &AppPaths,
    s: &AppSettings,
    compute: &ComputeChoice,
) -> Result<Settings, String> {
    let spec = download::model(&s.model).ok_or_else(|| format!("unknown model {:?}", s.model))?;
    Ok(Settings {
        index_db: paths.index_db.clone(),
        llama_server: paths.llama_server.clone(),
        chat_model: std::path::Path::new(&paths.models).join(spec.file).to_string_lossy().into_owned(),
        embed_model: paths.embed_model.clone(),
        prompts_dir: paths::prompts_dir(),
        crisis_terms: paths::crisis_terms(),
        crisis_note: paths::crisis_note(),
        log_dir: Some(paths.logs.clone()),
        canon: CanonMode::parse(&s.canon)?,
        query_mode: QueryMode::Raw,
        chat_ctx: 8192,
        threads: None,
        // The measured difference is 12 seconds against 178, so this is the
        // single largest thing the app can do for the reader's wait. The
        // decision is `compute::decide`'s; this only carries it.
        gpu_layers: compute.gpu_layers(),
        allow_both_servers: true,
    })
}

// ------------------------------------------------------------------ compute

/// Which processor will answer, decided once and remembered.
///
/// The probe runs `llama-server --list-devices`, which loads no model and takes
/// about a second. Nothing about a machine's graphics card changes between two
/// questions, so it runs when the answer is first wanted and again only when
/// the reader changes the compute setting or the model.
fn compute_choice(state: &AppState, settings: &AppSettings) -> Result<ComputeChoice, String> {
    {
        let cached = state.compute.lock().map_err(lock)?;
        if let Some(c) = cached.as_ref() {
            if c.mode == settings.compute {
                return Ok(c.clone());
            }
        }
    }
    let needs = download::model(&settings.model).map(|m| m.vram_mib).unwrap_or(u64::MAX);
    let choice = compute::decide(&settings.compute, &state.paths.llama_server, needs);
    *state.compute.lock().map_err(lock)? = Some(choice.clone());
    Ok(choice)
}

/// What Settings shows: the mode asked for, the processor that will run, the
/// device found, and one sentence saying why.
#[tauri::command]
fn compute_status(state: State<'_, AppState>) -> Result<ComputeChoice, String> {
    let settings = {
        let db = state.db.lock().map_err(lock)?;
        read_settings(&db)
    };
    compute_choice(&state, &settings)
}

// ---------------------------------------------------------------- commands

#[derive(Clone, Debug, Serialize)]
pub struct AppInfo {
    pub app_version: String,
    pub index_version: String,
    pub model_id: String,
    pub model_file: String,
    pub embedding_model: String,
    pub disclaimer: String,
    pub crisis_note: String,
    pub offline_statement: String,
    pub authors: Vec<String>,
    pub license: String,
    pub sources: Vec<[String; 2]>,
    pub reference_hardware: String,
    pub paths: AppPaths,
    pub prompt_versions: Vec<(String, String)>,
}

#[tauri::command]
fn app_info(state: State<'_, AppState>) -> Result<AppInfo, String> {
    let db = state.db.lock().map_err(lock)?;
    let s = read_settings(&db);
    let spec = download::model(&s.model).ok_or("unknown model")?;
    let index_version = {
        let sess = state.session.lock().map_err(lock)?;
        match sess.as_ref() {
            Some(x) => x.engine.retriever.index.index_version.clone(),
            None => pastor_bible_core::index::Index::open(&state.paths.index_db)
                .map(|i| i.index_version)
                .unwrap_or_else(|_| "unknown".into()),
        }
    };
    let r = hardware::REFERENCE;
    Ok(AppInfo {
        app_version: APP_VERSION.to_string(),
        index_version,
        model_id: s.model.clone(),
        model_file: spec.file.to_string(),
        embedding_model: EMBED_GGUF.to_string(),
        disclaimer: read_text(&paths::data_dir(), "disclaimer.txt")?,
        crisis_note: read_text(&paths::data_dir(), "crisis_note.txt")?,
        offline_statement:
            "The Pastor Bible works entirely on this computer. After the one-time model \
             download it makes no connection to anything, and nothing you type is ever sent \
             anywhere."
                .to_string(),
        authors: vec!["Jared".to_string(), "Claude (Anthropic)".to_string()],
        license: "Apache-2.0".to_string(),
        sources: vec![
            ["World English Bible, Classic".into(), "public domain".into()],
            ["Treasury of Scripture Knowledge".into(), "public domain".into()],
            ["Nave's Topical Bible".into(), "public domain".into()],
            ["llama.cpp".into(), "MIT".into()],
            ["Tauri".into(), "MIT or Apache-2.0".into()],
            ["Qwen3 (answering model)".into(), "Apache-2.0".into()],
            ["nomic-embed-text-v1.5 (search model)".into(), "Apache-2.0".into()],
        ],
        reference_hardware: format!("{}, {}, {:.0} GB RAM, {}", r.cpu, r.gpu, r.ram_gb, r.os),
        paths: state.paths.clone(),
        prompt_versions: Vec::new(),
    })
}

fn read_text(dir: &str, name: &str) -> Result<String, String> {
    std::fs::read_to_string(std::path::Path::new(dir).join(name))
        .map(|s| s.trim().to_string())
        .map_err(|e| format!("cannot read {}: {}", name, e))
}

#[tauri::command]
fn hardware_check(state: State<'_, AppState>) -> Hardware {
    hardware::probe(&state.paths.app_data)
}

#[derive(Clone, Debug, Serialize)]
pub struct StartupState {
    pub first_run: bool,
    pub models: Vec<ModelStatus>,
    pub chat_model_present: bool,
    pub embedding_model_present: bool,
    pub settings: AppSettings,
    pub self_test: Option<SelfTestResult>,
    pub history_count: i64,
    /// A plain sentence naming a model file that is missing or wrong, or
    /// `None` when everything the app needs is where it should be.
    pub model_problem: Option<String>,
}

#[tauri::command]
fn startup_state(state: State<'_, AppState>) -> Result<StartupState, String> {
    let db = state.db.lock().map_err(lock)?;
    let settings = read_settings(&db);
    let dir = std::path::PathBuf::from(&state.paths.models);
    let models: Vec<ModelStatus> =
        download::MODELS.iter().map(|m| download::status(m, &dir)).collect();
    let chat_present = models.iter().find(|m| m.id == settings.model).map(|m| m.present).unwrap_or(false);
    let embed_present = models.iter().find(|m| m.id == "embedding").map(|m| m.present).unwrap_or(false);
    let self_test: Option<SelfTestResult> =
        db.get_setting("self_test").and_then(|s| serde_json::from_str(&s).ok());
    let checksum = state.embed_checksum.lock().map_err(lock)?.clone();
    let model_problem = model_problem(&state.paths, &settings, &checksum);
    Ok(StartupState {
        model_problem,
        first_run: db.get_setting("first_run_done").is_none(),
        chat_model_present: chat_present,
        embedding_model_present: embed_present,
        models,
        settings,
        self_test,
        history_count: db.count(),
    })
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    let db = state.db.lock().map_err(lock)?;
    Ok(read_settings(&db))
}

#[tauri::command]
fn set_setting(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<AppSettings, String> {
    const ALLOWED: &[&str] = &["canon", "model", "compute", "first_run_done"];
    if !ALLOWED.contains(&key.as_str()) {
        return Err(format!("unknown setting {:?}", key));
    }
    {
        let db = state.db.lock().map_err(lock)?;
        db.set_setting(&key, &value)?;
    }
    // A changed model may not fit the card the last one fitted, and a changed
    // compute setting is a direct instruction; either way the decision is made
    // again rather than carried over.
    if key == "model" || key == "compute" {
        *state.compute.lock().map_err(lock)? = None;
    }
    // Canon and model change how the next question is answered, so the open
    // session is rebuilt rather than left holding the old ones.
    if key == "canon" || key == "model" || key == "compute" {
        let s = {
            let db = state.db.lock().map_err(lock)?;
            read_settings(&db)
        };
        let choice = compute_choice(&state, &s)?;
        let mut sess = state.session.lock().map_err(lock)?;
        if let Some(existing) = sess.as_mut() {
            existing.engine.settings = engine_settings(&state.paths, &s, &choice)?;
        }
    }
    let db = state.db.lock().map_err(lock)?;
    Ok(read_settings(&db))
}

#[tauri::command]
async fn download_model(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<String, String> {
    let dir = std::path::PathBuf::from(&state.paths.models);
    let cancel = state.download_cancel.clone();
    cancel.store(false, Ordering::SeqCst);
    let spec = download::model(&id).ok_or_else(|| format!("unknown model {:?}", id))?;
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        download::ensure_model(spec, &dir, cancel, |p: Progress| {
            let _ = app2.emit("download-progress", &p);
        })
        .map(|p| p.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn cancel_download(state: State<'_, AppState>) {
    state.download_cancel.store(true, Ordering::SeqCst);
}

#[tauri::command]
async fn ask(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    question: String,
) -> Result<Answer, String> {
    let question = question.trim().to_string();
    if question.is_empty() {
        return Err("Type a question first.".to_string());
    }
    {
        let settings = {
            let d = state.db.lock().map_err(lock)?;
            read_settings(&d)
        };
        let checksum = state.embed_checksum.lock().map_err(lock)?.clone();
        if let Some(why) = model_problem(&state.paths, &settings, &checksum) {
            return Err(why);
        }
    }
    if state.busy.swap(true, Ordering::SeqCst) {
        return Err("An answer is already being written. Wait for it, or cancel it.".to_string());
    }
    let session = state.session.clone();
    let db = state.db.clone();
    let busy = state.busy.clone();
    let paths = state.paths.clone();
    let settings = {
        let d = db.lock().map_err(lock)?;
        read_settings(&d)
    };
    let choice = compute_choice(&state, &settings)?;
    let engine_settings = engine_settings(&paths, &settings, &choice)?;
    let slot = state.retrieved.clone();
    let pid_slot = state.chat_pid.clone();

    let out = tauri::async_runtime::spawn_blocking(move || {
        let result = (|| -> Result<Answer, String> {
            let mut guard = session.lock().map_err(|_| "the session is unavailable".to_string())?;
            if guard.is_none() {
                *guard = Some(Session::with_slot(Engine::open(engine_settings)?, slot, pid_slot));
            }
            let s = guard.as_mut().unwrap();
            let app2 = app.clone();
            let mut on = move |st: Stage| {
                let _ = app2.emit("ask-stage", &st);
            };
            let answer = s.ask(&question, &mut on)?;
            let d = db.lock().map_err(|_| "user.db is unavailable".to_string())?;
            let _ = d.save_answer(&answer)?;
            Ok(answer)
        })();
        busy.store(false, Ordering::SeqCst);
        result
    })
    .await
    .map_err(|e| e.to_string())?;
    out
}

/// The passages for the question being answered, as soon as they exist.
///
/// The window calls this the moment it hears that retrieval is done, which is
/// about forty milliseconds in, and puts them on screen while the answer is
/// still being written. Two and a half minutes of waiting becomes two and a
/// half minutes of reading.
#[derive(Clone, Debug, Serialize)]
pub struct Retrieved {
    pub passages: Vec<pastor_bible_core::api::PassageOut>,
    pub topic_groups: Vec<pastor_bible_core::api::TopicGroup>,
}

#[tauri::command]
fn retrieved_passages(state: State<'_, AppState>) -> Result<Option<Retrieved>, String> {
    // Deliberately does not touch state.session: `ask` holds that for the whole
    // of a generation, and this has to answer during one.
    let mut slot = state.retrieved.lock().map_err(lock)?;
    Ok(slot.take().map(|(passages, topic_groups)| Retrieved { passages, topic_groups }))
}

/// Stop the answer that is being written.
///
/// Asking politely is the first move: the flag is set, and the thread reading
/// the answer sees it between chunks and closes the connection, which is what
/// makes llama-server abandon the slot. During prompt processing, though, no
/// chunk arrives for tens of seconds and the reader is blocked inside a read
/// that will not look at the flag. Measured on 2026-08-26, that was 16.3
/// seconds from Stop to the call returning, which is not what Stop means to
/// the person who pressed it. So two seconds later, if the answer is still
/// running, the answering model is stopped outright and the session starts it
/// again. The cost is one model load, about four seconds, on a path the reader
/// chose to abandon anyway.
#[tauri::command]
fn cancel_ask(state: State<'_, AppState>) -> Result<(), String> {
    {
        // Not the session lock: `ask` holds that for the whole generation.
        // Setting the flag needs nothing but the flag.
        let guard = state.session.try_lock();
        if let Ok(g) = guard {
            if let Some(s) = g.as_ref() {
                s.request_cancel();
            }
        }
    }
    state.cancelling.store(true, Ordering::SeqCst);
    let busy = state.busy.clone();
    let cancelling = state.cancelling.clone();
    let pid_slot = state.chat_pid.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(2));
        if busy.load(Ordering::SeqCst) {
            let pid = pid_slot.lock().ok().and_then(|p| *p);
            if let Some(pid) = pid {
                pastor_bible_core::sidecar::terminate(pid);
            }
        }
        cancelling.store(false, Ordering::SeqCst);
    });
    Ok(())
}

#[tauri::command]
async fn run_self_test(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<SelfTestResult, String> {
    let questions = self_test_questions()?;
    let session = state.session.clone();
    let db = state.db.clone();
    let paths = state.paths.clone();
    let settings = {
        let d = db.lock().map_err(lock)?;
        read_settings(&d)
    };
    let choice = compute_choice(&state, &settings)?;
    let es = engine_settings(&paths, &settings, &choice)?;
    let slot = state.retrieved.clone();
    let pid_slot = state.chat_pid.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut guard = session.lock().map_err(|_| "the session is unavailable".to_string())?;
        if guard.is_none() {
            *guard = Some(Session::with_slot(Engine::open(es)?, slot, pid_slot));
        }
        let s = guard.as_mut().unwrap();
        let app2 = app.clone();
        let mut on = move |st: Stage| {
            let _ = app2.emit("ask-stage", &st);
        };
        let result = s.self_test(&questions, &mut on)?;
        let d = db.lock().map_err(|_| "user.db is unavailable".to_string())?;
        d.set_setting("self_test", &serde_json::to_string(&result).unwrap_or_default())?;
        if result.passed {
            d.set_setting("first_run_done", "1")?;
        }
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The three canned questions, read from the evaluation set so the self-test
/// asks what a reader would ask rather than something chosen to pass.
fn self_test_questions() -> Result<Vec<(String, String)>, String> {
    let path = std::path::Path::new(&paths::data_dir()).join("eval").join("questions.json");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read the self-test questions: {}", e))?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for want in SELF_TEST_IDS {
        let found = v["smoke"]
            .as_array()
            .and_then(|a| a.iter().find(|q| q["id"] == want))
            .and_then(|q| q["question"].as_str());
        match found {
            Some(q) => out.push((want.to_string(), q.to_string())),
            None => return Err(format!("the self-test question {} is missing", want)),
        }
    }
    Ok(out)
}

#[tauri::command]
fn finish_first_run(state: State<'_, AppState>) -> Result<(), String> {
    let db = state.db.lock().map_err(lock)?;
    db.set_setting("first_run_done", "1")
}

// ---------------------------------------------------------------- history

#[tauri::command]
fn history_list(
    state: State<'_, AppState>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<HistoryRow>, String> {
    let db = state.db.lock().map_err(lock)?;
    db.list(limit.unwrap_or(50), offset.unwrap_or(0))
}

#[tauri::command]
fn history_search(state: State<'_, AppState>, query: String) -> Result<Vec<HistoryRow>, String> {
    let db = state.db.lock().map_err(lock)?;
    db.search(&query, 100)
}

#[tauri::command]
fn history_get(state: State<'_, AppState>, id: i64) -> Result<Option<HistoryDetail>, String> {
    let db = state.db.lock().map_err(lock)?;
    let session = state.session.lock().map_err(lock)?;
    match session.as_ref() {
        Some(s) => db.get(id, &s.engine.retriever.index),
        None => {
            let index = pastor_bible_core::index::Index::open(&state.paths.index_db)?;
            db.get(id, &index)
        }
    }
}

#[tauri::command]
fn history_delete(state: State<'_, AppState>, id: i64) -> Result<bool, String> {
    let db = state.db.lock().map_err(lock)?;
    db.delete(id)
}

#[tauri::command]
fn history_clear(state: State<'_, AppState>) -> Result<usize, String> {
    let db = state.db.lock().map_err(lock)?;
    db.delete_all()
}

/// The history as a file the reader keeps.
///
/// `format` is "txt" for the plain-text copy, which is what someone prints, or
/// "xlsx" for the workbook, which is what someone sorts and hands on. Both are
/// written from the same reader in user.db, so the two cannot come to disagree
/// about the same entry, and both read their verse text from index.db.
#[tauri::command]
fn history_export(
    state: State<'_, AppState>,
    path: String,
    format: Option<String>,
) -> Result<String, String> {
    let db = state.db.lock().map_err(lock)?;
    let session = state.session.lock().map_err(lock)?;
    let opened;
    let index = match session.as_ref() {
        Some(s) => &s.engine.retriever.index,
        None => {
            opened = pastor_bible_core::index::Index::open(&state.paths.index_db)?;
            &opened
        }
    };
    match format.as_deref().unwrap_or("txt") {
        "txt" => {
            let text = db.export_text(index)?;
            std::fs::write(&path, text.as_bytes())
                .map_err(|e| format!("cannot write {}: {}", path, e))?;
        }
        "xlsx" => pastor_bible_core::spreadsheet::write(&db, index, &path)?,
        other => return Err(format!("unknown export format {:?}", other)),
    }
    Ok(path)
}

/// A whole chapter, for reading a cited passage in its place.
///
/// The verse text comes from index.db, which is the same rule the answer
/// itself is built under: nothing a model wrote reaches it. The canon setting
/// decides only where Previous and Next may go, never whether the chapter the
/// reader asked for is returned; a citation they are following always opens.
#[tauri::command]
fn chapter(
    state: State<'_, AppState>,
    book_id: i64,
    chapter: i64,
) -> Result<Option<pastor_bible_core::api::ChapterOut>, String> {
    let canon = {
        let db = state.db.lock().map_err(lock)?;
        read_settings(&db).canon
    };
    let session = state.session.lock().map_err(lock)?;
    Ok(match session.as_ref() {
        Some(s) => s.engine.retriever.index.chapter(book_id, chapter, &canon),
        None => pastor_bible_core::index::Index::open(&state.paths.index_db)?
            .chapter(book_id, chapter, &canon),
    })
}

/// The crisis note, for the panel above an answer. Loaded from the same file
/// README quotes, so the two cannot drift.
#[tauri::command]
fn crisis_note() -> Result<String, String> {
    read_text(&paths::data_dir(), "crisis_note.txt")
}

/// The tokens an answer cites, so the frontend marks the same passages the
/// verifier saw rather than parsing markdown itself.
#[tauri::command]
fn cited_tokens(text: String) -> Vec<String> {
    verifier::cited_tokens(&text)
}

#[tauri::command]
fn shutdown_models(state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.session.lock().map_err(lock)?;
    if let Some(s) = guard.as_mut() {
        s.shutdown();
    }
    *guard = None;
    Ok(())
}

fn lock<T>(_: T) -> String {
    "the application is busy; try again".to_string()
}

// ------------------------------------------------------------------- setup

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let paths = resolve_paths(&app.handle())?;
            let db = UserDb::open(&paths.user_db)?;
            let embed_checksum: Arc<Mutex<Option<Result<(), String>>>> = Arc::new(Mutex::new(None));
            {
                let (p, slot) = (paths.embed_model.clone(), embed_checksum.clone());
                std::thread::spawn(move || verify_embed_model(p, slot));
            }
            app.manage(AppState {
                embed_checksum,
                paths,
                session: Arc::new(Mutex::new(None)),
                db: Arc::new(Mutex::new(db)),
                busy: Arc::new(AtomicBool::new(false)),
                cancelling: Arc::new(AtomicBool::new(false)),
                download_cancel: Arc::new(AtomicBool::new(false)),
                retrieved: Arc::new(Mutex::new(None)),
                chat_pid: Arc::new(Mutex::new(None)),
                compute: Arc::new(Mutex::new(None)),
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            // PLAN 7.4: no background process remains. The Job Object the
            // sidecar sets up would kill them anyway when this process exits;
            // this stops them at the moment the window goes, so the machine is
            // not holding nine gigabytes while the app is closing.
            if matches!(
                event,
                tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed
            ) {
                if let Some(state) = window.app_handle().try_state::<AppState>() {
                    if let Ok(mut guard) = state.session.lock() {
                        if let Some(s) = guard.as_mut() {
                            s.shutdown();
                        }
                        *guard = None;
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            hardware_check,
            startup_state,
            get_settings,
            set_setting,
            download_model,
            cancel_download,
            ask,
            retrieved_passages,
            cancel_ask,
            run_self_test,
            finish_first_run,
            history_list,
            history_search,
            history_get,
            history_delete,
            history_clear,
            history_export,
            chapter,
            compute_status,
            crisis_note,
            cited_tokens,
            shutdown_models,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
