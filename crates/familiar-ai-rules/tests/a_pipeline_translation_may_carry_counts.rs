//! The binary writes `counts` on a pipeline run's translation as it writes its label and its
//! location (lane h1): the action's free fields name it, or CI's `action_append` refuses every
//! translation that carries one.

use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::sample::apply_move;
use familiar_ai_rules::{action, View};
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";

fn raw(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { unreachable!() };
    append_raw(log, kind, m, AT, TENANT).unwrap_or_else(|e| panic!("append {kind}: {e}"))
}

fn reserved_and_ended() -> Log {
    let mut log = raw(&Vec::new(), "intent_submitted", json!({ "text": "greet with hello", "sha": "k0" }));
    log = raw(&log, "criterion", json!({ "criterion_id": "c1", "text": "c1 works", "path": "full" }));
    log = raw(&log, "place_chosen", json!({ "criterion_id": "c1", "places": [{ "file_path": "src/app.sh", "symbol": "f", "line_start": 1, "line_end": 3 }] }));
    log = raw(&log, "test_planned", json!({ "criterion_id": "c1", "file_path": "tests/test_c1.sh", "will": "create", "test_command": "sh tests/test_c1.sh" }));
    log = raw(&log, "test_written", json!({ "criterion_id": "c1", "file_path": "tests/test_c1.sh", "test_name": "" }));
    log = raw(&log, "commit_seen", json!({ "commit": "k1", "files": ["tests/test_c1.sh"], "touches_tests": true, "touches_code": false, "anchor_overlap": [] }));
    log = raw(&log, "tests_observed", json!({ "criterion_id": "c1", "criterion_rev": "R", "commit": "k1", "tests_hash": "Hc1" }));
    log = raw(&log, "pipeline_run_seen", json!({ "head_commit": "k1" }));
    let ms = action::action_admissible(&View::new(&log));
    log = apply_move(&log, &ms[0], AT, TENANT).expect("reserved");
    let ms = action::action_admissible(&View::new(&log));
    let mut end = ms[1].fields.clone();
    end.insert("exit_code".into(), json!(1));
    end.insert("stderr_tail".into(), json!(""));
    end.insert("stdout_tail".into(), json!("no hello\n"));
    action::action_append(&log, "run_ended", &end, AT, TENANT).expect("the binary's end is admissible")
}

#[test]
fn counts_and_a_location_kind_are_the_binarys_own_fields() {
    let log = reserved_and_ended();
    let ms = action::action_admissible(&View::new(&log));
    assert!(ms.iter().all(|m| m.kind == "test_log_translation"), "{ms:?}");
    let mut t = ms[0].fields.clone();
    t.insert("label".into(), json!("tests_failed"));
    t.insert("evidence".into(), json!("FAILED tests/test_c1.sh"));
    t.insert("failing_tests".into(), json!(["tests/test_c1.sh"]));
    t.insert("location".into(), json!({ "file_path": "tests/test_c1.sh", "line": 2, "kind": "assertion" }));
    t.insert("counts".into(), json!({ "passed": 0, "failed": 1 }));
    let log = action::action_append(&log, "test_log_translation", &t, AT, TENANT).expect("a translation carrying counts is admissible");
    let kept = log.iter().rev().find(|f| f.kind == "test_log_translation").expect("written");
    assert_eq!(kept.fields.get("counts"), Some(&json!({ "passed": 0, "failed": 1 })));
}

#[test]
fn counts_free_no_other_field() {
    let log = reserved_and_ended();
    let ms = action::action_admissible(&View::new(&log));
    let mut t = ms[0].fields.clone();
    t.insert("label".into(), json!("tests_failed"));
    t.insert("counts".into(), json!({ "failed": 1 }));
    t.insert("criterion_id".into(), json!("c3"));
    assert!(action::action_append(&log, "test_log_translation", &t, AT, TENANT).is_err(), "only the reserved run's part");
}
