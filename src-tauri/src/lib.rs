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
use pastor_bible_core::download::{self, ModelStatus, Progress};
use pastor_bible_core::hardware::{self, Hardware};
use pastor_bible_core::pipeline::{Engine, QueryMode, Settings, EMBED_GGUF};
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
    pub models: String,
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
    download_cancel: Arc<AtomicBool>,
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

    let server_name = if cfg!(windows) { "llama-server.exe" } else { "llama-server" };
    let triple_name = format!(
        "llama-server-{}{}",
        current_triple(),
        if cfg!(windows) { ".exe" } else { "" }
    );
    let llama_server = first_existing(&[
        std::env::var("TPB_LLAMA_SERVER").ok(),
        resource.as_ref().map(|r| r.join(&triple_name).to_string_lossy().into_owned()),
        resource.as_ref().map(|r| r.join(server_name).to_string_lossy().into_owned()),
        Some(paths::llama_server()),
    ])
    .ok_or("the model server was not found. It ships with the installer.")?;

    let models = std::env::var("TPB_MODEL_DIR")
        .unwrap_or_else(|_| app_data.join("models").to_string_lossy().into_owned());
    let logs = app_data.join("logs").to_string_lossy().into_owned();

    Ok(AppPaths {
        user_db: app_data.join("user.db").to_string_lossy().into_owned(),
        app_data: app_data.to_string_lossy().into_owned(),
        models,
        index_db,
        llama_server,
        logs,
    })
}

fn current_triple() -> &'static str {
    if cfg!(all(windows, target_arch = "x86_64")) {
        "x86_64-pc-windows-msvc"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "x86_64-unknown-linux-gnu"
    } else {
        "unknown"
    }
}

fn first_existing(candidates: &[Option<String>]) -> Option<String> {
    candidates
        .iter()
        .flatten()
        .find(|p| std::path::Path::new(p).exists())
        .cloned()
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
    pub group_by: String,
}

fn read_settings(db: &UserDb) -> AppSettings {
    AppSettings {
        canon: db.get_setting("canon").unwrap_or_else(|| "66".into()),
        model: db.get_setting("model").unwrap_or_else(|| "standard".into()),
        compute: db.get_setting("compute").unwrap_or_else(|| "auto".into()),
        group_by: db.get_setting("group_by").unwrap_or_else(|| "topic".into()),
    }
}

fn engine_settings(paths: &AppPaths, s: &AppSettings) -> Result<Settings, String> {
    let spec = download::model(&s.model).ok_or_else(|| format!("unknown model {:?}", s.model))?;
    Ok(Settings {
        index_db: paths.index_db.clone(),
        llama_server: paths.llama_server.clone(),
        chat_model: std::path::Path::new(&paths.models).join(spec.file).to_string_lossy().into_owned(),
        embed_model: std::path::Path::new(&paths.models)
            .join(EMBED_GGUF)
            .to_string_lossy()
            .into_owned(),
        prompts_dir: paths::prompts_dir(),
        crisis_terms: paths::crisis_terms(),
        crisis_note: paths::crisis_note(),
        log_dir: Some(paths.logs.clone()),
        canon: CanonMode::parse(&s.canon)?,
        query_mode: QueryMode::Raw,
        chat_ctx: 8192,
        threads: None,
        // P5 ships the CPU path only. "auto" and "gpu" are recorded and shown;
        // P6 makes them mean something.
        gpu_layers: 0,
        allow_both_servers: true,
    })
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
    Ok(StartupState {
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
    const ALLOWED: &[&str] = &["canon", "model", "compute", "group_by", "first_run_done"];
    if !ALLOWED.contains(&key.as_str()) {
        return Err(format!("unknown setting {:?}", key));
    }
    {
        let db = state.db.lock().map_err(lock)?;
        db.set_setting(&key, &value)?;
    }
    // Canon and model change how the next question is answered, so the open
    // session is rebuilt rather than left holding the old ones.
    if key == "canon" || key == "model" {
        let db = state.db.lock().map_err(lock)?;
        let s = read_settings(&db);
        drop(db);
        let mut sess = state.session.lock().map_err(lock)?;
        if let Some(existing) = sess.as_mut() {
            existing.engine.settings = engine_settings(&state.paths, &s)?;
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
    let engine_settings = engine_settings(&paths, &settings)?;

    let out = tauri::async_runtime::spawn_blocking(move || {
        let result = (|| -> Result<Answer, String> {
            let mut guard = session.lock().map_err(|_| "the session is unavailable".to_string())?;
            if guard.is_none() {
                *guard = Some(Session::new(Engine::open(engine_settings)?));
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

#[tauri::command]
fn cancel_ask(state: State<'_, AppState>) -> Result<(), String> {
    let guard = state.session.lock().map_err(lock)?;
    if let Some(s) = guard.as_ref() {
        s.request_cancel();
    }
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
    let es = engine_settings(&paths, &settings)?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut guard = session.lock().map_err(|_| "the session is unavailable".to_string())?;
        if guard.is_none() {
            *guard = Some(Session::new(Engine::open(es)?));
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

#[tauri::command]
fn history_export(state: State<'_, AppState>, path: String) -> Result<String, String> {
    let db = state.db.lock().map_err(lock)?;
    let text = db.export_text()?;
    std::fs::write(&path, text.as_bytes())
        .map_err(|e| format!("cannot write {}: {}", path, e))?;
    Ok(path)
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
            app.manage(AppState {
                paths,
                session: Arc::new(Mutex::new(None)),
                db: Arc::new(Mutex::new(db)),
                busy: Arc::new(AtomicBool::new(false)),
                download_cancel: Arc::new(AtomicBool::new(false)),
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
            cancel_ask,
            run_self_test,
            finish_first_run,
            history_list,
            history_search,
            history_get,
            history_delete,
            history_clear,
            history_export,
            crisis_note,
            cited_tokens,
            shutdown_models,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
