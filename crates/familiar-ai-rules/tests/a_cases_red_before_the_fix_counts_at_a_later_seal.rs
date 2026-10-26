//! Red once counts (the owner's ruling, 2026-10-25) [draft names]: after the part's first fix
//! commit, a test change committed alone makes a new tests-first commit, and its seal run is on
//! code that already holds the fix. A normal case's named test that failed at an earlier seal run
//! of the part, at a tests-first commit before that fix commit, counts there as having failed
//! first. Only a named test that never failed at such a run must fail at the latest seal run: one
//! written after the fix, one that passed at every seal run before the fix, one whose only red
//! came at a seal run after it. A seal run before any fix is its own witness, as before [draft
//! reading]: a test change that leaves the case's test passing there is refused, and the fix
//! landing after it does not change that.
//!
//! The bench's pytest-13575 at 23b964c2 is the shape: red at the seal, the fix, a test added
//! alone and red, the case's test green at the new seal; "did not fail at the seal run" refused
//! every edit of the code until the budget was gone.

mod common;

use common::{raw, step, AT, TENANT, TEST, TOUCHING};
use familiar_ai_facts::Log;
use familiar_ai_rules::criterion::{contract_unmet_at_seal, sealed_at, tests_first_commit, Place};
use familiar_ai_rules::sample::criterion_fixture;
use familiar_ai_rules::{lifecycle, View};
use serde_json::{json, Value};

/// Case n1's test, then o1's.
const PRESENT: &str = "tests/test_c1.py::test_present";
const ABSENT: &str = "tests/test_c1.py::test_absent";
/// Tests the change after the fix adds: one red on the fixed code, one green there.
const ADDED: &str = "tests/test_c1.py::test_added";
const LATER: &str = "tests/test_c1.py::test_later";

const REFUSED: &str = "case n1 (normal): the judge named test_present, but it did not fail at the seal run; a normal case's test fails before the fix";

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

/// A run of c1's own test at the latest commit, read by Familiar's reporter: `failing` among
/// `ran_tests`; the touching test passed; the stamp.
fn ran(log: &Log, failing: &[&str], ran_tests: &[&str]) -> Log {
    let id = head(log);
    let rid = format!("r-{}", log.len());
    let (word, label) = if failing.is_empty() { ("passed", "passed") } else { ("failed", "tests_failed") };
    let mut log = raw(log, "run_reserved", json!({"run_id": rid, "criterion_id": "c1", "criterion_rev": "R1", "commit": id, "command": "pytest -q", "resources": ["tests"], "wait": "60", "by": "binary"}));
    log = raw(&log, "run_ended", json!({"run_id": rid, "outcome": word, "end": "exited"}));
    let counts = json!({"failed": failing.len(), "passed": ran_tests.len() - failing.len(), "skipped": 0});
    let mut t = json!({"run_id": rid, "criterion_id": "c1", "commit": id, "by": "reporter", "label": label, "evidence": "", "failing_tests": failing, "ran_tests": ran_tests, "skipped_tests": [], "counts": counts, "classified_on": "full"});
    if !failing.is_empty() {
        t["location"] = json!({"file_path": TEST, "line": 7, "kind": "assertion"});
    }
    log = raw(&log, "test_log_translation", t);
    for (fp, o) in [(TEST, word), (TOUCHING, "passed")] {
        log = raw(&log, "pass_please_pass", json!({"criterion_id": "c1", "file_path": fp, "commit": id, "outcome": o, "ran_in": "laptop", "with_anchor_reverted": false, "run_id": rid}));
    }
    step(&log, "fingerprint_stamped", None)
}

/// The judge's word on the latest seal, naming n1's test and o1's.
fn judged(log: &Log, n1: &str) -> Log {
    let cases = json!([{"case_id": "n1", "test": n1, "ok": true}, {"case_id": "o1", "test": "test_absent", "ok": true}]);
    raw(log, "judge_verdict", json!({"criterion_id": "c1", "model": "claude-opus-4-6", "answers_intent": "yes", "reason": "r", "cases": cases}))
}

/// c1 sealed at its tests-first commit by a run failing `failing` of n1's test and o1's, and the
/// judge's word on it.
fn sealed(failing: &[&str]) -> Log {
    let log = step(&planned(), "start_check", None);
    let log = step(&log, "tests_only", None);
    let log = step(&log, "test_written", None);
    let log = ran(&log, failing, &[PRESENT, ABSENT]);
    assert!(sealed_at(&View::new(&log), "c1", Place::Laptop).is_some(), "sealed");
    judged(&log, "test_present")
}

/// The code on the anchor, then a run passing every test in `ran_tests`.
fn fixed(log: &Log, ran_tests: &[&str]) -> Log {
    ran(&step(log, "code", None), &[], ran_tests)
}

