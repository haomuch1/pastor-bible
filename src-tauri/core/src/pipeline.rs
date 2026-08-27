//! One question, end to end.
//!
//! The order is fixed and the sequencing rules are P3's, carried forward
//! because the machine has not changed: embed the query, stop the embedding
//! server, start the chat server, generate, verify, retry once, fall back.
//! Two model processes are never alive at once unless `allow_both_servers` is
//! set and the free-RAM check has cleared their sum, and the path that ran is
//! recorded in the answer.

use std::time::Instant;

use crate::api::*;
use crate::crisis::CrisisMatcher;
use crate::prompts::{fill, Prompts};
use crate::retrieve::{CanonMode, Config, Passage, Retriever};
use crate::sidecar::{peak_working_set_mb, Options, Role, Sidecar};
use crate::verifier::{cited_tokens, Sent, Verdict, Verifier};

pub const DEFAULT_CHAT_GGUF: &str = "Qwen3-8B-Q4_K_M.gguf";
pub const FALLBACK_CHAT_GGUF: &str = "Qwen3-1.7B-Q8_0.gguf";
pub const EMBED_GGUF: &str = "nomic-embed-text-v1.5-f16.gguf";
pub const EMBED_MODEL_ID: &str = "nomic-embed-text-v1.5";
pub const TOP_N: usize = 25;
pub const DEUTERO_SLICE: usize = 8;
pub const MAX_TOKENS: u32 = 900;
pub const SEED: i64 = 20260826;

/// How the query is turned into search terms.
///
/// P3 measured that model rewrites lowered recall against hand-written keyword
/// lists. Hand-written lists do not exist at run time, so P4 measured the
/// rewrite against the raw question instead; the default below is what that
/// measurement chose, and the other modes stay reachable by flag.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum QueryMode {
    /// The question itself, embedded and used as the keyword list.
    Raw,
    /// The model's rewritten queries only.
    Rewrite,
    /// The question and the model's rewrites together.
    Fused,
}

impl QueryMode {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "raw" => Ok(QueryMode::Raw),
            "rewrite" => Ok(QueryMode::Rewrite),
            "fused" => Ok(QueryMode::Fused),
            _ => Err(format!("unknown query mode {:?}; expected raw, rewrite or fused", s)),
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            QueryMode::Raw => "raw",
            QueryMode::Rewrite => "rewrite",
            QueryMode::Fused => "fused",
        }
    }
    pub fn needs_model(&self) -> bool {
        !matches!(self, QueryMode::Raw)
    }
}

pub struct Settings {
    pub index_db: String,
    pub llama_server: String,
    pub chat_model: String,
    pub embed_model: String,
    pub prompts_dir: String,
    pub crisis_terms: String,
    pub crisis_note: String,
    pub log_dir: Option<String>,
    pub canon: CanonMode,
    pub query_mode: QueryMode,
    pub chat_ctx: u32,
    pub threads: Option<u32>,
    /// Layers to offload to the GPU. Zero everywhere the app ships today: the
    /// bundled sidecar is the CPU build and PLAN's hardware floor is a CPU
    /// floor. P4 measures a Vulkan build through this flag; P6 decides whether
    /// one is bundled.
    pub gpu_layers: u32,
    /// Allow the embedding server and the chat server to be alive at once.
    /// Off by default; the RAM check still has to clear their sum.
    pub allow_both_servers: bool,
}

pub struct Engine {
    pub retriever: Retriever,
    pub verifier: Verifier,
    pub crisis: CrisisMatcher,
    pub prompts: Prompts,
    pub settings: Settings,
    pub index_load_seconds: f64,
}

impl Engine {
    pub fn open(settings: Settings) -> Result<Self, String> {
        let t0 = Instant::now();
        let retriever = Retriever::open(&settings.index_db, EMBED_MODEL_ID)?;
        let index_load_seconds = t0.elapsed().as_secs_f64();
        let verifier = Verifier::new(&retriever.index);
        let crisis = CrisisMatcher::load(&settings.crisis_terms, &settings.crisis_note)?;
        let prompts = Prompts::load(&settings.prompts_dir)?;
        Ok(Engine { retriever, verifier, crisis, prompts, settings, index_load_seconds })
    }

