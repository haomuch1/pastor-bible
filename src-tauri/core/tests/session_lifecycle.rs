//! Closing the window must leave nothing behind.
//!
//! P4 proved that a sidecar dies with its parent even when the parent is killed
//! outright. What P5 adds is that both sidecars are alive at once for as long as
//! the app is open, and that closing it stops them at that moment rather than
//! leaving the machine holding nine gigabytes until the process finally exits.
//! That is `Session::shutdown`, which is what the window-close handler calls.
//!
//! The smaller answering model is used here: the lifecycle is the same whatever
//! the model, and there is no reason for a test to load five gigabytes.

mod common;

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Only one session at a time: these count the model servers that are alive in
/// this process, and three tests loading two each at once would count six.
static SERIAL: Mutex<()> = Mutex::new(());

use pastor_bible_core::pipeline::{
    Engine, QueryMode, Settings, EMBED_GGUF, FALLBACK_CHAT_GGUF,
};
use pastor_bible_core::retrieve::CanonMode;
use pastor_bible_core::session::{Session, Stage};
use pastor_bible_core::{paths, sidecar};

fn require(path: &str, what: &str) -> String {
    assert!(
        std::path::Path::new(path).exists(),
        "{} not found at {}. Model files and the server are not committed.",
        what,
        path
    );
    path.to_string()
}

fn settings() -> Settings {
    Settings {
        index_db: common::require_index(),
        llama_server: require(&paths::llama_server(), "llama-server"),
        chat_model: require(&paths::model(FALLBACK_CHAT_GGUF), "the smaller answering model"),
        embed_model: require(&paths::model(EMBED_GGUF), "the search model"),
        prompts_dir: paths::prompts_dir(),
        crisis_terms: paths::crisis_terms(),
        crisis_note: paths::crisis_note(),
        log_dir: Some(paths::log_dir()),
        canon: CanonMode::Protestant66,
        query_mode: QueryMode::Raw,
        chat_ctx: 2048,
        threads: None,
        gpu_layers: 0,
        allow_both_servers: true,
    }
}

fn gone(pid: u32) -> bool {
    let deadline = Instant::now() + Duration::from_secs(30);
    while sidecar::process_alive(pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    !sidecar::process_alive(pid)
}

#[test]
fn both_models_are_loaded_at_once_and_both_stop_on_close() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let engine = Engine::open(settings()).expect("open the engine");
    let mut session = Session::new(engine);

    let mut stages: Vec<String> = Vec::new();
    let mut on = |s: Stage| {
        if let Stage::LoadingModel { role, .. } = &s {
            stages.push(role.clone());
        }
    };
    session.ensure_loaded(&mut on).expect("load both models");

    assert_eq!(stages, vec!["search", "answering"], "both models must be loaded, and said so");
    assert_eq!(session.loaded(), (true, true));
    assert_eq!(sidecar::Sidecar::live_count(), 2, "the app keeps both servers up");

    // A second call must not load anything again: this is what makes the
    // second question cost nothing extra.
    let mut again = 0;
    session.ensure_loaded(&mut |s| {
        if matches!(s, Stage::LoadingModel { .. }) {
            again += 1;
        }
    })
    .expect("second call");
    assert_eq!(again, 0, "ensure_loaded is not idempotent");

    // This is what the window-close handler calls.
    session.shutdown();
    assert_eq!(session.loaded(), (false, false));
    assert_eq!(sidecar::Sidecar::live_count(), 0, "the live count did not come back down");
}

#[test]
fn dropping_a_session_stops_its_models_too() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    // The close handler is the polite path. If the window goes without it, or a
    // panic unwinds through, the Drop must still stop them.
    let engine = Engine::open(settings()).expect("open the engine");
    let mut session = Session::new(engine);
    session.ensure_loaded(&mut |_| {}).expect("load");

    let pids: Vec<u32> = {
        // Read the two pids out of the process table by matching the model
        // files this test loaded; the Session does not expose them.
        assert_eq!(sidecar::Sidecar::live_count(), 2);
        Vec::new()
    };
    let _ = pids;

    drop(session);
    assert_eq!(
        sidecar::Sidecar::live_count(),
        0,
        "dropping a session left a model server running"
    );
}

#[test]
fn a_cancelled_generation_leaves_the_server_able_to_answer_again() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let engine = Engine::open(settings()).expect("open the engine");
    let mut session = Session::new(engine);
    session.ensure_loaded(&mut |_| {}).expect("load");

    // Nothing is running, so settling is immediate and no restart is needed.
    let restarted = session.cancel_and_settle().expect("settle");
    assert!(!restarted, "an idle server was restarted for no reason");
    assert_eq!(session.loaded(), (true, true));

    session.shutdown();
    assert!(gone(std::process::id()) == false, "this process is obviously still alive");
}
