//! pastor-bible-cli: the whole pipeline from a terminal.
//!
//! This is the P4 deliverable's harness. It runs one question at a time through
//! the same code the app will call, prints the output structure as JSON and a
//! readable rendering beside it, and exits non-zero if anything failed.
//!
//!   pastor-bible-cli ask "What does the Bible say about anxiety?"
//!   pastor-bible-cli ask "..." --canon both --model fallback --json out.json
//!   pastor-bible-cli selftest                 sidecar lifecycle, no model call
//!   pastor-bible-cli spawn-and-hang           used by the orphan test
//!   pastor-bible-cli session --questions 5    one open session, several
//!                                             questions, the way the app runs
//!   pastor-bible-cli session --cancel-after 20  cancel mid-generation

use std::process::ExitCode;

use pastor_bible_core::api::Answer;
use pastor_bible_core::paths;
use pastor_bible_core::pipeline::{
    Engine, QueryMode, Settings, DEFAULT_CHAT_GGUF, EMBED_GGUF, FALLBACK_CHAT_GGUF,
};
use pastor_bible_core::retrieve::CanonMode;
use pastor_bible_core::sidecar::{free_ram_gb, Options, Role, Sidecar};

fn usage() -> &'static str {
    "usage:\n  \
     pastor-bible-cli ask \"<question>\" [--canon 66|both] [--model default|fallback|<file>]\n                   \
     [--query raw|rewrite|fused] [--ctx N] [--threads N] [--json <path>] [--quiet]\n  \
     pastor-bible-cli selftest\n  \
     pastor-bible-cli session [--questions N] [--cancel-after SECONDS] [--model ...]\n  \
     pastor-bible-cli spawn-and-hang [--model <file>]\n"
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprint!("{}", usage());
        return ExitCode::from(2);
    }
    let result = match args[0].as_str() {
        "ask" => cmd_ask(&args[1..]),
        "selftest" => cmd_selftest(),
        "session" => cmd_session(&args[1..]),
        "spawn-and-hang" => cmd_spawn_and_hang(&args[1..]),
        other => Err(format!("unknown command {:?}\n{}", other, usage())),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {}", e);
            ExitCode::from(1)
        }
    }
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(|s| s.as_str())
}

fn cmd_ask(args: &[String]) -> Result<(), String> {
    let question = args.first().filter(|a| !a.starts_with("--")).ok_or_else(|| {
        format!("ask needs a question in quotes\n{}", usage())
    })?;

    let canon = CanonMode::parse(flag(args, "--canon").unwrap_or("66"))?;
    let query_mode = QueryMode::parse(flag(args, "--query").unwrap_or("raw"))?;
    let chat_model = match flag(args, "--model").unwrap_or("default") {
        "default" => paths::model(DEFAULT_CHAT_GGUF),
        "fallback" => paths::model(FALLBACK_CHAT_GGUF),
        other => {
            if std::path::Path::new(other).exists() {
                other.to_string()
            } else {
                paths::model(other)
            }
        }
    };
    let chat_ctx: u32 = flag(args, "--ctx").unwrap_or("8192").parse().map_err(|_| "--ctx wants a number")?;
    let threads: Option<u32> = match flag(args, "--threads") {
        Some(t) => Some(t.parse().map_err(|_| "--threads wants a number")?),
        None => None,
    };
    let quiet = args.iter().any(|a| a == "--quiet");

    let settings = Settings {
        index_db: paths::index_db(),
        llama_server: paths::llama_server(),
        chat_model,
        embed_model: paths::model(EMBED_GGUF),
        prompts_dir: paths::prompts_dir(),
        crisis_terms: paths::crisis_terms(),
        crisis_note: paths::crisis_note(),
        log_dir: Some(paths::log_dir()),
        canon,
        query_mode,
        chat_ctx,
        threads,
        gpu_layers: flag(args, "--gpu-layers")
            .unwrap_or("0")
            .parse()
            .map_err(|_| "--gpu-layers wants a number")?,
        allow_both_servers: args.iter().any(|a| a == "--allow-both-servers"),
    };

    for missing in [&settings.index_db, &settings.llama_server, &settings.chat_model] {
        if !std::path::Path::new(missing).exists() {
            return Err(format!("not found: {}", missing));
        }
    }
    if !quiet {
        eprintln!("free RAM before any model load: {:.2} GB", free_ram_gb());
    }

    let engine = Engine::open(settings)?;
    for d in engine.prompts.drift() {
        eprintln!("warning: prompt {}", d);
    }
    let answer = engine.ask(question)?;

    let json = serde_json::to_string_pretty(&answer).map_err(|e| e.to_string())?;
    if let Some(path) = flag(args, "--json") {
        std::fs::write(path, &json).map_err(|e| format!("cannot write {}: {}", path, e))?;
        eprintln!("wrote {}", path);
    } else {
        println!("{}", json);
    }
    if !quiet {
        eprintln!("\n{}", render(&answer));
    }
    Ok(())
}

