//! One open app: two loaded sidecars, and the questions asked through them.
//!
//! P4's `Engine::ask` started a server, answered, and stopped it. That is right
//! for a harness measuring one question and wrong for an app, where the second
//! question would pay for a five-gigabyte load again. A session keeps both
//! sidecars up for as long as the window is open and stops both when it closes
//! (PLAN 7.4, as amended 2026-08-26).
//!
//! Everything the reader waits for is reported as it happens, and can be
//! stopped. What is never reported is the text being generated: PLAN 5.6 says
//! no unverified reference reaches a reader, and a token stream is exactly
//! that. The progress that leaves this module is a stage, a count and a clock.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::api::{Answer, AttemptOut, PassageOut, Timings, TopicGroup, ViolationOut};
use crate::pipeline::{with_no_think, Engine, DEUTERO_SLICE, MAX_TOKENS, SEED, TOP_N};
use crate::prompts::fill;
use crate::retrieve::Config;
use crate::sidecar::{Options, Role, Sidecar, CANCELLED};
use crate::verifier::{Verdict, Verifier};

/// What the reader is waiting for, right now.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case")]
pub enum Stage {
    /// A model is being loaded. Only on the first question of a session.
    LoadingModel { role: String, model: String },
    Retrieving,
    /// Retrieval is finished and the passages are ready to be collected.
    ///
    /// This is the whole point of the amended PLAN 7.2: retrieval takes about
    /// forty milliseconds and generation takes two and a half minutes, so the
    /// reader should be reading scripture for the whole of that wait rather
    /// than watching a spinner. The passages themselves are not in this event.
    /// They are a quarter of a megabyte, and a payload that size does not
    /// survive the event channel, silently: the counts arrive and the list does
    /// not. The caller fetches them with `take_retrieved` instead, over the
    /// same channel that already carries the finished answer.
    Retrieved { passages: usize, sent: usize },
    Generating { tokens: u64, attempt: u32 },
    CheckingReferences { attempt: u32 },
    /// The first answer broke the citation rule and is being written again.
    Retrying { reason: String },
    Done { verdict: String },
    Cancelled,
    Failed { message: String },
}

pub struct Session {
    pub engine: Engine,
    embed: Option<Sidecar>,
    chat: Option<Sidecar>,
    cancel: Arc<AtomicBool>,
    /// The chat model currently loaded, so a change in Settings reloads it.
    loaded_chat_model: Option<String>,
    /// The passages of the question being answered, ready for the window to
    /// collect as soon as it hears that retrieval is done.
    ///
    /// Behind its own lock rather than inside the session, because `ask` holds
    /// the session for the two and a half minutes an answer takes and the
    /// whole point is to hand these over in the first second of that.
    retrieved: RetrievedSlot,
    chat_pid: ChatPidSlot,
}

/// Where the retrieved passages wait for the window to collect them.
pub type RetrievedSlot = Arc<Mutex<Option<(Vec<PassageOut>, Vec<TopicGroup>)>>>;

/// The answering model's process id, published so that a cancellation can stop
/// it without waiting for the thread that is reading from it.
pub type ChatPidSlot = Arc<Mutex<Option<u32>>>;

impl Session {
    pub fn new(engine: Engine) -> Self {
        Session {
            engine,
            embed: None,
            chat: None,
            cancel: Arc::new(AtomicBool::new(false)),
            loaded_chat_model: None,
            retrieved: Arc::new(Mutex::new(None)),
            chat_pid: Arc::new(Mutex::new(None)),
        }
    }

    /// A handle on the answering model's process id.
    ///
    /// Cancelling closes the connection, and llama-server abandons the slot
    /// when its client goes away. But the thread doing the reading is blocked
    /// inside that read, and during prompt processing no data arrives for
    /// tens of seconds, so the flag is not looked at and the reader waits.
    /// Measured on 2026-08-26: 16.3 seconds from Stop to the call returning.
    /// A caller that wants the two-second bound the plan asks for uses this to
    /// stop the process outright; the session notices and starts it again.
    pub fn chat_pid_slot(&self) -> ChatPidSlot {
        self.chat_pid.clone()
    }

    /// A session that publishes its retrieved passages into a slot the caller
    /// already holds, so the window can collect them while `ask` is running.
    pub fn with_slot(engine: Engine, retrieved: RetrievedSlot, chat_pid: ChatPidSlot) -> Self {
        let mut s = Session::new(engine);
        s.retrieved = retrieved;
        s.chat_pid = chat_pid;
        s
    }