    /// Passages as `[P1] .. [Pn]`, with reference, text and canon marker.
    ///
    /// The text comes from index.db. Nothing a model wrote ever reaches this
    /// function.
    pub fn pack(&self, passages: &[Passage]) -> (String, Vec<Sent>) {
        let mut blocks = Vec::new();
        let mut sent = Vec::new();
        for (i, p) in passages.iter().enumerate() {
            let token = format!("[P{}]", i + 1);
            let texts: Vec<String> =
                self.retriever.index.text_of(&p.verse_ids).into_iter().map(|(_, t)| t).collect();
            let marker = if p.canon == "deutero" { " (Deuterocanon)" } else { "" };
            blocks.push(format!("{} {}{}\n{}", token, p.reference, marker, texts.join(" ")));
            // The prompt block above keeps the compact reference, which is what
            // P3 and P4 measured and what the parity fixtures hold. The record
            // kept beside it carries the form a reader is shown, because the
            // fallback is read by the reader and the prompt never is.
            sent.push(Sent {
                token,
                reference: p.display_reference.clone(),
                verse_ids: p.verse_ids.clone(),
            });
        }
        (blocks.join("\n\n"), sent)
    }

    /// Embed, and either stop the embedding server or hand it back alive.
    ///
    /// Sequential by default: the embedding server is stopped before the chat
    /// server starts, so one model process exists at a time. With
    /// `allow_both_servers` the embedding server is kept up and the chat server
    /// starts beside it, which saves reloading 262 MB per question in an
    /// interactive session. The free-RAM check is not skipped either way; it
    /// simply sees the first model already resident when it runs for the
    /// second.
    fn embed_query(
        &self,
        texts: &[String],
    ) -> Result<(Vec<Vec<f32>>, f64, f64, Option<Sidecar>), String> {
        let mut opts =
            Options::new(&self.settings.llama_server, &self.settings.embed_model, Role::Embedding);
        opts.log_dir = self.settings.log_dir.clone();
        opts.threads = self.settings.threads;
        let t0 = Instant::now();
        let server = Sidecar::start(&opts)?;
        let up = t0.elapsed().as_secs_f64();
        let t1 = Instant::now();
        let vecs = server.embed(texts);
        let took = t1.elapsed().as_secs_f64();
        if !self.settings.allow_both_servers {
            server.stop();
            return Ok((vecs?, up, took, None));
        }
        Ok((vecs?, up, took, Some(server)))
    }