/// A readable rendering of the same answer, for a person at a terminal.
fn render(a: &Answer) -> String {
    let mut out = String::new();
    out.push_str(&format!("QUESTION  {}\n", a.question));
    out.push_str(&format!(
        "canon {}  query {}  model {}  index {}  verdict {}\n",
        a.canon_mode, a.query_mode, a.model_id, a.index_version, a.verdict
    ));
    if let Some(note) = &a.crisis_note {
        out.push_str("\n--- CRISIS NOTE (shown above the answer, never instead of it) ---\n");
        out.push_str(note);
        out.push('\n');
    }
    out.push_str("\n--- ANSWER ---\n");
    match (&a.synopsis_markdown, &a.fallback_markdown) {
        (Some(s), _) => out.push_str(s),
        (None, Some(f)) => out.push_str(f),
        _ => out.push_str("(nothing)"),
    }
    out.push('\n');
    if let Some(f) = &a.deuterocanon_footer {
        out.push_str(&format!("\n{}\n", f));
    }
    out.push_str(&format!(
        "\n--- PASSAGES: {} retrieved, {} sent, {} cited ---\n",
        a.passages.len(),
        a.sent_count,
        a.cited_tokens.len()
    ));
    for p in a.passages.iter().filter(|p| p.sent) {
        out.push_str(&format!(
            "{:>6} {:<14} {}{}  [{}]\n",
            p.token.as_deref().unwrap_or(""),
            p.reference,
            if p.cited { "cited " } else { "      " },
            if p.canon == "deutero" { "(Deuterocanon)" } else { "" },
            p.origins.join(",")
        ));
    }
    out.push_str("\n--- TOPIC GROUPING OF THE FULL SET ---\n");
    for g in &a.topic_groups {
        out.push_str(&format!("{} ({} passages)\n", g.heading, g.passage_refs.len()));
    }
    let t = &a.timings;
    out.push_str(&format!(
        "\n--- TIMINGS (s) ---\nindex {:.2}  embed-server {:.2}  embed {:.2}  chat-server {:.2}  \
         retrieve {:.3}  generate {:.1}  retry {:.1}  verify {:.3}  total {:.1}\n",
        t.index_load_seconds,
        t.embed_server_seconds,
        t.embed_seconds,
        t.chat_server_seconds,
        t.retrieve_seconds,
        t.generate_seconds,
        t.retry_seconds,
        t.verify_seconds,
        t.total_seconds
    ));
    if let Some(mb) = a.peak_ram_mb {
        out.push_str(&format!("peak sidecar RAM {:.0} MB  sidecar path {}\n", mb, a.sidecar_path));
    }
    out
}

