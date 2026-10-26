//! Attempts count by family, not by label (the owner, 2026-10-16; S6 review finding 2): every
//! valid red after the seal counts toward `two_failures`, whatever its label, and a
//! `tests_failed` that reads Test is broken (an exception outside the place) does not.

mod common;

use common::{raw, step, whys, AT, TENANT, TEST, TOUCHING};
use familiar_ai_facts::Log;
use familiar_ai_rules::sample::criterion_fixture;
use familiar_ai_rules::{lifecycle, View};
use serde_json::{json, Value};

fn tests_first() -> Log {
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).unwrap_or_default();
    let log = step(&log, "start_check", None);
    let log = step(&log, "tests_only", None);
    step(&log, "test_written", None)
}

fn head(log: &Log) -> String {
    log.iter().rev().find(|f| f.kind == "commit_seen").and_then(|f| f.fields.get("commit")).and_then(Value::as_str).unwrap_or("").to_string()
}

/// A run at the latest commit as the binary writes it, translated as `label` with `extra`.
fn ran(log: &Log, word: &str, label: &str, extra: Value) -> Log {
    let id = head(log);
    let rid = format!("r-{}", log.len());
    let mut log = raw(log, "run_reserved", json!({"run_id": rid, "criterion_id": "c1", "criterion_rev": "R1", "commit": id, "command": "pytest -q", "resources": ["tests"], "wait": "60", "by": "binary"}));
    log = raw(&log, "run_ended", json!({"run_id": rid, "outcome": word, "end": "exited"}));
    let mut t = json!({"run_id": rid, "criterion_id": "c1", "commit": id, "by": "regex", "label": label, "evidence": "", "failing_tests": [], "classified_on": "full"});
    if let (Value::Object(m), Value::Object(x)) = (&mut t, extra) {
        m.extend(x);
    }
    log = raw(&log, "test_log_translation", t);
    for (fp, o) in [(TEST, word), (TOUCHING, "passed")] {
        log = raw(&log, "pass_please_pass", json!({"criterion_id": "c1", "file_path": fp, "commit": id, "outcome": o, "ran_in": "laptop", "with_anchor_reverted": false, "run_id": rid}));
    }
    step(&log, "fingerprint_stamped", None)
}

fn code(log: &Log, label: &str, extra: Value) -> Log {
    ran(&step(log, "code", None), "failed", label, extra)
}

fn sealed() -> Log {
    ran(&tests_first(), "failed", "tests_failed", json!({}))
}

#[test]
fn every_valid_red_counts_toward_two_failures_whatever_its_label() {
    // the place's symbol missing in the planned test is a valid red (§6.2)
    let missing = || json!({"missing_name": "check_password", "location": {"file_path": TEST, "line": 1}});
    let log = code(&sealed(), "missing_target_symbol", missing());
    let log = code(&log, "tests_failed", json!({}));
    let v = View::new(&log);
    assert_eq!(lifecycle::attempts_of(&v, "c1", "tests_failed"), 2, "both are valid reds");
    assert_eq!(lifecycle::attempts(&v, "c1"), 2);
    assert!(whys(&log).contains(&"two_failures".to_string()), "{:?}", whys(&log));
}

#[test]
fn a_tests_failed_that_reads_test_is_broken_is_no_attempt_toward_two_failures() {
    let outside = || json!({"location": {"file_path": "src/db.py", "line": 3, "function": "def connect():"}});
    let log = code(&sealed(), "tests_failed", outside());
    let log = code(&log, "tests_failed", outside());
    let v = View::new(&log);
    assert_eq!(lifecycle::attempts_of(&v, "c1", "tests_failed"), 0, "an exception outside the place is Test is broken");
    assert_eq!(lifecycle::attempts_of(&v, "c1", "test_broken"), 2, "counted under its family");
    assert!(!whys(&log).contains(&"two_failures".to_string()), "{:?}", whys(&log));
}

#[test]
fn a_collection_error_and_a_setup_error_are_one_family() {
    let log = code(&sealed(), "collection_error", json!({}));
    let log = code(&log, "setup_error", json!({}));
    let v = View::new(&log);
    assert_eq!(lifecycle::attempts_of(&v, "c1", "test_broken"), 2);
    assert_eq!(lifecycle::attempt_key(&v, "c1", log.iter().rev().find(|f| f.kind == "pass_please_pass" && f.fields["file_path"] == TEST).expect("a result")), Some("test_broken"));
}