    /// The whole pipeline for one question.
    pub fn ask(&self, question: &str) -> Result<Answer, String> {
        let t_start = Instant::now();
        let mut timings = Timings { index_load_seconds: self.index_load_seconds, ..Default::default() };
        let mut peak_ram: Option<f64> = None;

        let crisis_term = self.crisis.matches(question);
        let crisis = crisis_term.is_some();

        // ---- query terms -------------------------------------------------
        // A rewrite needs the chat model, and the chat model must not be up
        // while the embedding server is. So the rewrite runs first, alone, and
        // its server is stopped before the embedding server starts. That is
        // one extra model load, which is why the raw mode is cheaper as well
        // as, on measurement, no worse.
        let mut rewrites: Vec<String> = Vec::new();
        if self.settings.query_mode.needs_model() {
            let mut opts = self.chat_options();
            opts.n_ctx = 2048;
            let t = Instant::now();
            let server = Sidecar::start(&opts)?;
            timings.chat_server_seconds += t.elapsed().as_secs_f64();
            let p = fill(self.prompts.body("rewrite"), &[("question", question)]);
            let out = server.complete(&with_no_think(&p), 200, SEED);
            peak_ram = max_opt(peak_ram, server.peak_ram_mb());
            server.stop();
            rewrites = parse_queries(&out?.text);
        }

        let (query_text, keywords) = match self.settings.query_mode {
            QueryMode::Raw => (question.to_string(), question_terms(question)),
            QueryMode::Rewrite => {
                if rewrites.is_empty() {
                    (question.to_string(), question_terms(question))
                } else {
                    (format!("{} {}", question, rewrites.join(" ")), rewrites.clone())
                }
            }
            QueryMode::Fused => {
                let mut kw = question_terms(question);
                kw.extend(rewrites.iter().cloned());
                (format!("{} {}", question, rewrites.join(" ")), kw)
            }
        };

        // ---- embed -------------------------------------------------------
        let prefixed = format!("{}{}", self.retriever.query_prefix, query_text);
        let (vecs, up, took, kept_embed) = self.embed_query(&[prefixed])?;
        timings.embed_server_seconds = up;
        timings.embed_seconds = took;
        let concurrent = kept_embed.is_some();
        let qvec = vecs.into_iter().next().ok_or("the embedding server returned nothing")?;

        // ---- retrieve ----------------------------------------------------
        let t = Instant::now();
        let (full, _top, topics) = self.retriever.search(
            &qvec,
            &keywords,
            self.settings.canon,
            Config::f(),
            TOP_N,
            100,
            DEUTERO_SLICE,
        );
        let ranges = self.retriever.as_ranges(&full);
        let cut = self.retriever.top_cut(&ranges, self.settings.canon, TOP_N, DEUTERO_SLICE);
        timings.retrieve_seconds = t.elapsed().as_secs_f64();

        let (packed, sent) = self.pack(&cut);

        // ---- generate, verify, retry once, fall back ----------------------
        let t = Instant::now();
        let server = Sidecar::start(&self.chat_options())?;
        timings.chat_server_seconds += t.elapsed().as_secs_f64();

        let mut attempts: Vec<AttemptOut> = Vec::new();
        let prompt = fill(
            self.prompts.body("synopsis"),
            &[("question", question), ("passages", &packed)],
        );
        let gen = server.complete(&with_no_think(&prompt), MAX_TOKENS, SEED)?;
        timings.generate_seconds = gen.seconds;

        let tv = Instant::now();
        let (verdict, violations) = self.verifier.check(&self.retriever.index, &gen.text, &sent);
        timings.verify_seconds += tv.elapsed().as_secs_f64();
        attempts.push(AttemptOut {
            verdict: verdict.as_str().to_string(),
            seconds: gen.seconds,
            prompt_tokens: gen.prompt_tokens,
            completion_tokens: gen.completion_tokens,
            violations: violations.iter().map(to_out).collect(),
        });

        let mut final_text = gen.text.clone();
        let mut fallback_used = false;
        if verdict == Verdict::Violation {
            let note = Verifier::failure_note(&violations);
            let p2 = fill(
                self.prompts.body("retry"),
                &[("failure", &note), ("question", question), ("passages", &packed)],
            );
            let gen2 = server.complete(&with_no_think(&p2), MAX_TOKENS, SEED)?;
            timings.retry_seconds = gen2.seconds;
            let tv = Instant::now();
            let (v2, viol2) = self.verifier.check(&self.retriever.index, &gen2.text, &sent);
            timings.verify_seconds += tv.elapsed().as_secs_f64();
            attempts.push(AttemptOut {
                verdict: v2.as_str().to_string(),
                seconds: gen2.seconds,
                prompt_tokens: gen2.prompt_tokens,
                completion_tokens: gen2.completion_tokens,
                violations: viol2.iter().map(to_out).collect(),
            });
            if v2 == Verdict::Ok {
                final_text = gen2.text;
            } else {
                fallback_used = true;
            }
        }
        peak_ram = max_opt(peak_ram, server.peak_ram_mb());
        server.stop();
        if let Some(e) = kept_embed {
            peak_ram = max_opt(peak_ram, e.peak_ram_mb());
            e.stop();
        }

        let answer = self.assemble(
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
            peak_ram,
            concurrent,
            t_start,
        );
        Ok(answer)
    }