    /// A handle on the slot the retrieved passages are put in.
    pub fn retrieved_slot(&self) -> RetrievedSlot {
        self.retrieved.clone()
    }

    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.cancel.clone()
    }

    /// Ask the current generation to stop. Safe to call when nothing is running.
    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    fn embed_options(&self) -> Options {
        let s = &self.engine.settings;
        let mut o = Options::new(&s.llama_server, &s.embed_model, Role::Embedding);
        o.log_dir = s.log_dir.clone();
        o.threads = s.threads;
        // Both servers live at once for the whole session, so each has to be
        // allowed to start beside the other. The free-RAM check still runs and
        // still sees the first one resident.
        o.allow_concurrent = true;
        o
    }

    fn chat_options(&self) -> Options {
        let s = &self.engine.settings;
        let mut o = Options::new(&s.llama_server, &s.chat_model, Role::Chat);
        o.n_ctx = s.chat_ctx;
        o.threads = s.threads;
        o.gpu_layers = s.gpu_layers;
        o.log_dir = s.log_dir.clone();
        o.allow_concurrent = true;
        o
    }

    /// Both models loaded and answering. Idempotent.
    pub fn ensure_loaded(&mut self, on: &mut impl FnMut(Stage)) -> Result<(), String> {
        if self.embed.is_none() {
            on(Stage::LoadingModel {
                role: "search".to_string(),
                model: file_name(&self.engine.settings.embed_model),
            });
            self.embed = Some(Sidecar::start(&self.embed_options())?);
        }
        let want = self.engine.settings.chat_model.clone();
        if self.chat.is_some() && self.loaded_chat_model.as_deref() != Some(want.as_str()) {
            // The reader changed the model in Settings.
            if let Some(c) = self.chat.take() {
                c.stop();
            }
            self.loaded_chat_model = None;
        }
        if self.chat.is_none() {
            on(Stage::LoadingModel {
                role: "answering".to_string(),
                model: file_name(&want),
            });
            let s = Sidecar::start(&self.chat_options())?;
            if let Ok(mut slot) = self.chat_pid.lock() {
                *slot = s.pid();
            }
            self.chat = Some(s);
            self.loaded_chat_model = Some(want);
        }
        Ok(())
    }

    pub fn loaded(&self) -> (bool, bool) {
        (self.embed.is_some(), self.chat.is_some())
    }

    /// Peak resident memory of both sidecars, in MB.
    pub fn sidecar_peak_mb(&self) -> (Option<f64>, Option<f64>) {
        (
            self.embed.as_ref().and_then(|s| s.peak_ram_mb()),
            self.chat.as_ref().and_then(|s| s.peak_ram_mb()),
        )
    }

    /// Stop reading, and make sure the server is free for the next question.
    ///
    /// Dropping the response reader is what stops the generation. If the slot
    /// is still busy two seconds later that did not work, and the sidecar is
    /// restarted rather than left to make the next question queue behind an
    /// answer nobody is waiting for.
    fn settle_after_cancel(&mut self) -> Result<bool, String> {
        // If the process is already gone, there is nothing to wait for.
        let dead = self
            .chat
            .as_ref()
            .and_then(|c| c.pid())
            .map(|p| !crate::sidecar::process_alive(p))
            .unwrap_or(false);
        if dead {
            // Put it down and hand control back now. The next question calls
            // ensure_loaded and pays the four seconds then; a reader who
            // pressed Stop should not wait for a model they may not want.
            if let Some(c) = self.chat.take() {
                c.stop();
            }
            self.loaded_chat_model = None;
            if let Ok(mut slot) = self.chat_pid.lock() {
                *slot = None;
            }
            return Ok(true);
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if self.chat.as_ref().map(|c| c.is_idle()).unwrap_or(true) {
                return Ok(false);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        if let Some(c) = self.chat.take() {
            c.stop();
        }
        self.loaded_chat_model = None;
        if let Ok(mut slot) = self.chat_pid.lock() {
            *slot = None;
        }
        let mut ignored = |_: Stage| {};
        self.ensure_loaded(&mut ignored)?;
        Ok(true)
    }

    /// One question, end to end, reporting each stage as it starts.
    pub fn ask(&mut self, question: &str, on: &mut impl FnMut(Stage)) -> Result<Answer, String> {
        self.cancel.store(false, Ordering::SeqCst);
        if let Ok(mut slot) = self.retrieved.lock() {
            *slot = None;
        }
        let t_start = Instant::now();
        let mut timings =
            Timings { index_load_seconds: self.engine.index_load_seconds, ..Default::default() };

        let t = Instant::now();
        self.ensure_loaded(on)?;
        timings.chat_server_seconds = t.elapsed().as_secs_f64();

        let crisis = self.engine.crisis.is_crisis(question);

        // ---- retrieve ----------------------------------------------------
        on(Stage::Retrieving);
        let t = Instant::now();
        // The app asks in raw mode, which P4 chose by measurement. The rewrite
        // modes need a second generation before retrieval can even start, and
        // are reachable from the CLI rather than from the window.
        let keywords = crate::pipeline::question_terms(question);
        let prefixed = format!("{}{}", self.engine.retriever.query_prefix, question);
        let embed = self.embed.as_ref().ok_or("the search model is not loaded")?;
        let vecs = embed.embed(&[prefixed])?;
        timings.embed_seconds = t.elapsed().as_secs_f64();
        let qvec = vecs.into_iter().next().ok_or("the search model returned nothing")?;

        let t = Instant::now();
        let canon = self.engine.settings.canon;
        let (full, _top, topics) = self.engine.retriever.search(
            &qvec,
            &keywords,
            canon,
            Config::f(),
            TOP_N,
            100,
            DEUTERO_SLICE,
        );
        let ranges = self.engine.retriever.as_ranges(&full);
        let cut = self.engine.retriever.top_cut(&ranges, canon, TOP_N, DEUTERO_SLICE);
        timings.retrieve_seconds = t.elapsed().as_secs_f64();
        if let Ok(mut slot) = self.retrieved.lock() {
            *slot = Some(self.engine.passages_for_display(&ranges, &cut, &topics));
        }
        on(Stage::Retrieved { passages: ranges.len(), sent: cut.len() });

        if self.is_cancelled() {
            on(Stage::Cancelled);
            return Err(CANCELLED.to_string());
        }

        let (packed, sent) = self.engine.pack(&cut);

        // ---- generate, verify, retry once, fall back ----------------------
        let mut attempts: Vec<AttemptOut> = Vec::new();
        let prompt = fill(
            self.engine.prompts.body("synopsis"),
            &[("question", question), ("passages", &packed)],
        );

        on(Stage::Generating { tokens: 0, attempt: 1 });
        let gen = self.generate(&prompt, 1, on)?;
        timings.generate_seconds = gen.seconds;

        on(Stage::CheckingReferences { attempt: 1 });
        let tv = Instant::now();
        let (verdict, violations) =
            self.engine.verifier.check(&self.engine.retriever.index, &gen.text, &sent);
        timings.verify_seconds += tv.elapsed().as_secs_f64();
        attempts.push(attempt_out(&verdict, &gen, &violations));

        let mut final_text = gen.text.clone();
        let mut fallback_used = false;
        if verdict == Verdict::Violation {
            let note = Verifier::failure_note(&violations);
            on(Stage::Retrying { reason: reader_reason(&violations) });
            let p2 = fill(
                self.engine.prompts.body("retry"),
                &[("failure", &note), ("question", question), ("passages", &packed)],
            );
            on(Stage::Generating { tokens: 0, attempt: 2 });
            let gen2 = self.generate(&p2, 2, on)?;
            timings.retry_seconds = gen2.seconds;
            on(Stage::CheckingReferences { attempt: 2 });
            let tv = Instant::now();
            let (v2, viol2) =
                self.engine.verifier.check(&self.engine.retriever.index, &gen2.text, &sent);
            timings.verify_seconds += tv.elapsed().as_secs_f64();
            attempts.push(attempt_out(&v2, &gen2, &viol2));
            if v2 == Verdict::Ok {
                final_text = gen2.text;
            } else {
                fallback_used = true;
            }
        }

        let peak = self.sidecar_peak_mb();
        let answer = self.engine.assemble(
            question,
            crisis,
            &ranges,
            &cut,
            &sent,
            &topics,
            if fallback_used { None } else { Some(final_text) },
            attempts,
            fallback_used,
            timings,
            max_opt(peak.0, peak.1),
            true,
            t_start,
        );
        on(Stage::Done { verdict: answer.verdict.clone() });
        Ok(answer)
    }

    fn generate(
        &mut self,
        prompt: &str,
        attempt: u32,
        on: &mut impl FnMut(Stage),
    ) -> Result<crate::sidecar::Completion, String> {
        let cancel = self.cancel.clone();
        let out = {
            let chat = self.chat.as_ref().ok_or("the answering model is not loaded")?;
            chat.complete_streaming(&with_no_think(prompt), MAX_TOKENS, SEED, &cancel, |n| {
                if n % 8 == 0 {
                    on(Stage::Generating { tokens: n, attempt });
                }
            })
        };
        match out {
            Ok(c) => Ok(c),
            Err(e) if e == CANCELLED || self.is_cancelled() => {
                // Either the flag was seen between chunks, or the server was
                // stopped under the reader to make Stop mean Stop. Both are a
                // cancellation, and both leave a server to bring back.
                let restarted = self.settle_after_cancel()?;
                let _ = restarted;
                on(Stage::Cancelled);
                Err(CANCELLED.to_string())
            }
            Err(e) => {
                on(Stage::Failed { message: e.clone() });
                Err(e)
            }
        }
    }

    /// Cancel, and report whether the sidecar had to be restarted. Used by the
    /// measurement harness; the app calls `request_cancel` and lets `ask`
    /// return.
    pub fn cancel_and_settle(&mut self) -> Result<bool, String> {
        self.request_cancel();
        self.settle_after_cancel()
    }

    pub fn shutdown(&mut self) {
        self.request_cancel();
        if let Some(c) = self.chat.take() {
            c.stop();
        }
        if let Some(e) = self.embed.take() {
            e.stop();
        }
        self.loaded_chat_model = None;
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn attempt_out(
    verdict: &Verdict,
    gen: &crate::sidecar::Completion,
    violations: &[crate::verifier::Violation],
) -> AttemptOut {
    AttemptOut {
        verdict: verdict.as_str().to_string(),
        seconds: gen.seconds,
        prompt_tokens: gen.prompt_tokens,
        completion_tokens: gen.completion_tokens,
        violations: violations
            .iter()
            .map(|v| ViolationOut {
                kind: v.kind.as_str().to_string(),
                text: v.text.clone(),
                reason: v.reason.clone(),
                span: v.char_span,
            })
            .collect(),
    }
}

/// The retry, said in words a reader can act on rather than in the verifier's.
fn reader_reason(violations: &[crate::verifier::Violation]) -> String {
    let refs: Vec<&str> = violations
        .iter()
        .filter(|v| v.kind == crate::verifier::Kind::Reference)
        .map(|v| v.text.as_str())
        .collect();
    if refs.is_empty() {
        "The answer cited a passage that was not among those found.".to_string()
    } else {
        format!(
            "The answer named {} without support from the passages found.",
            refs.join(", ")
        )
    }
}

fn max_opt(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (Some(x), None) => Some(x),
        (None, y) => y,
    }
}

fn file_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

/// The three canned questions of PLAN 7.1 step 4.
///
/// They are the wording of s01, s03 and s13 from the smoke pool, so the
/// self-test asks what a reader would ask rather than something chosen to pass.
pub const SELF_TEST_IDS: [&str; 3] = ["s01", "s03", "s13"];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelfTestResult {
    pub questions: Vec<SelfTestQuestion>,
    pub passed: bool,
    pub seconds: f64,
    pub ran_at: String,
    pub model_id: String,
    pub index_version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelfTestQuestion {
    pub id: String,
    pub question: String,
    pub verdict: String,
    pub cited: usize,
    pub sent: usize,
    pub fabrications: usize,
    pub seconds: f64,
    pub ok: bool,
}

impl Session {
    /// PLAN 7.1 step 4: three canned questions end to end. It passes only if
    /// all three produce a verified answer with nothing fabricated in it.
    pub fn self_test(
        &mut self,
        questions: &[(String, String)],
        on: &mut impl FnMut(Stage),
    ) -> Result<SelfTestResult, String> {
        let t0 = Instant::now();
        let mut rows = Vec::new();
        for (id, q) in questions {
            let t = Instant::now();
            let a = self.ask(q, on)?;
            let shown = a.synopsis_markdown.clone().or(a.fallback_markdown.clone()).unwrap_or_default();
            let sent: std::collections::HashSet<&str> =
                a.passages.iter().filter(|p| p.sent).filter_map(|p| p.token.as_deref()).collect();
            let fabrications = crate::verifier::cited_tokens(&shown)
                .iter()
                .filter(|t| !sent.contains(t.as_str()))
                .count();
            rows.push(SelfTestQuestion {
                id: id.clone(),
                question: q.clone(),
                verdict: a.verdict.clone(),
                cited: a.cited_tokens.len(),
                sent: a.sent_count,
                fabrications,
                seconds: t.elapsed().as_secs_f64(),
                ok: a.verdict == "ok" && fabrications == 0,
            });
        }
        let passed = !rows.is_empty() && rows.iter().all(|r| r.ok);
        Ok(SelfTestResult {
            passed,
            seconds: t0.elapsed().as_secs_f64(),
            ran_at: crate::userdb::iso8601(crate::userdb::now_secs()),
            model_id: file_name(&self.engine.settings.chat_model),
            index_version: self.engine.retriever.index.index_version.clone(),
            questions: rows,
        })
    }
}
