//! Two things that would break quietly if they broke.
//!
//! The prompt bodies: the Rust loader must strip the same header the Python
//! harness strips and keep the same body, byte for byte. An instruction lost
//! from the top of a prompt changes what the model does and shows up in no
//! diff anyone reads.
//!
//! The output structure: it must survive a round trip through serde, and the
//! fields the citation guarantee rests on must hold their invariants no matter
//! which branch produced the answer.

mod common;

use pastor_bible_core::api::*;
use pastor_bible_core::paths;
use pastor_bible_core::pipeline::{question_terms, short_heading, QueryMode};
use pastor_bible_core::prompts::Prompts;

#[test]
fn prompt_bodies_match_the_python_harness() {
    let fx = common::read_json(&common::fixtures_dir().join("prompts.json"));
    let p = Prompts::load(&paths::prompts_dir()).expect("prompts load");
    for name in ["synopsis", "retry", "rewrite"] {
        let want = fx[name]["body"].as_str().expect("a body in the fixture");
        assert_eq!(
            p.body(name),
            want,
            "the {} prompt body differs from what the Python harness sends",
            name
        );
        assert_eq!(
            p.version(name),
            fx[name]["version"].as_str().unwrap(),
            "the {} prompt version differs",
            name
        );
    }
    assert!(p.drift().is_empty(), "prompt version drift: {:?}", p.drift());
}

#[test]
fn every_prompt_placeholder_is_filled() {
    let p = Prompts::load(&paths::prompts_dir()).expect("prompts load");
    // The pipeline fills exactly these. A prompt that grew a new placeholder
    // would otherwise reach the model with a literal "{something}" in it.
    let known: &[(&str, &[&str])] = &[
        ("synopsis", &["question", "passages"]),
        ("retry", &["failure", "question", "passages"]),
        ("rewrite", &["question"]),
    ];
    let re = regex_lite(r"\{([a-z_]+)\}");
    for (name, allowed) in known {
        for found in re(p.body(name)) {
            assert!(
                allowed.contains(&found.as_str()),
                "the {} prompt uses {{{}}}, which the pipeline does not fill",
                name,
                found
            );
        }
    }
}

/// A very small placeholder finder, so this test needs no extra dependency.
fn regex_lite(_pattern: &str) -> impl Fn(&str) -> Vec<String> {
    |text: &str| {
        let mut out = Vec::new();
        let bytes: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == '{' {
                let start = i + 1;
                let mut j = start;
                while j < bytes.len() && (bytes[j].is_ascii_lowercase() || bytes[j] == '_') {
                    j += 1;
                }
                if j < bytes.len() && bytes[j] == '}' && j > start {
                    out.push(bytes[start..j].iter().collect());
                    i = j;
                }
            }
            i += 1;
        }
        out
    }
}

fn sample_answer() -> Answer {
    Answer {
        question: "What does the Bible say about anxiety?".to_string(),
        canon_mode: "66".to_string(),
        crisis: false,
        crisis_note: None,
        synopsis_markdown: Some("## Trust\nThe text says so [P1].".to_string()),
        fallback_markdown: None,
        verdict: "ok".to_string(),
        attempts: vec![AttemptOut {
            verdict: "ok".to_string(),
            seconds: 1.5,
            prompt_tokens: Some(10),
            completion_tokens: Some(20),
            violations: vec![ViolationOut {
                kind: "reference".to_string(),
                text: "John 3:16".to_string(),
                reason: "verses not in sent set".to_string(),
                span: (3, 12),
            }],
        }],
        fallback_used: false,
        cited_tokens: vec!["[P1]".to_string()],
        cited_passage_ids: vec![55006025],
        deuterocanon_cited: false,
        deuterocanon_footer: None,
        passages: vec![PassageOut {
            token: Some("[P1]".to_string()),
            reference: "Mat 6:25".to_string(),
            verse_ids: vec![55006025],
            verses: vec![VerseOut {
                verse_id: 55006025,
                reference: "Mat 6:25".to_string(),
                text: "Therefore I tell you...".to_string(),
            }],
            score: 0.0123,
            origins: vec!["fts".to_string()],
            canon: "protestant".to_string(),
            cited: true,
            sent: true,
        }],
        sent_count: 1,
        topics: vec![TopicOut {
            topic_id: 7,
            heading: "CARE".to_string(),
            heading_display: "CARE".to_string(),
            verses: 53,
            score: 0.5,
            passage_refs: vec!["Mat 6:25".to_string()],
        }],
        topic_groups: vec![TopicGroup {
            heading: "CARE".to_string(),
            heading_display: "CARE".to_string(),
            topic_id: Some(7),
            passage_refs: vec!["Mat 6:25".to_string()],
        }],
        timings: Timings { total_seconds: 2.0, ..Default::default() },
        model_id: "Qwen3-8B-Q4_K_M".to_string(),
        embedding_model_id: "nomic-embed-text-v1.5".to_string(),
        index_version: "0.2.0".to_string(),
        prompt_versions: vec![("synopsis".to_string(), "1".to_string())],
        sidecar_path: "sequential".to_string(),
        peak_ram_mb: Some(8999.0),
        query_mode: "raw".to_string(),
    }
}