    fn chat_options(&self) -> Options {
        let mut opts =
            Options::new(&self.settings.llama_server, &self.settings.chat_model, Role::Chat);
        opts.n_ctx = self.settings.chat_ctx;
        opts.threads = self.settings.threads;
        opts.gpu_layers = self.settings.gpu_layers;
        opts.log_dir = self.settings.log_dir.clone();
        opts.allow_concurrent = self.settings.allow_both_servers;
        opts
    }

    /// The passages as the panel shows them, before any answer exists.
    ///
    /// Nothing is cited yet, so `cited` is false everywhere and the tokens are
    /// the ones the passages were sent under. The verse text is read from
    /// index.db here exactly as it is in `assemble`.
    pub fn passages_for_display(
        &self,
        ranges: &[Passage],
        cut: &[Passage],
        topics: &[crate::retrieve::MatchedTopic],
    ) -> (Vec<PassageOut>, Vec<TopicGroup>) {
        let token_of: std::collections::HashMap<&str, String> = cut
            .iter()
            .enumerate()
            .map(|(i, p)| (p.reference.as_str(), format!("[P{}]", i + 1)))
            .collect();
        let list = ranges
            .iter()
            .map(|p| {
                let token = token_of.get(p.reference.as_str()).cloned();
                PassageOut {
                    cited: false,
                    sent: token.is_some(),
                    token,
                    reference: p.display_reference.clone(),
                    verse_ids: p.verse_ids.clone(),
                    verses: self.verses_of(&p.verse_ids),
                    score: p.score,
                    origins: p.origins.clone(),
                    canon: p.canon.clone(),
                }
            })
            .collect();
        let (_, groups) = self.group_by_topic(ranges, topics);
        (list, groups)
    }

    fn verses_of(&self, ids: &[i64]) -> Vec<VerseOut> {
        self.retriever
            .index
            .text_of(ids)
            .into_iter()
            .map(|(vid, text)| VerseOut {
                verse_id: vid,
                reference: self.retriever.index.verse_reference(vid),
                text,
            })
            .collect()
    }

