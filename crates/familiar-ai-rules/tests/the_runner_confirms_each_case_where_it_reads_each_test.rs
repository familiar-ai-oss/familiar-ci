//! The runner confirms the contract where it reads each test (the owner's plan of 2026-10-21,
//! step 1) [draft names]: when the judge's latest word on the seal names, for a case of the
//! promise, the test that exercises it, and a run's translation is Familiar's reporter's with its
//! per-test lists, a normal case's test must be among the failing at the seal run, and every
//! named test must have run and not failed at the green run. An opposite or edge case may pass at
//! the seal: what stays the same passes before the fix too. A case the judge named no test for
//! keeps green away. A name the run's lists do not hold, a run read without per-test lists, or a
//! word with no cases: the judge's mapping stands alone, as before.

mod common;

use common::{raw, step, AT, TENANT, TEST, TOUCHING};
use familiar_ai_facts::Log;
use familiar_ai_rules::criterion::{contract_unmet, test_id_matches};
use familiar_ai_rules::sample::criterion_fixture;
use familiar_ai_rules::{lifecycle, View};
use serde_json::{json, Value};

const PRESENT: &str = "tests/test_c1.py::test_present";
const ABSENT: &str = "tests/test_c1.py::test_absent";

fn planned() -> Log {
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).unwrap_or_default();
    let cases = json!([
        {"case_id": "n1", "kind": "normal", "given": "the trigger present", "expect": "the new result"},
        {"case_id": "o1", "kind": "opposite", "given": "the trigger absent", "expect": "the result as before"}
    ]);
    raw(&log, "promise", json!({"promise_id": "p1", "behavior_id": "b1", "accepts": "a user name and a password", "gives_back": "a session", "refuses": "a wrong password", "cases": cases}))
}

fn head(log: &Log) -> String {
    log.iter().rev().find(|f| f.kind == "commit_seen").and_then(|f| f.fields.get("commit")).and_then(Value::as_str).unwrap_or("").to_string()
}

/// A run at the latest commit, translated as `label` by `by` with `extra`; the own test's result
/// names the run, the touching test passed, the stamp.
fn ran(log: &Log, word: &str, label: &str, by: &str, extra: Value) -> Log {
    let id = head(log);
    let rid = format!("r-{}", log.len());
    let mut log = raw(log, "run_reserved", json!({"run_id": rid, "criterion_id": "c1", "criterion_rev": "R1", "commit": id, "command": "pytest -q", "resources": ["tests"], "wait": "60", "by": "binary"}));
    log = raw(&log, "run_ended", json!({"run_id": rid, "outcome": word, "end": "exited"}));
    let mut t = json!({"run_id": rid, "criterion_id": "c1", "commit": id, "by": by, "label": label, "evidence": "", "failing_tests": [], "classified_on": "full"});
    if let (Value::Object(m), Value::Object(x)) = (&mut t, extra) {
        m.extend(x);
    }
    log = raw(&log, "test_log_translation", t);
    for (fp, o) in [(TEST, word), (TOUCHING, "passed")] {
        log = raw(&log, "pass_please_pass", json!({"criterion_id": "c1", "file_path": fp, "commit": id, "outcome": o, "ran_in": "laptop", "with_anchor_reverted": false, "run_id": rid}));
    }
    step(&log, "fingerprint_stamped", None)
}

/// c1 sealed by a run read with per-test lists (`by`), `failing` among `ran`, `skipped` beside.
fn sealed(by: &str, failing: &[&str], ran_tests: &[&str], skipped: &[&str]) -> Log {
    let log = step(&planned(), "start_check", None);
    let log = step(&log, "tests_only", None);
    let log = step(&log, "test_written", None);
    let counts = json!({"failed": failing.len(), "passed": ran_tests.len() - failing.len(), "skipped": skipped.len()});
    let extra = json!({"location": {"file_path": TEST, "line": 7, "kind": "assertion"}, "counts": counts, "failing_tests": failing, "ran_tests": ran_tests, "skipped_tests": skipped});
    let log = ran(&log, "failed", "tests_failed", by, extra);
    assert!(familiar_ai_rules::criterion::sealed_at(&View::new(&log), "c1", familiar_ai_rules::criterion::Place::Laptop).is_some(), "sealed");
    log
}

/// The judge's word on the seal, naming a test for each case given.
fn judged(log: &Log, cases: &[(&str, &str, bool)]) -> Log {
    let cases: Vec<Value> = cases.iter().map(|(id, test, ok)| json!({"case_id": id, "test": test, "ok": ok})).collect();
    raw(log, "judge_verdict", json!({"criterion_id": "c1", "model": "claude-opus-4-6", "answers_intent": "yes", "reason": "r", "cases": cases}))
}

/// The code, then a passing run read by `by` with `ran_tests` and `skipped`.
fn fixed(log: &Log, by: &str, ran_tests: &[&str], skipped: &[&str]) -> Log {
    let counts = json!({"passed": ran_tests.len(), "skipped": skipped.len()});
    ran(&step(log, "code", None), "passed", "passed", by, json!({"counts": counts, "ran_tests": ran_tests, "skipped_tests": skipped}))
}

fn state(log: &Log) -> &'static str {
    lifecycle::state(&View::new(log), "c1")
}