/// Several questions through one open session, which is how the app runs.
///
/// P4 measured a fresh server per question at 9,001 MB. The app keeps both
/// servers loaded for as long as the window is open, so this is the figure that
/// actually applies to a reader, and it is the one P3 saw climb to 15 GB over
/// ten questions with a long-lived server.
fn cmd_session(args: &[String]) -> Result<(), String> {
    use pastor_bible_core::session::{Session, Stage};

    let n: usize = flag(args, "--questions").unwrap_or("5").parse().map_err(|_| "--questions wants a number")?;
    let cancel_after: Option<f64> = match flag(args, "--cancel-after") {
        Some(v) => Some(v.parse().map_err(|_| "--cancel-after wants a number of seconds")?),
        None => None,
    };
    let chat_model = match flag(args, "--model").unwrap_or("default") {
        "default" => paths::model(DEFAULT_CHAT_GGUF),
        "fallback" => paths::model(FALLBACK_CHAT_GGUF),
        other => paths::model(other),
    };
    let settings = Settings {
        index_db: paths::index_db(),
        llama_server: paths::llama_server(),
        chat_model,
        embed_model: paths::model(EMBED_GGUF),
        prompts_dir: paths::prompts_dir(),
        crisis_terms: paths::crisis_terms(),
        crisis_note: paths::crisis_note(),
        log_dir: Some(paths::log_dir()),
        canon: CanonMode::parse(flag(args, "--canon").unwrap_or("66"))?,
        query_mode: QueryMode::Raw,
        chat_ctx: 8192,
        threads: None,
        gpu_layers: 0,
        allow_both_servers: true,
    };

    let questions = session_questions(n)?;
    println!("free RAM before any model load: {:.2} GB", free_ram_gb());
    let engine = Engine::open(settings)?;
    let mut session = Session::new(engine);

    let mut rows = Vec::new();
    for (i, (id, q)) in questions.iter().enumerate() {
        let t0 = std::time::Instant::now();
        let mut last = String::new();
        let flag_cancel = session.cancel_flag();
        let pid_slot = session.chat_pid_slot();
        let deadline = cancel_after;
        // A cancel is asked for from another thread, exactly as the window does
        // it: Stop sets the flag, and two seconds later, if the answer is still
        // running, the answering model is stopped outright. Without that second
        // step a cancellation during prompt processing waits for the first
        // token, which was measured at 16.3 seconds.
        let watcher = deadline.map(|secs| {
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs_f64(secs));
                flag_cancel.store(true, std::sync::atomic::Ordering::SeqCst);
                let pressed = std::time::Instant::now();
                std::thread::sleep(std::time::Duration::from_secs(2));
                if let Some(pid) = pid_slot.lock().ok().and_then(|p| *p) {
                    pastor_bible_core::sidecar::terminate(pid);
                }
                pressed
            })
        });

        let mut on = |s: Stage| {
            let text = match &s {
                Stage::LoadingModel { role, .. } => format!("loading the {} model", role),
                Stage::Retrieving => "searching".to_string(),
                Stage::Retrieved { passages, sent, .. } => {
                    format!("found {} passages, sending {}", passages, sent)
                }
                Stage::Generating { tokens, attempt } => {
                    format!("writing (attempt {}) {} tokens", attempt, tokens)
                }
                Stage::CheckingReferences { .. } => "checking references".to_string(),
                Stage::Retrying { .. } => "writing again".to_string(),
                Stage::Done { verdict } => format!("done: {}", verdict),
                Stage::Cancelled => "cancelled".to_string(),
                Stage::Failed { message } => format!("failed: {}", message),
            };
            if text != last {
                eprintln!("    [{:6.1}s] {}", t0.elapsed().as_secs_f64(), text);
                last = text;
            }
        };

        println!("\n{}. {}  {}", i + 1, id, q);
        let outcome = session.ask(q, &mut on);
        let seconds = t0.elapsed().as_secs_f64();
        let asked_for_cancel_at = watcher.map(|h| h.join().ok());

        let (embed_peak, chat_peak) = session.sidecar_peak_mb();
        match outcome {
            Ok(a) => {
                println!(
                    "   verdict {}  cited {}  sent {}  {:.1}s  embed peak {:.0} MB  chat peak {:.0} MB",
                    a.verdict,
                    a.cited_tokens.len(),
                    a.sent_count,
                    seconds,
                    embed_peak.unwrap_or(0.0),
                    chat_peak.unwrap_or(0.0)
                );
                rows.push((id.clone(), "ok".to_string(), seconds, embed_peak, chat_peak));
            }
            Err(e) if e == pastor_bible_core::sidecar::CANCELLED => {
                let asked = asked_for_cancel_at.flatten();
                let stop_to_return = asked.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0);
                // Was the sidecar restarted, or did closing the connection
                // stop it? A second cancel on an idle server answers that.
                let t = std::time::Instant::now();
                let restarted = session.cancel_and_settle().unwrap_or(true);
                println!(
                    "   cancelled after {:.1}s; the call returned {:.2}s after Stop was pressed; \
                     the server was {} ({:.2}s to settle)",
                    seconds,
                    stop_to_return,
                    if restarted { "RESTARTED" } else { "still usable" },
                    t.elapsed().as_secs_f64()
                );
                rows.push((id.clone(), format!("cancelled/{:.2}", stop_to_return), seconds, embed_peak, chat_peak));
            }
            Err(e) => return Err(e),
        }
    }

    let (embed_peak, chat_peak) = session.sidecar_peak_mb();
    let own = pastor_bible_core::pipeline::own_peak_ram_mb();
    println!("\n--- one session, {} questions ---", rows.len());
    for (id, verdict, seconds, _, _) in &rows {
        println!("  {:<5} {:<18} {:.1}s", id, verdict, seconds);
    }
    println!(
        "  peak resident: search model {:.0} MB, answering model {:.0} MB, both together {:.0} MB",
        embed_peak.unwrap_or(0.0),
        chat_peak.unwrap_or(0.0),
        embed_peak.unwrap_or(0.0) + chat_peak.unwrap_or(0.0)
    );
    println!("  peak resident of this process: {:.0} MB", own.unwrap_or(0.0));
    session.shutdown();
    Ok(())
}