    /// Build the answer the frontend receives. Public because `Session` does
    /// the same assembly after its own staged run.
    #[allow(clippy::too_many_arguments)]
    pub fn assemble(
        &self,
        question: &str,
        crisis: bool,
        ranges: &[Passage],
        cut: &[Passage],
        sent: &[Sent],
        topics: &[crate::retrieve::MatchedTopic],
        synopsis: Option<String>,
        attempts: Vec<AttemptOut>,
        fallback_used: bool,
        mut timings: Timings,
        peak_ram: Option<f64>,
        concurrent: bool,
        t_start: Instant,
    ) -> Answer {
        let token_of: std::collections::HashMap<&str, &str> =
            cut.iter().zip(sent.iter()).map(|(p, s)| (p.reference.as_str(), s.token.as_str())).collect();

        let cited = synopsis.as_deref().map(cited_tokens).unwrap_or_default();
        let cited_set: std::collections::HashSet<&str> = cited.iter().map(|s| s.as_str()).collect();
        let sent_by_token: std::collections::HashMap<&str, &Sent> =
            sent.iter().map(|s| (s.token.as_str(), s)).collect();

        let mut cited_passage_ids: Vec<i64> = Vec::new();
        let mut deutero_cited = false;
        for t in &cited {
            if let Some(s) = sent_by_token.get(t.as_str()) {
                cited_passage_ids.extend(s.verse_ids.iter().copied());
                if self.retriever.index.canon_of_verse(s.verse_ids[0]) == "deutero" {
                    deutero_cited = true;
                }
            }
        }
        cited_passage_ids.sort();
        cited_passage_ids.dedup();

        let passages: Vec<PassageOut> = ranges
            .iter()
            .map(|p| {
                let token = token_of.get(p.reference.as_str()).map(|t| t.to_string());
                let verses: Vec<VerseOut> = self
                    .retriever
                    .index
                    .text_of(&p.verse_ids)
                    .into_iter()
                    .map(|(vid, text)| VerseOut {
                        verse_id: vid,
                        reference: self.retriever.index.verse_reference(vid),
                        text,
                    })
                    .collect();
                PassageOut {
                    cited: token.as_deref().map(|t| cited_set.contains(t)).unwrap_or(false),
                    sent: token.is_some(),
                    token,
                    reference: p.display_reference.clone(),
                    verse_ids: p.verse_ids.clone(),
                    verses,
                    score: p.score,
                    origins: p.origins.clone(),
                    canon: p.canon.clone(),
                }
            })
            .collect();

        let (topics_out, topic_groups) = self.group_by_topic(ranges, topics);

        timings.total_seconds = t_start.elapsed().as_secs_f64();

        Answer {
            question: question.to_string(),
            canon_mode: self.settings.canon.as_str().to_string(),
            crisis,
            crisis_note: if crisis { Some(self.crisis.note.clone()) } else { None },
            verdict: attempts
                .last()
                .map(|a| if fallback_used { "fallback".to_string() } else { a.verdict.clone() })
                .unwrap_or_else(|| "fallback".to_string()),
            fallback_markdown: if fallback_used {
                Some(Verifier::fallback(&self.retriever.index, sent))
            } else {
                None
            },
            synopsis_markdown: synopsis,
            attempts,
            fallback_used,
            cited_tokens: cited,
            cited_passage_ids,
            deuterocanon_cited: deutero_cited,
            deuterocanon_footer: if deutero_cited {
                Some(
                    "This answer includes passages from the Deuterocanon, which some \
                     traditions include and others do not. Each is labelled."
                        .to_string(),
                )
            } else {
                None
            },
            sent_count: sent.len(),
            passages,
            topics: topics_out,
            topic_groups,
            timings,
            model_id: file_stem(&self.settings.chat_model),
            embedding_model_id: EMBED_MODEL_ID.to_string(),
            index_version: self.retriever.index.index_version.clone(),
            prompt_versions: self.prompts.versions(),
            // What actually happened, not what was asked for.
            sidecar_path: if concurrent {
                "concurrent".to_string()
            } else {
                "sequential".to_string()
            },
            peak_ram_mb: peak_ram,
            query_mode: self.settings.query_mode.as_str().to_string(),
        }
    }

    /// The root of a Nave's topic, and the path down to the matched subtopic.
    ///
    /// Nave's is a tree, and what retrieval matches is usually a leaf: not
    /// "PRIDE" but "INSTANCES OF Ahithophel Naaman, refusing to wash in the
    /// Jordan River...". A leaf like that is unreadable as a group label and a
    /// root like "PRIDE" is exactly what a reader recognises, so the group is
    /// the root and the leaf becomes a second line beneath it.
    fn topic_root(&self, topic_id: i64) -> (i64, String) {
        let mut id = topic_id;
        let mut heading = String::new();
        // Nave's is shallow, but a cycle in the data would hang the window, so
        // the walk is bounded rather than trusting the shape of the table.
        for _ in 0..16 {
            let row: rusqlite::Result<(String, Option<i64>)> = self.retriever.index.con.query_row(
                "SELECT heading, parent_topic_id FROM nave_topics WHERE topic_id = ?",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            );
            match row {
                Ok((h, parent)) => {
                    heading = h;
                    match parent {
                        Some(p) if p != id => id = p,
                        _ => break,
                    }
                }
                Err(_) => break,
            }
        }
        (id, heading)
    }