#[test]
fn a_judges_name_matches_a_runners_id_by_its_last_segments() {
    for (name, id) in [
        ("test_present", "tests/test_c1.py::test_present"),
        ("test_present()", "tests/test_c1.py::test_present"),
        ("TestGreet::test_present", "tests/test_c1.py::TestGreet::test_present"),
        ("TestGreet.test_present", "tests/test_c1.py::TestGreet::test_present"),
        ("test_present", "tests/test_c1.py::test_present[one]"),
        ("test_present", "tests.test_c1.TestGreet.test_present"),
        ("tests/test_c1.py::test_present", "tests/test_c1.py::test_present"),
        ("TestPresent", "TestPresent"),
    ] {
        assert!(test_id_matches(name, id), "{name} ~ {id}");
    }
    for (name, id) in [
        ("test_present", "tests/test_c1.py::test_present_too"),
        ("present", "tests/test_c1.py::test_present"),
        ("TestOther::test_present", "tests/test_c1.py::TestGreet::test_present"),
        ("", "tests/test_c1.py::test_present"),
        ("none", "tests/test_c1.py::none_of_these"),
    ] {
        assert!(!test_id_matches(name, id), "{name} !~ {id}");
    }
}

#[test]
fn the_normal_cases_test_failing_at_the_seal_and_both_passing_at_green_is_green() {
    let log = judged(&sealed("reporter", &[PRESENT], &[PRESENT, ABSENT], &[]), &[("n1", "test_present", true), ("o1", "test_absent", true)]);
    assert_eq!(contract_unmet(&View::new(&log), "c1"), None, "the opposite case passes before the fix, as it should");
    let log = fixed(&log, "reporter", &[PRESENT, ABSENT], &[]);
    assert_eq!(state(&log), "green");
    assert_eq!(contract_unmet(&View::new(&log), "c1"), None);
}

#[test]
fn a_normal_cases_test_that_passed_at_the_seal_keeps_green_away() {
    let log = judged(&sealed("reporter", &[ABSENT], &[PRESENT, ABSENT], &[]), &[("n1", "test_present", true), ("o1", "test_absent", true)]);
    assert_eq!(
        contract_unmet(&View::new(&log), "c1").as_deref(),
        Some("case n1 (normal): the judge named test_present, but it did not fail at the seal run; a normal case's test fails before the fix")
    );
    let log = fixed(&log, "reporter", &[PRESENT, ABSENT], &[]);
    assert_eq!(state(&log), "sealed", "the run passed, but the case's test never went red");
}

#[test]
fn a_named_test_that_did_not_run_at_green_keeps_green_away() {
    // the opposite case's test is skipped at the seal and at green: the counts allow it, the contract does not
    let log = judged(&sealed("reporter", &[PRESENT], &[PRESENT], &[ABSENT]), &[("n1", "test_present", true), ("o1", "test_absent", true)]);
    assert_eq!(contract_unmet(&View::new(&log), "c1"), None, "an opposite case need not fail at the seal");
    let log = fixed(&log, "reporter", &[PRESENT], &[ABSENT]);
    assert_eq!(state(&log), "sealed");
    let unmet = contract_unmet(&View::new(&log), "c1").unwrap_or_default();
    assert!(unmet.starts_with("case o1 (opposite): the judge named test_absent, but it did not run and pass at "), "{unmet}");
    // run again with it running: green
    let log = fixed(&log, "reporter", &[PRESENT, ABSENT], &[]);
    assert_eq!(state(&log), "green");
}

#[test]
fn a_case_the_judge_named_no_test_for_keeps_green_away() {
    let log = judged(&sealed("reporter", &[PRESENT], &[PRESENT, ABSENT], &[]), &[("n1", "test_present", true), ("o1", "none", false)]);
    assert_eq!(contract_unmet(&View::new(&log), "c1").as_deref(), Some("case o1 (opposite): no test exercises it, as the judge read the tests"));
    assert_eq!(state(&fixed(&log, "reporter", &[PRESENT, ABSENT], &[])), "sealed");
    let left_out = judged(&sealed("reporter", &[PRESENT], &[PRESENT, ABSENT], &[]), &[("n1", "test_present", true)]);
    assert!(contract_unmet(&View::new(&left_out), "c1").is_some_and(|w| w.starts_with("case o1 (opposite): no test")), "a case the word left out");
}

#[test]
fn without_per_test_lists_or_cases_the_judges_mapping_stands_alone() {
    // read by the tables: no per-test lists, the mapping stands
    let log = judged(&sealed("regex", &[ABSENT], &[PRESENT, ABSENT], &[]), &[("n1", "test_present", true), ("o1", "test_absent", true)]);
    assert_eq!(contract_unmet(&View::new(&log), "c1"), None);
    assert_eq!(state(&fixed(&log, "regex", &[PRESENT, ABSENT], &[])), "green");
    // a name the run's lists do not hold
    let log = judged(&sealed("reporter", &[PRESENT], &[PRESENT, ABSENT], &[]), &[("n1", "it shows the new result", true), ("o1", "test_absent", true)]);
    assert_eq!(contract_unmet(&View::new(&log), "c1"), None);
    assert_eq!(state(&fixed(&log, "reporter", &[PRESENT, ABSENT], &[])), "green");
    // a word with no cases, and no word at all
    let log = raw(&sealed("reporter", &[ABSENT], &[PRESENT, ABSENT], &[]), "judge_verdict", json!({"criterion_id": "c1", "model": "m", "answers_intent": "yes", "reason": "r"}));
    assert_eq!(state(&fixed(&log, "reporter", &[PRESENT, ABSENT], &[])), "green");
    assert_eq!(state(&fixed(&sealed("reporter", &[ABSENT], &[PRESENT, ABSENT], &[]), "reporter", &[PRESENT, ABSENT], &[])), "green");
}
