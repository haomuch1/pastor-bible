//! The answer, as the frontend receives it.
//!
//! Documented in docs/API.md. Two rules shape this type. Verse text is always
//! carried from index.db and never from model output, at every stage including
//! the fallback. And the synopsis field is populated only when the verifier
//! passed: when it did not, `synopsis` is None and `fallback` carries the
//! grouped passages, so a caller that forgets to check `verdict` still cannot
//! show an unverified reference.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerseOut {
    pub verse_id: i64,
    pub reference: String,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PassageOut {
    /// The opaque token this passage was sent under, when it was sent. None for
    /// a passage that is in the retrieved set but was not sent to the model.
    pub token: Option<String>,
    pub reference: String,
    pub verse_ids: Vec<i64>,
    pub verses: Vec<VerseOut>,
    pub score: f64,
    pub origins: Vec<String>,
    pub canon: String,
    /// True when the synopsis cites this passage.
    pub cited: bool,
    pub sent: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TopicOut {
    pub topic_id: i64,
    pub heading: String,
    /// The heading trimmed to something that fits above a list.
    ///
    /// Nave's writes some subtopics as full sentences and a few as whole
    /// paragraphs, the longest running to over a thousand tokens; P2 recorded
    /// this and decided not to re-derive them. Callers that need the source
    /// text have `heading`; callers that need a label have this.
    pub heading_display: String,
    /// How many verses the topic holds in Nave's, which is not the same as how
    /// many of them were retrieved.
    pub verses: i64,
    pub score: f64,
    /// The verses of this topic that are in the retrieved set, in canonical
    /// order, by passage reference.
    pub passage_refs: Vec<String>,
}

/// PLAN 5.6 as amended on 2026-08-26: the retrieved set grouped under the
/// matched topic headings, topics in match order, passages within a topic in
/// canonical order, and everything else under "Other passages". No model call.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TopicGroup {
    pub heading: String,
    pub heading_display: String,
    pub topic_id: Option<i64>,
    pub passage_refs: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ViolationOut {
    pub kind: String,
    pub text: String,
    pub reason: String,
    pub span: (usize, usize),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttemptOut {
    pub verdict: String,
    pub seconds: f64,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub violations: Vec<ViolationOut>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Timings {
    pub index_load_seconds: f64,
    pub embed_server_seconds: f64,
    pub embed_seconds: f64,
    pub chat_server_seconds: f64,
    pub retrieve_seconds: f64,
    pub generate_seconds: f64,
    pub retry_seconds: f64,
    pub verify_seconds: f64,
    pub total_seconds: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Answer {
    pub question: String,
    pub canon_mode: String,
    pub crisis: bool,
    /// PLAN 9.3 verbatim, present only when `crisis` is true. Shown above the
    /// answer, never instead of it.
    pub crisis_note: Option<String>,

    /// The verified synopsis. None when the verifier rejected both attempts.
    pub synopsis_markdown: Option<String>,
    /// The passages grouped by book with the one-line note, per PLAN 5.6.
    /// Present only when `synopsis_markdown` is None.
    pub fallback_markdown: Option<String>,
    pub verdict: String,
    pub attempts: Vec<AttemptOut>,
    pub fallback_used: bool,

    pub cited_tokens: Vec<String>,
    pub cited_passage_ids: Vec<i64>,
    pub deuterocanon_cited: bool,
    /// One line under the answer when the Deuterocanon contributed, per PLAN 5.7.
    pub deuterocanon_footer: Option<String>,

    /// Every passage retrieved, in rank order, with its text from index.db.
    pub passages: Vec<PassageOut>,
    pub sent_count: usize,
    pub topics: Vec<TopicOut>,
    pub topic_groups: Vec<TopicGroup>,

    pub timings: Timings,
    pub model_id: String,
    pub embedding_model_id: String,
    pub index_version: String,
    pub prompt_versions: Vec<(String, String)>,
    /// Which lifecycle path ran: "sequential" when the embedding server was
    /// stopped before the chat server started, "concurrent" when both were
    /// allowed to be alive at once after a RAM check.
    pub sidecar_path: String,
    pub peak_ram_mb: Option<f64>,
    pub query_mode: String,
}