/// The test change announced and committed alone (`commit` the move's when `None`, else a commit
/// of tests hash `hash` by hand), its run failing `failing` among `ran_tests`, a new seal there,
/// and the judge's word on it naming `n1` for case n1.
fn resealed(log: &Log, by_hand: Option<(&str, &str)>, failing: &[&str], ran_tests: &[&str], n1: &str) -> Log {
    let before = tests_first_commit(&View::new(log), "c1").map(|t| t.key);
    let mut log = raw(log, "test_change_announced", json!({"criterion_id": "c1", "reason": "adding a case"}));
    log = match by_hand {
        None => step(&log, "changed_test", None),
        Some((id, hash)) => {
            let log = raw(&log, "commit_seen", json!({"commit": id, "files": [TEST], "touches_tests": true, "touches_code": false, "anchor_overlap": []}));
            raw(&log, "tests_observed", json!({"criterion_id": "c1", "criterion_rev": "R1", "commit": id, "tests_hash": hash}))
        }
    };
    let log = ran(&log, failing, ran_tests);
    let v = View::new(&log);
    assert!(tests_first_commit(&v, "c1").is_some_and(|t| Some(&t.key) != before.as_ref()), "a new tests-first commit");
    assert!(sealed_at(&v, "c1", Place::Laptop).is_some(), "a new seal there");
    judged(&log, n1)
}

fn unmet(log: &Log) -> Option<String> {
    contract_unmet_at_seal(&View::new(log), "c1")
}

fn state(log: &Log) -> &'static str {
    lifecycle::state(&View::new(log), "c1")
}

#[test]
fn a_cases_test_red_before_the_fix_counts_at_the_seal_after_it() {
    // pytest-13575: n1's test red at the seal, the fix, a test added alone and red on the fixed
    // code, n1's test green at the new seal
    let log = fixed(&sealed(&[PRESENT]), &[PRESENT, ABSENT]);
    assert_eq!(state(&log), "green");
    let log = resealed(&log, None, &[ADDED], &[PRESENT, ABSENT, ADDED], "test_present");
    assert_eq!(unmet(&log), None, "n1's test failed first, at the seal run before the fix");
    // the added test's own fix, and the part is green again
    let log = fixed(&log, &[PRESENT, ABSENT, ADDED]);
    assert_eq!(state(&log), "green");
    assert_eq!(familiar_ai_rules::criterion::contract_unmet(&View::new(&log), "c1"), None);
}

#[test]
fn a_test_written_after_the_fix_and_green_at_the_new_seal_is_refused() {
    let log = fixed(&sealed(&[PRESENT]), &[PRESENT, ABSENT]);
    let log = resealed(&log, None, &[ADDED], &[PRESENT, ABSENT, ADDED, LATER], "test_later");
    assert_eq!(
        unmet(&log).as_deref(),
        Some("case n1 (normal): the judge named test_later, but it did not fail at the seal run; a normal case's test fails before the fix"),
        "test_later never ran before the fix"
    );
    assert_eq!(state(&fixed(&log, &[PRESENT, ABSENT, ADDED, LATER])), "sealed");
}

#[test]
fn a_test_that_never_failed_at_a_seal_before_the_fix_is_refused() {
    // n1's test passed at the seal before the fix (only o1's failed), and passes at the new seal
    let log = fixed(&sealed(&[ABSENT]), &[PRESENT, ABSENT]);
    assert_eq!(unmet(&log).as_deref(), Some(REFUSED), "refused at the first seal, as before");
    let log = resealed(&log, None, &[ADDED], &[PRESENT, ABSENT, ADDED], "test_present");
    assert_eq!(unmet(&log).as_deref(), Some(REFUSED));
    assert_eq!(state(&fixed(&log, &[PRESENT, ABSENT, ADDED])), "sealed");
}

#[test]
fn a_red_at_a_seal_after_the_fix_does_not_count() {
    // n1's test passed before the fix; changed after it, it failed at a seal on the fixed code;
    // changed again, it passes at the latest seal: its only red is not before the fix
    let log = fixed(&sealed(&[ABSENT]), &[PRESENT, ABSENT]);
    let log = resealed(&log, None, &[PRESENT, ADDED], &[PRESENT, ABSENT, ADDED], "test_present");
    assert_eq!(unmet(&log), None, "n1's test fails at this seal run");
    let log = resealed(&log, Some(("e3", "S1y")), &[ADDED], &[PRESENT, ABSENT, ADDED], "test_present");
    assert_eq!(unmet(&log).as_deref(), Some(REFUSED));
}

#[test]
fn a_seal_run_before_any_fix_is_its_own_witness() {
    // the test change comes before the fix: n1's test, red at the first seal, passes at the new
    // seal on code without the fix, so it does not fail before the fix
    let log = resealed(&sealed(&[PRESENT]), None, &[ADDED], &[PRESENT, ABSENT, ADDED], "test_present");
    assert_eq!(unmet(&log).as_deref(), Some(REFUSED));
    let log = fixed(&log, &[PRESENT, ABSENT, ADDED]);
    assert_eq!(unmet(&log).as_deref(), Some(REFUSED), "the fix landing after the new seal changes nothing");
    assert_eq!(state(&log), "sealed");
}