    /// PLAN 5.6 as amended: the retrieved set under the matched topic headings.
    ///
    /// A passage may belong to more than one topic; it is listed under the
    /// first, strongest one, so the groups partition the set and the reader is
    /// not shown the same passage four times.
    fn group_by_topic(
        &self,
        ranges: &[Passage],
        topics: &[crate::retrieve::MatchedTopic],
    ) -> (Vec<TopicOut>, Vec<TopicGroup>) {
        let mut topics_out: Vec<TopicOut> = Vec::new();
        let mut groups: Vec<TopicGroup> = Vec::new();
        let mut claimed: std::collections::HashSet<&str> = std::collections::HashSet::new();

        for t in topics {
            let verses: std::collections::HashSet<i64> = self
                .retriever
                .index
                .con
                .prepare_cached("SELECT verse_id FROM nave_topic_verses WHERE topic_id = ?")
                .and_then(|mut st| {
                    st.query_map([t.topic_id], |r| r.get::<_, i64>(0))
                        .map(|rows| rows.filter_map(|r| r.ok()).collect())
                })
                .unwrap_or_default();

            let mut all_refs: Vec<&Passage> =
                ranges.iter().filter(|p| p.verse_ids.iter().any(|v| verses.contains(v))).collect();
            all_refs.sort_by_key(|p| p.verse_ids.first().copied().unwrap_or(0));

            let (root_id, root_heading) = self.topic_root(t.topic_id);
            topics_out.push(TopicOut {
                topic_id: t.topic_id,
                heading_display: short_heading(&t.heading),
                heading: t.heading.clone(),
                verses: t.verses,
                score: t.score,
                passage_refs: all_refs.iter().map(|p| p.display_reference.clone()).collect(),
            });

            let mine: Vec<String> = all_refs
                .iter()
                .filter(|p| claimed.insert(p.display_reference.as_str()))
                .map(|p| p.display_reference.clone())
                .collect();
            if !mine.is_empty() {
                // The group is named for the root; the matched subtopic goes
                // underneath, trimmed, so the reader can see why these
                // passages are together without reading a paragraph.
                let sub = if root_id == t.topic_id { String::new() } else { short_heading(&t.heading) };
                groups.push(TopicGroup {
                    heading_display: if root_heading.is_empty() {
                        short_heading(&t.heading)
                    } else {
                        root_heading.clone()
                    },
                    heading: sub,
                    topic_id: Some(root_id),
                    passage_refs: mine,
                });
            }
        }

        // Two matched subtopics under one root become one group.
        let mut merged: Vec<TopicGroup> = Vec::new();
        for g in groups.into_iter() {
            match merged.iter_mut().find(|m| m.topic_id == g.topic_id) {
                Some(m) => {
                    if !g.heading.is_empty() && !m.heading.contains(&g.heading) {
                        if m.heading.is_empty() {
                            m.heading = g.heading.clone();
                        } else {
                            m.heading = format!("{}; {}", m.heading, g.heading);
                        }
                    }
                    m.passage_refs.extend(g.passage_refs);
                }
                None => merged.push(g),
            }
        }
        let mut groups = merged;

        let mut rest: Vec<&Passage> =
            ranges.iter().filter(|p| !claimed.contains(p.display_reference.as_str())).collect();
        rest.sort_by_key(|p| p.verse_ids.first().copied().unwrap_or(0));
        if !rest.is_empty() {
            groups.push(TopicGroup {
                heading: "Other passages".to_string(),
                heading_display: "Other passages".to_string(),
                topic_id: None,
                passage_refs: rest.iter().map(|p| p.display_reference.clone()).collect(),
            });
        }
        (topics_out, groups)
    }
}

/// A Nave's heading trimmed to a label: up to the first sentence break, then
/// hard-capped, on a word boundary where there is one.
pub fn short_heading(heading: &str) -> String {
    short_heading_to(heading, 60)
}

