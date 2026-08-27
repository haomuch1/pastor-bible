//! The verifier's two parity checks.
//!
//! First the 35 contract vectors of docs/VERIFIER.md, which both
//! implementations must agree on. Then every model output P3 stored, checked
//! against the record Python produced for it: verdict, each violation's kind,
//! text, reason and span, the stripped text, the retry note, and the fallback
//! rendering. The contract vectors are written by hand and therefore test what
//! we thought of; the P3 outputs are real model prose and test what we did not.

mod common;

use pastor_bible_core::index::Index;
use pastor_bible_core::verifier::{Sent, Verifier};

fn sent_from(value: &serde_json::Value) -> Vec<Sent> {
    value
        .as_array()
        .expect("sent set is an array")
        .iter()
        .map(|s| Sent {
            token: s["token"].as_str().unwrap().to_string(),
            reference: s["ref"].as_str().unwrap_or("").to_string(),
            verse_ids: s["verse_ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap())
                .collect(),
        })
        .collect()
}

#[test]
fn contract_vectors_all_hold() {
    let db = common::require_index();
    let index = Index::open(&db).expect("open index");
    let v = Verifier::new(&index);
    let fx = common::read_json(&common::fixtures_dir().join("verifier_vectors.json"));
    let sent = sent_from(&fx["sent"]);
    let vectors = fx["vectors"].as_array().unwrap();
    assert_eq!(vectors.len(), 35, "the contract is 35 vectors");

    let mut failures = Vec::new();
    for case in vectors {
        let n = case["n"].as_i64().unwrap();
        let text = case["text"].as_str().unwrap();
        let expected = case["expected"].as_str().unwrap();
        let (verdict, violations) = v.check(&index, text, &sent);
        if verdict.as_str() != expected {
            failures.push(format!(
                "vector {}: expected {} got {} on {:?} ({:?})",
                n,
                expected,
                verdict.as_str(),
                text,
                violations.iter().map(|x| x.text.clone()).collect::<Vec<_>>()
            ));
            continue;
        }
        // The violation records must match Python's, not merely the verdict.
        let want = case["violations"].as_array().unwrap();
        if want.len() != violations.len() {
            failures.push(format!(
                "vector {}: {} violations in Python, {} here",
                n,
                want.len(),
                violations.len()
            ));
            continue;
        }
        for (w, g) in want.iter().zip(violations.iter()) {
            let want_span =
                (w["span"][0].as_u64().unwrap() as usize, w["span"][1].as_u64().unwrap() as usize);
            if w["kind"].as_str().unwrap() != g.kind.as_str()
                || w["text"].as_str().unwrap() != g.text
                || w["reason"].as_str().unwrap() != g.reason
                || want_span != g.char_span
            {
                failures.push(format!(
                    "vector {}: violation differs. Python {:?}, here {:?}",
                    n,
                    (
                        w["kind"].as_str().unwrap(),
                        w["text"].as_str().unwrap(),
                        w["reason"].as_str().unwrap(),
                        want_span
                    ),
                    (g.kind.as_str(), g.text.as_str(), g.reason.as_str(), g.char_span)
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{} of 35 vectors differ:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn p3_outputs_agree_with_python_row_for_row() {
    let db = common::require_index();
    let index = Index::open(&db).expect("open index");
    let v = Verifier::new(&index);
    let fx = common::read_json(&common::fixtures_dir().join("p3_verifier.json"));
    let rows = fx["rows"].as_array().unwrap();
    assert!(rows.len() >= 80, "expected every P3 output, got {} rows", rows.len());

    let mut failures = Vec::new();
    for row in rows {
        let label =
            format!("{}/{}/{}", row["run"].as_str().unwrap(), row["question_id"].as_str().unwrap(), row["stage"].as_str().unwrap());
        let text = row["text"].as_str().unwrap();
        let sent = sent_from(&row["sent"]);
        let (verdict, violations) = v.check(&index, text, &sent);

        if verdict.as_str() != row["verdict"].as_str().unwrap() {
            failures.push(format!(
                "{}: verdict {} in Python, {} here",
                label,
                row["verdict"].as_str().unwrap(),
                verdict.as_str()
            ));
            continue;
        }
        // Where P3 recorded a verdict at the time, it must still be that.
        if let Some(rec) = row["recorded_verdict"].as_str() {
            if rec != verdict.as_str() {
                failures.push(format!("{}: P3 recorded {}, here {}", label, rec, verdict.as_str()));
            }
        }
        let want = row["violations"].as_array().unwrap();
        if want.len() != violations.len() {
            failures.push(format!(
                "{}: {} violations in Python, {} here",
                label,
                want.len(),
                violations.len()
            ));
            continue;
        }
        for (w, g) in want.iter().zip(violations.iter()) {
            let want_span =
                (w["span"][0].as_u64().unwrap() as usize, w["span"][1].as_u64().unwrap() as usize);
            if w["kind"].as_str().unwrap() != g.kind.as_str()
                || w["text"].as_str().unwrap() != g.text
                || w["reason"].as_str().unwrap() != g.reason
                || want_span != g.char_span
            {
                failures.push(format!("{}: violation record differs: {:?} vs {:?}", label, w, g));
            }
        }
        if Verifier::strip(text, &violations) != row["stripped"].as_str().unwrap() {
            failures.push(format!("{}: stripped text differs", label));
        }
        if Verifier::failure_note(&violations) != row["failure_note"].as_str().unwrap() {
            failures.push(format!("{}: retry failure note differs", label));
        }
        if Verifier::fallback(&index, &sent) != row["fallback"].as_str().unwrap() {
            failures.push(format!("{}: fallback rendering differs", label));
        }
    }
    assert!(
        failures.is_empty(),
        "{} differences over {} P3 outputs:\n{}",
        failures.len(),
        rows.len(),
        failures.iter().take(20).cloned().collect::<Vec<_>>().join("\n")
    );
}
