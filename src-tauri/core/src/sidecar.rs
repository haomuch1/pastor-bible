//! The llama-server sidecar: spawning it, talking to it, and above all making
//! sure it dies.
//!
//! docs/SIDECAR.md records the pinned build, the exact flags and the endpoints,
//! all verified against the binary. The rules this module enforces are P3's,
//! carried forward: one model process at a time, one slot so the server cannot
//! serve two requests at once, a free-RAM check before every load, and below
//! normal priority so the machine stays usable while a 5 GB model is running.
//!
//! An orphaned llama-server holding five gigabytes is the worst failure this
//! program could leave behind, so the child is killed on drop, on panic, and by
//! the kernel if the parent dies without running any code at all.

use std::io::Read;
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// The error text a cancelled generation returns, so callers can tell a
/// cancellation from a failure without matching on prose.
pub const CANCELLED: &str = "cancelled";

/// How many sidecars are alive. A second spawn is refused rather than allowed
/// to race: two model processes at once is the thing the sequential rule exists
/// to prevent, and it is refused here rather than left to the caller to
/// remember. `Options::allow_concurrent` lifts the refusal, and the free-RAM
/// check still has to clear the second model on top of the first, because
/// `free_ram_gb` reads what is actually free with the first one already loaded.
static LIVE: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Embedding,
    Chat,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Embedding => "embedding",
            Role::Chat => "chat",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Options {
    pub server: String,
    pub model: String,
    pub role: Role,
    pub n_ctx: u32,
    pub threads: Option<u32>,
    /// Physical batch. Must hold the longest single input, which is a different
    /// constraint from the context window; a smaller batch makes the server
    /// reject long documents outright.
    pub batch: Option<u32>,
    pub gpu_layers: u32,
    pub log_dir: Option<String>,
    /// Free RAM headroom demanded over the model file's own size.
    pub headroom_gb: f64,
    pub ready_timeout: Duration,
    /// Allow this sidecar to start while another is already running. Off by
    /// default. The RAM check is not skipped when it is on.
    pub allow_concurrent: bool,
}

impl Options {
    pub fn new(server: &str, model: &str, role: Role) -> Self {
        Options {
            server: server.to_string(),
            model: model.to_string(),
            role,
            n_ctx: match role {
                Role::Embedding => 2048,
                Role::Chat => 8192,
            },
            threads: None,
            batch: match role {
                Role::Embedding => Some(2048),
                Role::Chat => None,
            },
            gpu_layers: 0,
            log_dir: None,
            headroom_gb: 2.0,
            ready_timeout: Duration::from_secs(900),
            allow_concurrent: false,
        }
    }
}

pub struct Sidecar {
    child: Option<Child>,
    pub port: u16,
    pub role: Role,
    pub model: String,
    pub log_path: Option<String>,
    pub free_ram_before_gb: f64,
    pub ready_seconds: f64,
    counted_down: bool,
    agent: ureq::Agent,
}

fn free_port() -> Result<u16, String> {
    let l = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("no free port: {}", e))?;
    let p = l.local_addr().map_err(|e| e.to_string())?.port();
    drop(l);
    Ok(p)
}

/// Free physical memory in GB, from the OS.
pub fn free_ram_gb() -> f64 {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::SystemInformation::{
            GlobalMemoryStatusEx, MEMORYSTATUSEX,
        };
        unsafe {
            let mut st: MEMORYSTATUSEX = std::mem::zeroed();
            st.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
            if GlobalMemoryStatusEx(&mut st) != 0 {
                return st.ullAvailPhys as f64 / (1024.0 * 1024.0 * 1024.0);
            }
        }
        0.0
    }
    #[cfg(not(windows))]
    {
        if let Ok(text) = std::fs::read_to_string("/proc/meminfo") {
            for line in text.lines() {
                if let Some(rest) = line.strip_prefix("MemAvailable:") {
                    if let Some(kb) = rest.split_whitespace().next() {
                        if let Ok(kb) = kb.parse::<f64>() {
                            return kb / (1024.0 * 1024.0);
                        }
                    }
                }
            }
        }
        0.0
    }
}

/// Stop a process by id.
///
/// Used only to make Stop mean Stop: a llama-server that is deep in prompt
/// processing sends nothing for tens of seconds, and the thread reading its
/// answer cannot notice a cancellation until it does. Killing it unblocks that
/// read at once, and the session starts a new one.
pub fn terminate(pid: u32) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
        unsafe {
            let h = OpenProcess(PROCESS_TERMINATE, 0, pid);
            if !h.is_null() {
                TerminateProcess(h, 1);
                CloseHandle(h);
            }
        }
    }
    #[cfg(not(windows))]
    {
        unsafe {
            libc::kill(pid as i32, libc::SIGKILL);
        }
    }
}