/// A Nave's heading trimmed to a label of at most `max` characters.
pub fn short_heading_to(heading: &str, max: usize) -> String {
    let mut text = heading.trim();
    for stop in ['.', ';', ','] {
        if let Some(i) = text.find(stop) {
            if i >= 8 {
                text = &text[..i];
                break;
            }
        }
    }
    let text = text.trim();
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max).collect();
    let cut = match cut.rfind(' ') {
        Some(i) if i >= 30 => cut[..i].to_string(),
        _ => cut,
    };
    format!("{}...", cut.trim_end())
}

fn to_out(v: &crate::verifier::Violation) -> ViolationOut {
    ViolationOut {
        kind: v.kind.as_str().to_string(),
        text: v.text.clone(),
        reason: v.reason.clone(),
        span: v.char_span,
    }
}

fn max_opt(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (Some(x), None) => Some(x),
        (None, y) => y,
    }
}

/// Qwen3's own switch for turning reasoning off. Reasoning tokens multiply the
/// wait several times over for an answer nobody reads.
pub fn with_no_think(text: &str) -> String {
    format!("{} /no_think", text.trim_end())
}

fn file_stem(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

/// English function words that carry no retrieval signal. Kept short and
/// obvious: this is not a linguistic claim, it is a way of stopping "what does
/// the bible say about" from being five FTS queries that match every verse.
const STOPWORDS: &[&str] = &[
    "a", "about", "all", "am", "an", "and", "any", "are", "as", "at", "be", "because", "been",
    "bible", "but", "by", "can", "did", "do", "does", "doing", "for", "from", "had", "has", "have",
    "he", "her", "him", "his", "how", "i", "if", "in", "into", "is", "it", "its", "me", "my", "of",
    "on", "or", "our", "out", "over", "say", "says", "scripture", "she", "should", "so", "some",
    "such", "tell", "than", "that", "the", "their", "them", "then", "there", "these", "they",
    "this", "those", "to", "up", "us", "was", "we", "were", "what", "when", "where", "which",
    "who", "whom", "why", "will", "with", "would", "you", "your",
];

/// The keyword list for a question with no model rewrite behind it.
///
/// The whole question cannot be used as one FTS term: a term containing a space
/// is quoted as a phrase, and no verse contains the phrase "what does the bible
/// say about anxiety and worry". The content words do the same job the rewrite
/// was meant to do, at no cost and with nothing invented.
pub fn question_terms(question: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    const APOSTROPHE: char = '\u{27}';
    for word in question.split(|c: char| !c.is_alphanumeric() && c != APOSTROPHE) {
        let w = word.trim_matches(APOSTROPHE).to_lowercase();
        if w.len() < 3 || STOPWORDS.contains(&w.as_str()) || out.contains(&w) {
            continue;
        }
        out.push(w);
    }
    out
}

/// Pull a JSON array of strings out of the rewrite output, falling back to
/// lines that look like queries.
pub fn parse_queries(text: &str) -> Vec<String> {
    if let (Some(a), Some(b)) = (text.find('['), text.rfind(']')) {
        if b > a {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text[a..=b]) {
                if let Some(arr) = v.as_array() {
                    let out: Vec<String> = arr
                        .iter()
                        .filter_map(|x| match x {
                            serde_json::Value::String(s) => Some(s.trim().to_string()),
                            serde_json::Value::Number(n) => Some(n.to_string()),
                            _ => None,
                        })
                        .filter(|s| !s.is_empty())
                        .take(5)
                        .collect();
                    if !out.is_empty() {
                        return out;
                    }
                }
            }
        }
    }
    text.lines()
        .map(|l| l.trim().trim_matches(|c| " -*\"'".contains(c)))
        .filter(|l| !l.is_empty() && l.split_whitespace().count() <= 6 && !l.starts_with('#'))
        .map(|l| l.to_string())
        .take(5)
        .collect()
}

/// Peak resident memory of this process, for the RAM figures.
pub fn own_peak_ram_mb() -> Option<f64> {
    peak_working_set_mb(std::process::id())
}
