//! The sidecar's lifecycle, proved rather than assumed.
//!
//! Three claims: it starts and answers, a second one is refused while the first
//! is alive, and it does not survive its parent. The third is the one that
//! matters, because an orphaned llama-server holding five gigabytes is the
//! worst thing this program could leave on someone's machine, and it is the one
//! that cannot be tested inside a single process: the test kills the parent
//! hard, with no unwinding and no destructors, which is exactly the case a
//! Rust-level guard cannot cover.
//!
//! The embedding model is used throughout: it is 262 MB and loads in under a
//! second, and the lifecycle is the same whatever the model.

mod common;

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use pastor_bible_core::paths;
use pastor_bible_core::pipeline::EMBED_GGUF;
use pastor_bible_core::sidecar::{free_ram_gb, process_alive, Options, Role, Sidecar};

/// Only one sidecar may be alive at a time, which is the point; the tests in
/// this file must therefore not run beside each other.
static SERIAL: Mutex<()> = Mutex::new(());

fn require(path: &str, what: &str) -> String {
    assert!(
        std::path::Path::new(path).exists(),
        "{} not found at {}. It is not committed; fetch it before running the \
         sidecar tests, or point TPB_LLAMA_SERVER / TPB_MODEL_DIR elsewhere.",
        what,
        path
    );
    path.to_string()
}

fn embed_options() -> Options {
    let server = require(&paths::llama_server(), "llama-server");
    let model = require(&paths::model(EMBED_GGUF), "the embedding model");
    let mut o = Options::new(&server, &model, Role::Embedding);
    o.log_dir = Some(paths::log_dir());
    o.ready_timeout = Duration::from_secs(180);
    o
}

#[test]
fn spawn_health_embed_stop_and_leave_nothing_behind() {
    let _guard = SERIAL.lock().unwrap();
    let opts = embed_options();

    let before = free_ram_gb();
    assert!(before > 0.0, "could not read free RAM, so the load check is not being made");

    let s = Sidecar::start(&opts).expect("sidecar start");
    let pid = s.pid().expect("a pid");
    assert!(s.port >= 1024, "the sidecar must be on a real port");
    assert!(process_alive(pid), "the sidecar is not running after start returned");

    // start() only returns once /health has answered 200, so reaching here is
    // the health check.
    assert!(s.ready_seconds > 0.0);

    let vecs = s.embed(&["search_query: anxiety and worry".to_string()]).expect("embed");
    assert_eq!(vecs.len(), 1);
    assert_eq!(vecs[0].len(), 768, "nomic-embed-text-v1.5 is 768-dimensional");
    let norm: f64 = vecs[0].iter().map(|x| (*x as f64) * (*x as f64)).sum::<f64>().sqrt();
    assert!((norm - 1.0).abs() < 1e-5, "the query vector must be unit length, got {}", norm);

    s.stop();

    let deadline = Instant::now() + Duration::from_secs(30);
    while process_alive(pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(!process_alive(pid), "the sidecar outlived stop(), pid {}", pid);
}

#[test]
fn a_second_sidecar_is_refused_while_one_is_running() {
    let _guard = SERIAL.lock().unwrap();
    let opts = embed_options();
    let s = Sidecar::start(&opts).expect("first sidecar");
    let second = Sidecar::start(&opts);
    assert!(
        second.is_err(),
        "two model processes were allowed at once, which is what the sequential \
         rule exists to prevent"
    );
    let pid = s.pid().unwrap();
    s.stop();
    // And the manager must be usable again afterwards, not wedged.
    let third = Sidecar::start(&opts).expect("a sidecar after the first stopped");
    let third_pid = third.pid().unwrap();
    third.stop();
    assert_ne!(pid, third_pid, "the second start returned the first process");
}

#[test]
fn two_sidecars_are_allowed_only_when_one_asks_to_share() {
    let _guard = SERIAL.lock().unwrap();
    let mut opts = embed_options();
    let first = Sidecar::start(&opts).expect("first sidecar");
    assert_eq!(Sidecar::live_count(), 1);

    // The refusal is the default, and the flag is what lifts it. Both models
    // here are 262 MB, so the RAM check clears easily; the point is that the
    // guard, not the caller's memory, decides.
    assert!(Sidecar::start(&opts).is_err(), "a second sidecar started without asking");
    opts.allow_concurrent = true;
    let second = Sidecar::start(&opts).expect("second sidecar with the flag set");
    assert_eq!(Sidecar::live_count(), 2);
    assert_ne!(first.pid(), second.pid());

    let (a, b) = (first.pid().unwrap(), second.pid().unwrap());
    first.stop();
    second.stop();
    assert_eq!(Sidecar::live_count(), 0, "the live count did not come back down");
    let deadline = Instant::now() + Duration::from_secs(30);
    while (process_alive(a) || process_alive(b)) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(!process_alive(a) && !process_alive(b), "a shared sidecar outlived stop()");
}

#[test]
fn the_sidecar_does_not_survive_a_hard_kill_of_its_parent() {
    let _guard = SERIAL.lock().unwrap();
    require(&paths::llama_server(), "llama-server");
    require(&paths::model(EMBED_GGUF), "the embedding model");

    let mut parent = Command::new(env!("CARGO_BIN_EXE_pastor-bible-cli"))
        .arg("spawn-and-hang")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("start the parent process");

    let stdout = parent.stdout.take().expect("parent stdout");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    let mut sidecar_pid: Option<u32> = None;
    let deadline = Instant::now() + Duration::from_secs(180);
    while Instant::now() < deadline {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                if let Some(rest) = line.trim().strip_prefix("SIDECAR_PID ") {
                    sidecar_pid = rest.parse().ok();
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let sidecar_pid = match sidecar_pid {
        Some(p) if p != 0 => p,
        _ => {
            let _ = parent.kill();
            let _ = parent.wait();
            panic!("the parent never reported a sidecar pid");
        }
    };
    assert!(process_alive(sidecar_pid), "the sidecar was not running before the kill");

    // TerminateProcess on Windows, SIGKILL on Unix: no unwinding, no Drop, no
    // handler of any kind runs in the parent.
    parent.kill().expect("kill the parent");
    parent.wait().expect("reap the parent");

    let deadline = Instant::now() + Duration::from_secs(30);
    while process_alive(sidecar_pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(
        !process_alive(sidecar_pid),
        "the sidecar (pid {}) outlived a hard kill of its parent. An orphaned \
         model server is the worst thing this program can leave behind.",
        sidecar_pid
    );
}