/// Is a process with this id still running?
///
/// Used by the orphan test to prove the child died with its parent. Process ids
/// are reused eventually, so this is evidence rather than proof; the window is
/// milliseconds and the alternative is no check at all.
pub fn process_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
        use windows_sys::Win32::System::Threading::{
            GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() {
                return false;
            }
            let mut code: u32 = 0;
            let ok = GetExitCodeProcess(h, &mut code);
            CloseHandle(h);
            ok != 0 && code == STILL_ACTIVE as u32
        }
    }
    #[cfg(not(windows))]
    {
        std::path::Path::new(&format!("/proc/{}", pid)).exists()
    }
}

/// Peak resident memory of a process, in MB.
pub fn peak_working_set_mb(pid: u32) -> Option<f64> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::ProcessStatus::{
            GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
        };
        use windows_sys::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
        };
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid);
            if h.is_null() {
                return None;
            }
            let mut c: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
            c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
            let ok = GetProcessMemoryInfo(h, &mut c, c.cb);
            CloseHandle(h);
            if ok == 0 {
                return None;
            }
            Some(c.PeakWorkingSetSize as f64 / (1024.0 * 1024.0))
        }
    }
    #[cfg(not(windows))]
    {
        let path = format!("/proc/{}/status", pid);
        let text = std::fs::read_to_string(path).ok()?;
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("VmHWM:") {
                let kb: f64 = rest.split_whitespace().next()?.parse().ok()?;
                return Some(kb / 1024.0);
            }
        }
        None
    }
}

/// Kill the child when this process dies, however it dies.
///
/// On Windows that is a Job Object with KILL_ON_JOB_CLOSE: the kernel kills
/// every process in the job when the last handle to it closes, which happens
/// when the parent exits for any reason, a hard kill included. No user-space
/// handler survives a hard kill, so nothing written in Rust could take its
/// place.
#[cfg(windows)]
mod watchdog {
    use std::process::Child;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, SetInformationJobObject,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JobObjectExtendedLimitInformation,
    };
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE};

    static mut JOB: HANDLE = std::ptr::null_mut();

    pub fn adopt(child: &Child) -> Result<(), String> {
        unsafe {
            if JOB.is_null() {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job.is_null() {
                    return Err("could not create the watchdog job object".to_string());
                }
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const std::ffi::c_void,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                if ok == 0 {
                    return Err("could not set kill-on-close on the watchdog job".to_string());
                }
                JOB = job;
            }
            let h = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, child.id());
            if h.is_null() {
                return Err("could not open the sidecar for the watchdog".to_string());
            }
            let ok = AssignProcessToJobObject(JOB, h);
            windows_sys::Win32::Foundation::CloseHandle(h);
            if ok == 0 {
                return Err("could not put the sidecar in the watchdog job".to_string());
            }
        }
        Ok(())
    }
}

#[cfg(not(windows))]
mod watchdog {
    use std::process::Child;
    pub fn adopt(_child: &Child) -> Result<(), String> {
        // On Linux the death signal is set in the child before exec; see
        // `spawn`. Nothing to do once it is running.
        Ok(())
    }
}

impl Sidecar {
    /// Spawn a server for one role, wait for it to be ready, and return it.
    pub fn start(opts: &Options) -> Result<Self, String> {
        let live = LIVE.fetch_add(1, Ordering::SeqCst);
        if live > 0 && !opts.allow_concurrent {
            LIVE.fetch_sub(1, Ordering::SeqCst);
            return Err(format!(
                "a sidecar is already running and this one did not ask to run \
                 beside it; refused. Two model processes on one machine is what \
                 the sequential rule exists to prevent. ({} live)",
                live
            ));
        }
        match Self::start_inner(opts) {
            Ok(s) => Ok(s),
            Err(e) => {
                LIVE.fetch_sub(1, Ordering::SeqCst);
                Err(e)
            }
        }
    }

