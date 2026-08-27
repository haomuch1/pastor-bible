//! Retrieval parity against the Python harness.
//!
//! For each fixture the query vector and the keyword list are the Python
//! harness's own inputs, and the expected output is what pipeline/retrieve.py
//! produced from them: the whole candidate set in rank order with scores and
//! origin tags, the passages those candidates group into, the cut that is sent
//! to generation, and the matched Nave's topics.
//!
//! The score tolerance is 1e-5, as the P4 brief sets it. It is not there to
//! absorb a difference in method: the two implementations do the same
//! arithmetic in the same order, and the only expected difference is the last
//! bit or two of a float32 dot product. Any mismatch beyond that is a bug in
//! one of the two, and the fix is to find which, never to widen the tolerance.

mod common;

use std::time::Instant;

use pastor_bible_core::retrieve::{CanonMode, Config, Retriever};

const TOL: f64 = 1e-5;

#[test]
fn rust_retrieval_reproduces_the_python_harness() {
    let db = common::require_index();
    let dir = common::fixtures_dir().join("retrieval");
    let index_fx = common::read_json(&dir.join("index.json"));
    let cases = index_fx["cases"].as_array().expect("case list");

    let t0 = Instant::now();
    let ret = Retriever::open(&db, "nomic-embed-text-v1.5").expect("open retriever");
    let load_seconds = t0.elapsed().as_secs_f64();

    let mut failures: Vec<String> = Vec::new();
    let mut timings: Vec<(String, f64, f64)> = Vec::new();

    for c in cases {
        let name = c["case"].as_str().unwrap();
        let fx = common::read_json(&dir.join(format!("{}.json", name)));
        let qvec: Vec<f32> =
            fx["qvec"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap() as f32).collect();
        let keywords: Vec<String> = fx["keywords"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        let mode = CanonMode::parse(fx["canon"].as_str().unwrap()).unwrap();
        let top_n = fx["top_n"].as_u64().unwrap() as usize;
        let deut_n = fx["deutero_slice"].as_u64().unwrap() as usize;

        let t = Instant::now();
        let (full, _top, topics) =
            ret.search(&qvec, &keywords, mode, Config::f(), top_n, 100, deut_n);
        let ranges = ret.as_ranges(&full);
        let cut = ret.top_cut(&ranges, mode, top_n, deut_n);
        let seconds = t.elapsed().as_secs_f64();
        timings.push((name.to_string(), seconds, fx["python_seconds"].as_f64().unwrap()));

        let mut fail = |m: String| failures.push(format!("{}: {}", name, m));

        // -- the full candidate set, in rank order
        let want_full = fx["full_set"].as_array().unwrap();
        if want_full.len() != full.len() {
            fail(format!("full set {} in Python, {} here", want_full.len(), full.len()));
        } else {
            for (i, (w, g)) in want_full.iter().zip(full.iter()).enumerate() {
                if w["verse_id"].as_i64().unwrap() != g.verse_id {
                    fail(format!(
                        "full[{}] verse {} in Python, {} here",
                        i,
                        w["verse_id"].as_i64().unwrap(),
                        g.verse_id
                    ));
                    break;
                }
                if (w["score"].as_f64().unwrap() - g.score).abs() > TOL {
                    fail(format!(
                        "full[{}] score {} in Python, {} here",
                        i,
                        w["score"].as_f64().unwrap(),
                        g.score
                    ));
                    break;
                }
                let want_o: Vec<&str> =
                    w["origins"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
                if want_o != g.origins.iter().map(|s| s.as_str()).collect::<Vec<_>>() {
                    fail(format!("full[{}] origins {:?} in Python, {:?} here", i, want_o, g.origins));
                    break;
                }
                if w["canon"].as_str().unwrap() != g.canon {
                    fail(format!("full[{}] canon differs", i));
                    break;
                }
            }
        }

        // -- the passages those candidates group into
        let want_ranges = fx["ranges"].as_array().unwrap();
        if want_ranges.len() != ranges.len() {
            fail(format!("{} passages in Python, {} here", want_ranges.len(), ranges.len()));
        } else {
            for (i, (w, g)) in want_ranges.iter().zip(ranges.iter()).enumerate() {
                let want_ids: Vec<i64> =
                    w["ids"].as_array().unwrap().iter().map(|v| v.as_i64().unwrap()).collect();
                if w["ref"].as_str().unwrap() != g.reference || want_ids != g.verse_ids {
                    fail(format!(
                        "passage[{}] {:?} in Python, {:?} here",
                        i,
                        w["ref"].as_str().unwrap(),
                        g.reference
                    ));
                    break;
                }
                if (w["score"].as_f64().unwrap() - g.score).abs() > TOL {
                    fail(format!("passage[{}] {} score differs", i, g.reference));
                    break;
                }
                let want_o: Vec<&str> =
                    w["origins"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
                if want_o != g.origins.iter().map(|s| s.as_str()).collect::<Vec<_>>() {
                    fail(format!("passage[{}] {} origins differ", i, g.reference));
                    break;
                }
            }
        }

        // -- the cut that is actually sent to generation
        let want_cut = fx["cut"].as_array().unwrap();
        let got_cut: Vec<&str> = cut.iter().map(|p| p.reference.as_str()).collect();
        let want_cut_refs: Vec<&str> =
            want_cut.iter().map(|w| w["ref"].as_str().unwrap()).collect();
        if want_cut_refs != got_cut {
            fail(format!("cut {:?} in Python, {:?} here", want_cut_refs, got_cut));
        }

        // -- matched Nave's topics
        let want_topics = fx["topics"].as_array().unwrap();
        if want_topics.len() != topics.len() {
            fail(format!("{} topics in Python, {} here", want_topics.len(), topics.len()));
        } else {
            for (i, (w, g)) in want_topics.iter().zip(topics.iter()).enumerate() {
                if w["topic_id"].as_i64().unwrap() != g.topic_id
                    || w["heading"].as_str().unwrap() != g.heading
                    || w["verses"].as_i64().unwrap() != g.verses
                    || (w["score"].as_f64().unwrap() - g.score).abs() > TOL
                {
                    fail(format!(
                        "topic[{}] {:?} in Python, {:?} here",
                        i,
                        w["heading"].as_str().unwrap(),
                        g.heading
                    ));
                }
            }
        }
    }

    eprintln!("index load {:.2}s", load_seconds);
    for (name, rust, py) in &timings {
        eprintln!("  {:<9} rust {:.4}s   python {:.4}s", name, rust, py);
    }
    assert!(
        failures.is_empty(),
        "{} retrieval differences:\n{}",
        failures.len(),
        failures.iter().take(30).cloned().collect::<Vec<_>>().join("\n")
    );
}

/// PLAN's additive-canon guarantee, asserted rather than assumed: turning the
/// Deuterocanon on may only add. P3 fixed two bugs of exactly this shape.
#[test]
fn both_canon_is_additive_over_canon_66() {
    let db = common::require_index();
    let dir = common::fixtures_dir().join("retrieval");
    let ret = Retriever::open(&db, "nomic-embed-text-v1.5").expect("open retriever");

    for qid in ["g19", "g20"] {
        let a = common::read_json(&dir.join(format!("{}-66.json", qid)));
        let qvec: Vec<f32> =
            a["qvec"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap() as f32).collect();
        let keywords: Vec<String> =
            a["keywords"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();

        let (full66, _, _) =
            ret.search(&qvec, &keywords, CanonMode::Protestant66, Config::f(), 25, 100, 8);
        let (full_both, _, _) =
            ret.search(&qvec, &keywords, CanonMode::Both, Config::f(), 25, 100, 8);

        assert!(
            full_both.len() >= full66.len(),
            "{}: both-canon returned fewer candidates than canon 66",
            qid
        );
        for (i, c) in full66.iter().enumerate() {
            assert_eq!(
                c.verse_id, full_both[i].verse_id,
                "{}: canon 66 is not a prefix of both at position {}",
                qid, i
            );
        }
        let cut66 = ret.top_cut(&ret.as_ranges(&full66), CanonMode::Protestant66, 25, 8);
        let ranges_both = ret.as_ranges(&full_both);
        let cut_both = ret.top_cut(&ranges_both, CanonMode::Both, 25, 8);
        for p in &cut66 {
            assert!(
                cut_both.iter().any(|q| q.reference == p.reference),
                "{}: {} is sent in canon 66 but not in both-canon mode",
                qid,
                p.reference
            );
        }
        assert!(
            cut_both.iter().any(|p| p.canon == "deutero"),
            "{}: both-canon mode sent no deuterocanonical passage",
            qid
        );
    }
}