#[test]
fn the_answer_survives_a_round_trip() {
    let a = sample_answer();
    let text = serde_json::to_string(&a).expect("serialize");
    let b: Answer = serde_json::from_str(&text).expect("deserialize");
    assert_eq!(serde_json::to_string(&b).unwrap(), text, "the answer did not round-trip");

    // The field names the frontend and docs/API.md rely on must be present.
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    for key in [
        "question",
        "canon_mode",
        "crisis",
        "crisis_note",
        "synopsis_markdown",
        "fallback_markdown",
        "verdict",
        "attempts",
        "fallback_used",
        "cited_tokens",
        "cited_passage_ids",
        "deuterocanon_cited",
        "deuterocanon_footer",
        "passages",
        "sent_count",
        "topics",
        "topic_groups",
        "timings",
        "model_id",
        "embedding_model_id",
        "index_version",
        "prompt_versions",
        "sidecar_path",
        "peak_ram_mb",
        "query_mode",
    ] {
        assert!(v.get(key).is_some(), "docs/API.md promises the field {:?}", key);
    }
    assert!(v["passages"][0]["verses"][0]["text"].is_string(), "verse text must be carried");
}

#[test]
fn a_fallback_answer_carries_no_synopsis() {
    let mut a = sample_answer();
    a.synopsis_markdown = None;
    a.fallback_markdown = Some("A synthesis could not be produced...".to_string());
    a.fallback_used = true;
    a.verdict = "fallback".to_string();
    a.cited_tokens.clear();
    let v: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
    assert!(v["synopsis_markdown"].is_null(), "a fallback answer must hold no synopsis");
    assert!(v["fallback_markdown"].is_string());
    // Nothing may claim a citation when nothing was verified.
    assert_eq!(v["cited_tokens"].as_array().unwrap().len(), 0);
}

#[test]
fn question_terms_drop_the_scaffolding_and_keep_the_subject() {
    let t = question_terms("What does the Bible say about anxiety and worry?");
    assert_eq!(t, vec!["anxiety", "worry"], "got {:?}", t);
    let t = question_terms("How should a Christian handle anger?");
    assert_eq!(t, vec!["christian", "handle", "anger"], "got {:?}", t);
    // No term may contain a space: a term with one is quoted as an FTS phrase
    // and would match nothing.
    for term in question_terms("What does the Bible say about the fear of the Lord?") {
        assert!(!term.contains(' '), "{:?} would be searched as a phrase", term);
    }
    // Duplicates would weight one word twice in the fusion.
    let t = question_terms("anger and anger and ANGER");
    assert_eq!(t, vec!["anger"]);
}

#[test]
fn long_navesheadings_are_trimmed_for_display() {
    assert_eq!(short_heading("CARE"), "CARE");
    let long = "In prayer for himself and his adversaries , Of Ahab, when Elijah \
                prophesied the destruction of himself and his house";
    let s = short_heading(long);
    assert!(s.chars().count() <= 63, "{:?} is still {} characters", s, s.chars().count());
    assert!(!s.is_empty());
    // The source text is never lost; only the label is trimmed.
    assert!(long.starts_with(s.trim_end_matches("...").trim_end()));
}

#[test]
fn query_modes_parse_and_only_two_of_them_need_the_model() {
    assert!(!QueryMode::parse("raw").unwrap().needs_model());
    assert!(QueryMode::parse("rewrite").unwrap().needs_model());
    assert!(QueryMode::parse("fused").unwrap().needs_model());
    assert!(QueryMode::parse("nonsense").is_err());
}
