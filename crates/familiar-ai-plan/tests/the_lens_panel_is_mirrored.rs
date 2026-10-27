//! The lens panel is mirrored (the owner's plan of 2026-10-26, lens-panel.md, approved): a record
//! rebuilt from the committed plan carries each lens's answer (`lens_found`), the synthesis's
//! finding with its questions (`cases_found`) and the person's pick on each question
//! (`question_answered`), so a fresh clone, a new lane or CI's verify neither runs the panel again
//! nor asks the person again. Which Agent call got which lens (`lens_released`) is the binary's
//! bookkeeping and stays out of the plan, on read and on write.

mod common;

use common::fields;
use familiar_ai_facts::{append, append_raw, IntentSubmitted, Log};
use familiar_ai_plan::{claims_from_log, from_bytes, to_bytes, Claim, LineError, PlanError, PLAN_KINDS};
use serde_json::{json, Map, Value};

const LENSES: [&str; 5] = ["ambiguity", "completeness", "happy_path", "edge_cases", "order_and_coupling"];

#[test]
fn a_lenss_answer_and_the_persons_pick_are_plan_kinds_and_the_release_is_not() {
    for kind in ["lens_found", "cases_found", "question_answered"] {
        assert!(PLAN_KINDS.contains(&kind), "{kind}: {PLAN_KINDS:?}");
    }
    assert!(!PLAN_KINDS.contains(&"lens_released"));
}

#[test]
fn the_release_is_refused_on_read_and_on_write() {
    let f = fields(&[("criterion_id", json!("c1")), ("lens", json!("ambiguity")), ("tool_use_id", json!("toolu_01"))]);
    match Claim::new("lens_released", f) {
        Err(PlanError::RefusedKind(k)) => assert_eq!(k, "lens_released"),
        other => panic!("expected RefusedKind on write, got {other:?}"),
    }
    let line = "{\"kind\":\"lens_released\",\"criterion_id\":\"c1\",\"lens\":\"ambiguity\",\"tool_use_id\":\"toolu_01\"}\n";
    match from_bytes(line.as_bytes()) {
        Err(PlanError::Line(e)) => assert_eq!(e, LineError::RefusedKind { line: 1, kind: "lens_released".to_string() }),
        other => panic!("expected a refused kind on read, got {other:?}"),
    }
}

#[test]
fn a_synthesis_with_questions_and_a_pick_round_trip_as_lines() {
    let synthesis = json!({ "kind": "cases_found", "criterion_id": "c1", "outcome": "none", "cases": [], "why": "the planned cases cover it", "model": "opus", "tokens": 900, "questions": [{ "question_id": "q1", "text": "Which styles?", "options": ["every style", "only long"], "why": "the ask names none" }] }).to_string();
    let pick = json!({ "kind": "question_answered", "criterion_id": "c1", "question_id": "q1", "choice": "typed", "typed": "only long and short" }).to_string();
    let text = format!("{synthesis}\n{pick}\n");
    let claims = from_bytes(text.as_bytes()).expect("read");
    assert_eq!(claims.iter().map(Claim::line).collect::<Vec<_>>(), [synthesis, pick], "kind first, then the form's order");
    assert_eq!(to_bytes(&claims), text.as_bytes());
}

/// The mirror keeps every lens's answer, the synthesis and the person's picks in log order, every
/// field kept, and leaves the releases out.
#[test]
fn the_mirror_keeps_the_panels_answers_and_the_picks_in_log_order_and_no_release() {
    let (at, tenant) = (1_700_000_000.0, "_default_");
    let fact = |log: &Log, kind: &str, v: Value| append_raw(log, kind, v.as_object().cloned().unwrap_or_default(), at, tenant).unwrap_or_else(|e| panic!("{kind}: {e}"));
    let mut log: Log = append(&Vec::new(), &IntentSubmitted { text: "lock out after three tries".into(), sha: "s0".into(), for_t: None }, at, tenant).unwrap();
    log = fact(&log, "behavior", json!({ "behavior_id": "b1", "text": "the lock" }));
    log = fact(&log, "criterion", json!({ "criterion_id": "c1", "text": "lock out after three tries", "path": "full", "behavior_id": "b1", "promise_id": "p1" }));
    for (i, lens) in LENSES.iter().enumerate() {
        log = fact(&log, "lens_released", json!({ "criterion_id": "c1", "lens": lens, "tool_use_id": format!("toolu_{i}") }));
    }
    for lens in LENSES.iter().rev() {
        log = fact(&log, "lens_found", json!({ "criterion_id": "c1", "lens": lens, "outcome": "none", "findings": [], "model": "opus", "tokens": 100 }));
    }
    log = fact(&log, "lens_released", json!({ "criterion_id": "c1", "lens": "synthesis", "tool_use_id": "toolu_9" }));
    log = fact(&log, "cases_found", json!({ "criterion_id": "c1", "outcome": "none", "cases": [], "why": "covered", "model": "opus", "tokens": 900, "questions": [{ "question_id": "q1", "text": "Which?", "options": ["a", "b"], "why": "w" }, { "question_id": "q2", "text": "And?", "options": ["c", "d"], "why": "w" }] }));
    log = fact(&log, "question_answered", json!({ "criterion_id": "c1", "question_id": "q2", "choice": "d", "typed": "" }));
    log = fact(&log, "question_answered", json!({ "criterion_id": "c1", "question_id": "q1", "choice": "a", "typed": "" }));

    let claims = claims_from_log(&log);
    let read: Vec<String> = claims
        .iter()
        .map(|c| {
            let s = |k: &str| c.fields().get(k).and_then(Value::as_str).unwrap_or("").to_string();
            match c.kind() {
                "lens_found" => format!("lens_found {}", s("lens")),
                "question_answered" => format!("question_answered {} {}", s("question_id"), s("choice")),
                kind => kind.to_string(),
            }
        })
        .collect();
    assert_eq!(
        read,
        [
            "intent_submitted",
            "behavior",
            "criterion",
            "lens_found order_and_coupling",
            "lens_found edge_cases",
            "lens_found happy_path",
            "lens_found completeness",
            "lens_found ambiguity",
            "cases_found",
            "question_answered q2 d",
            "question_answered q1 a",
        ]
    );
    let mirrored: Vec<&Map<String, Value>> = log.iter().filter(|f| matches!(f.kind, "lens_found" | "cases_found" | "question_answered")).map(|f| &f.fields).collect();
    let lines: Vec<&Map<String, Value>> = claims.iter().filter(|c| matches!(c.kind(), "lens_found" | "cases_found" | "question_answered")).map(|c| c.fields()).collect();
    assert_eq!(lines, mirrored, "each mirrored as it was recorded, every field kept");
    assert_eq!(from_bytes(&to_bytes(&claims)).expect("read back"), claims);
}