fn session_questions(n: usize) -> Result<Vec<(String, String)>, String> {
    let path = std::path::Path::new(&paths::data_dir()).join("eval").join("questions.json");
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let graded = v["graded"].as_array().ok_or("no graded questions")?;
    let ids = v["p3_graded"].as_array().ok_or("no p3_graded list")?;
    let mut out = Vec::new();
    for id in ids.iter().take(n) {
        let id = id.as_str().unwrap_or("");
        if let Some(q) = graded.iter().find(|g| g["id"] == id) {
            out.push((id.to_string(), q["question"].as_str().unwrap_or("").to_string()));
        }
    }
    Ok(out)
}

/// Spawn the embedding sidecar, check health, embed one string, stop. Proves
/// the lifecycle without loading a chat model.
fn cmd_selftest() -> Result<(), String> {
    let server_bin = paths::llama_server();
    let model = paths::model(EMBED_GGUF);
    for p in [&server_bin, &model] {
        if !std::path::Path::new(p).exists() {
            return Err(format!("not found: {}", p));
        }
    }
    println!("free RAM {:.2} GB", free_ram_gb());
    let mut opts = Options::new(&server_bin, &model, Role::Embedding);
    opts.log_dir = Some(paths::log_dir());
    let s = Sidecar::start(&opts)?;
    println!("sidecar up on 127.0.0.1:{} in {:.1}s, pid {:?}", s.port, s.ready_seconds, s.pid());
    let v = s.embed(&["search_query: anxiety".to_string()])?;
    println!("embedded 1 string, dim {}, first value {:.6}", v[0].len(), v[0][0]);
    let second = Sidecar::start(&opts);
    println!(
        "a second spawn while one is running is {}",
        if second.is_err() { "refused, as it must be" } else { "ALLOWED, which is a bug" }
    );
    if second.is_ok() {
        return Err("the sidecar manager allowed two servers at once".to_string());
    }
    let pid = s.pid();
    s.stop();
    println!("sidecar stopped; pid {:?} should no longer exist", pid);
    Ok(())
}

/// Start a sidecar, print its pid, and wait to be killed. The orphan test kills
/// this process hard and then checks the sidecar died with it.
fn cmd_spawn_and_hang(args: &[String]) -> Result<(), String> {
    let model = flag(args, "--model")
        .map(|m| m.to_string())
        .unwrap_or_else(|| paths::model(EMBED_GGUF));
    let mut opts = Options::new(&paths::llama_server(), &model, Role::Embedding);
    opts.log_dir = Some(paths::log_dir());
    let s = Sidecar::start(&opts)?;
    println!("SIDECAR_PID {}", s.pid().unwrap_or(0));
    use std::io::Write;
    std::io::stdout().flush().ok();
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}