    fn start_inner(opts: &Options) -> Result<Self, String> {
        let size_gb = std::fs::metadata(&opts.model)
            .map_err(|e| format!("cannot read model {}: {}", opts.model, e))?
            .len() as f64
            / (1024.0 * 1024.0 * 1024.0);
        let free = free_ram_gb();
        let need = size_gb + opts.headroom_gb;
        if free < need {
            return Err(format!(
                "refusing to load {}: needs {:.1} GB ({:.1} GB model + {:.1} GB headroom) \
                 but only {:.2} GB is free",
                opts.model, need, size_gb, opts.headroom_gb, free
            ));
        }

        let port = free_port()?;
        let threads = opts.threads.unwrap_or_else(|| {
            std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(4)
        });

        let mut cmd = Command::new(&opts.server);
        cmd.arg("-m")
            .arg(&opts.model)
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(port.to_string())
            .arg("-c")
            .arg(opts.n_ctx.to_string())
            .arg("-t")
            .arg(threads.to_string())
            .arg("-ngl")
            .arg(opts.gpu_layers.to_string())
            .arg("--no-webui");
        match opts.role {
            Role::Embedding => {
                cmd.arg("--embeddings");
                if let Some(b) = opts.batch {
                    cmd.arg("-b").arg(b.to_string()).arg("-ub").arg(b.to_string());
                }
            }
            // One slot: the server cannot serve two requests concurrently, so
            // "one question at a time" is enforced by the server and not only
            // by the caller.
            Role::Chat => {
                cmd.arg("-np").arg("1");
                if let Some(b) = opts.batch {
                    cmd.arg("-b").arg(b.to_string()).arg("-ub").arg(b.to_string());
                }
            }
        }

        // llama-server is chatty and an undrained pipe fills its OS buffer and
        // blocks the process, so its output goes to a file or to nowhere.
        let log_path = opts.log_dir.as_ref().map(|d| {
            let _ = std::fs::create_dir_all(d);
            format!("{}/llama-{}-{}.log", d, opts.role.as_str(), port)
        });
        match &log_path {
            Some(p) => {
                let f = std::fs::File::create(p).map_err(|e| format!("cannot open {}: {}", p, e))?;
                let f2 = f.try_clone().map_err(|e| e.to_string())?;
                cmd.stdout(Stdio::from(f)).stderr(Stdio::from(f2));
            }
            None => {
                cmd.stdout(Stdio::null()).stderr(Stdio::null());
            }
        }
        cmd.stdin(Stdio::null());

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(BELOW_NORMAL_PRIORITY_CLASS | CREATE_NO_WINDOW);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            unsafe {
                cmd.pre_exec(|| {
                    // Die with the parent, whatever kills it.
                    libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
                    // Below normal priority, so the machine stays usable.
                    libc::nice(10);
                    Ok(())
                });
            }
        }

        let child = cmd.spawn().map_err(|e| format!("cannot start {}: {}", opts.server, e))?;
        if let Err(e) = watchdog::adopt(&child) {
            // A sidecar we cannot guarantee to kill is worse than no sidecar.
            let mut child = child;
            let _ = child.kill();
            let _ = child.wait();
            return Err(e);
        }

        let mut s = Sidecar {
            child: Some(child),
            port,
            role: opts.role,
            model: opts.model.clone(),
            log_path,
            free_ram_before_gb: free,
            ready_seconds: 0.0,
            counted_down: false,
            agent: ureq::AgentBuilder::new()
                .timeout_connect(Duration::from_secs(5))
                .timeout(Duration::from_secs(3600))
                .build(),
        };
        s.wait_ready(opts.ready_timeout)?;
        Ok(s)
    }

    fn log_tail(&self) -> String {
        let Some(p) = &self.log_path else {
            return "(no log file)".to_string();
        };
        let mut buf = String::new();
        if std::fs::File::open(p).and_then(|mut f| f.read_to_string(&mut buf)).is_err() {
            return "(log unreadable)".to_string();
        }
        let start = buf.len().saturating_sub(3000);
        buf[start..].to_string()
    }

    /// Poll /health until it answers 200, checking on every pass that the child
    /// is still alive so a server that dies at load fails with its log tail
    /// rather than at the timeout.
    fn wait_ready(&mut self, timeout: Duration) -> Result<(), String> {
        let t0 = Instant::now();
        let url = format!("http://127.0.0.1:{}/health", self.port);
        while t0.elapsed() < timeout {
            if let Some(child) = self.child.as_mut() {
                if let Ok(Some(status)) = child.try_wait() {
                    let tail = self.log_tail();
                    self.finish();
                    return Err(format!("llama-server exited early ({}):\n{}", status, tail));
                }
            }
            if let Ok(r) = self.agent.get(&url).timeout(Duration::from_secs(3)).call() {
                if r.status() == 200 {
                    self.ready_seconds = t0.elapsed().as_secs_f64();
                    return Ok(());
                }
            }
            std::thread::sleep(Duration::from_millis(400));
        }
        let tail = self.log_tail();
        self.finish();
        Err(format!("llama-server was not ready in {:?}:\n{}", timeout, tail))
    }

    pub fn pid(&self) -> Option<u32> {
        self.child.as_ref().map(|c| c.id())
    }

    pub fn peak_ram_mb(&self) -> Option<f64> {
        self.pid().and_then(peak_working_set_mb)
    }

    fn post(&self, path: &str, body: serde_json::Value) -> Result<serde_json::Value, String> {
        let url = format!("http://127.0.0.1:{}{}", self.port, path);
        let resp = self
            .agent
            .post(&url)
            .set("Content-Type", "application/json")
            .send_json(body)
            .map_err(|e| format!("{} failed: {}", path, e))?;
        resp.into_json().map_err(|e| format!("{} returned no JSON: {}", path, e))
    }

    /// Embed a batch of strings through /v1/embeddings, unit-normalised.
    pub fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        if self.role != Role::Embedding {
            return Err("this sidecar was not started for embedding".to_string());
        }
        let payload = self.post("/v1/embeddings", serde_json::json!({ "input": texts }))?;
        let data = payload["data"].as_array().ok_or("no data in the embedding response")?;
        let mut rows: Vec<(usize, Vec<f32>)> = Vec::new();
        for d in data {
            let i = d["index"].as_u64().unwrap_or(0) as usize;
            let v: Vec<f32> = d["embedding"]
                .as_array()
                .ok_or("an embedding was not an array")?
                .iter()
                .map(|x| x.as_f64().unwrap_or(0.0) as f32)
                .collect();
            rows.push((i, v));
        }
        rows.sort_by_key(|(i, _)| *i);
        Ok(rows.into_iter().map(|(_, v)| normalize(v)).collect())
    }

    /// One completion through /v1/chat/completions.
    ///
    /// Greedy by default, with a fixed seed, so two runs of the same question
    /// give the same answer and a measurement means something.
    pub fn complete(&self, prompt: &str, max_tokens: u32, seed: i64) -> Result<Completion, String> {
        if self.role != Role::Chat {
            return Err("this sidecar was not started for chat".to_string());
        }
        let t0 = Instant::now();
        let payload = self.post(
            "/v1/chat/completions",
            serde_json::json!({
                "messages": [{"role": "user", "content": prompt}],
                "max_tokens": max_tokens,
                "temperature": 0.0,
                "top_k": 1,
                "top_p": 1.0,
                "seed": seed,
            }),
        )?;
        let raw = payload["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string();
        Ok(Completion {
            text: strip_think(&raw).trim().to_string(),
            raw,
            seconds: t0.elapsed().as_secs_f64(),
            prompt_tokens: payload["usage"]["prompt_tokens"].as_u64(),
            completion_tokens: payload["usage"]["completion_tokens"].as_u64(),
        })
    }

    /// One completion, read as it is produced, so the wait can be counted and
    /// stopped.
    ///
    /// The tokens are not shown to the reader: PLAN 5.6 forbids that, because a
    /// reference is only safe once the verifier has seen the whole answer. What
    /// streaming buys is a running token count for the progress indicator, and
    /// an exit. Dropping the reader closes the connection, and llama-server
    /// abandons the slot when its client goes away, which is the only way to
    /// stop a generation that has already started.
    pub fn complete_streaming(
        &self,
        prompt: &str,
        max_tokens: u32,
        seed: i64,
        cancel: &std::sync::atomic::AtomicBool,
        mut on_token: impl FnMut(u64),
    ) -> Result<Completion, String> {
        if self.role != Role::Chat {
            return Err("this sidecar was not started for chat".to_string());
        }
        let t0 = Instant::now();
        let url = format!("http://127.0.0.1:{}/v1/chat/completions", self.port);
        let resp = self
            .agent
            .post(&url)
            .set("Content-Type", "application/json")
            .send_json(serde_json::json!({
                "messages": [{"role": "user", "content": prompt}],
                "max_tokens": max_tokens,
                "temperature": 0.0,
                "top_k": 1,
                "top_p": 1.0,
                "seed": seed,
                "stream": true,
            }))
            .map_err(|e| format!("generation failed: {}", e))?;

        let mut reader = std::io::BufReader::new(resp.into_reader());
        let mut line = String::new();
        let mut text = String::new();
        let mut tokens: u64 = 0;
        let mut prompt_tokens: Option<u64> = None;
        let mut completion_tokens: Option<u64> = None;

        loop {
            if cancel.load(Ordering::SeqCst) {
                // Dropping the reader here is the cancellation.
                drop(reader);
                return Err(CANCELLED.to_string());
            }
            line.clear();
            let n = std::io::BufRead::read_line(&mut reader, &mut line)
                .map_err(|e| format!("the generation stream broke off: {}", e))?;
            if n == 0 {
                break;
            }
            let Some(payload) = line.trim().strip_prefix("data:") else {
                continue;
            };
            let payload = payload.trim();
            if payload.is_empty() || payload == "[DONE]" {
                continue;
            }
            let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) else {
                continue;
            };
            if let Some(delta) = v["choices"][0]["delta"]["content"].as_str() {
                if !delta.is_empty() {
                    text.push_str(delta);
                    tokens += 1;
                    on_token(tokens);
                }
            }
            if let Some(u) = v.get("usage") {
                if u.is_object() {
                    prompt_tokens = u["prompt_tokens"].as_u64().or(prompt_tokens);
                    completion_tokens = u["completion_tokens"].as_u64().or(completion_tokens);
                }
            }
        }

        Ok(Completion {
            text: strip_think(&text).trim().to_string(),
            raw: text,
            seconds: t0.elapsed().as_secs_f64(),
            prompt_tokens,
            completion_tokens: completion_tokens.or(Some(tokens)),
        })
    }

    /// Is the server free to take another request?
    ///
    /// Used after a cancellation: if the slot is still busy two seconds later
    /// the connection did not stop it, and the caller restarts the sidecar
    /// rather than leaving the next question to queue behind a generation
    /// nobody is waiting for.
    pub fn is_idle(&self) -> bool {
        let url = format!("http://127.0.0.1:{}/slots", self.port);
        let Ok(r) = self.agent.get(&url).timeout(Duration::from_secs(3)).call() else {
            return false;
        };
        let Ok(v) = r.into_json::<serde_json::Value>() else {
            return false;
        };
        match v.as_array() {
            // is_processing is what llama-server reports per slot; a build that
            // stops reporting it is treated as busy, which costs a restart
            // rather than a wedged server.
            Some(slots) => slots.iter().all(|s| s["is_processing"] == serde_json::Value::Bool(false)),
            None => false,
        }
    }

    /// Token count for a string, from the server's own tokenizer, so a length
    /// check means what the model means by it.
    pub fn token_count(&self, text: &str) -> Result<usize, String> {
        let payload = self.post("/tokenize", serde_json::json!({ "content": text }))?;
        Ok(payload["tokens"].as_array().map(|a| a.len()).unwrap_or(0))
    }

    fn finish(&mut self) {
        // Only the first call counts down; finish() runs from wait_ready's
        // failure path, from stop(), and from Drop.
        let had_child = self.child.is_some();
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if had_child && !self.counted_down {
            self.counted_down = true;
            LIVE.fetch_sub(1, Ordering::SeqCst);
        }
    }

    /// How many sidecars are alive right now.
    pub fn live_count() -> usize {
        LIVE.load(Ordering::SeqCst)
    }

    pub fn stop(mut self) {
        self.finish();
    }
}

impl Drop for Sidecar {
    fn drop(&mut self) {
        // Runs on a panic too, because unwinding drops locals.
        self.finish();
    }
}

#[derive(Clone, Debug)]
pub struct Completion {
    pub text: String,
    pub raw: String,
    pub seconds: f64,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
}

/// Qwen3 emits reasoning inside a think block. The reader never sees it, so it
/// must not be part of what the verifier checks. /v1/chat/completions removes
/// it already; this is here so the guarantee does not rest on the server
/// behaving the same way in the next build.
pub fn strip_think(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    loop {
        let lower = rest.to_lowercase();
        let Some(a) = lower.find("<think>") else {
            out.push_str(rest);
            return out;
        };
        out.push_str(&rest[..a]);
        match lower[a..].find("</think>") {
            Some(b) => rest = &rest[a + b + "</think>".len()..],
            None => return out,
        }
    }
}

pub fn normalize(mut v: Vec<f32>) -> Vec<f32> {
    let s: f64 = v.iter().map(|x| (*x as f64) * (*x as f64)).sum::<f64>().sqrt();
    if s > 0.0 {
        for x in v.iter_mut() {
            *x = ((*x as f64) / s) as f32;
        }
    }
    v
}
