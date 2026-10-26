//! A tests-first run that passed when it should have failed is a try, not
//! red: two make `two_tries_not_red`, not the fix phase's `two_failures`.
//! A seal whose run passed is no proof, so editing those tests is not
//! `proof_tampered`.

use familiar_ai_facts::Log;
use familiar_ai_rules::sample::{apply_move, criterion_fixture, Move};
use familiar_ai_rules::{lifecycle, moves, terminal, View};
use serde_json::Value;

const AT: f64 = 0.0;
const TENANT: &str = "_default_";

fn step(log: &Log, label: &str, outcome: Option<&str>) -> Log {
    let v = View::new(log);
    let m: Option<Move> = moves::criterion_admissible(&v).into_iter().find(|m| {
        m.label.as_deref() == Some(label) && outcome.is_none_or(|o| m.fields.get("outcome").and_then(Value::as_str) == Some(o))
    });
    match m {
        Some(m) => apply_move(log, &m, AT, TENANT).unwrap_or_default(),
        None => panic!("no {label} move at:\n{}", terminal::status(&v)),
    }
}

/// One tests commit run through by the binary: its test with `outcome`, the
/// touching test passed, the stamp.
fn tests_commit(log: &Log, label: &str, outcome: &str) -> Log {
    let mut log = step(log, label, None);
    if label == "tests_only" {
        log = step(&log, "test_written", None);
    }
    log = step(&log, "pass_please_pass", Some(outcome));
    log = step(&log, "pass_please_pass", Some("passed"));
    step(&log, "fingerprint_stamped", None)
}

fn whys(log: &Log) -> Vec<String> {
    lifecycle::holds(&View::new(log), "c1").iter().map(|h| h.why.to_string()).collect()
}

fn started() -> Log {
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).unwrap_or_default();
    step(&log, "start_check", None)
}

#[test]
fn two_tests_first_runs_that_pass_hold_two_tries_not_red_and_no_proof_tampered() {
    let log = tests_commit(&started(), "tests_only", "passed");
    assert_eq!(lifecycle::tries_not_red(&View::new(&log), "c1"), 1);
    assert_eq!(whys(&log), ["tests_not_red"]);
    let log = tests_commit(&log, "changed_test", "passed");
    assert_eq!(lifecycle::tries_not_red(&View::new(&log), "c1"), 2);
    assert_eq!(lifecycle::attempts(&View::new(&log), "c1"), 0, "no seal: no attempt of the fix phase");
    assert_eq!(whys(&log), ["tests_not_red", "two_tries_not_red"], "an unproven seal changed: no proof_tampered");
}

#[test]
fn a_red_tests_first_run_ends_the_row() {
    let log = tests_commit(&started(), "tests_only", "passed");
    let log = tests_commit(&log, "changed_test", "failed");
    assert_eq!(lifecycle::tries_not_red(&View::new(&log), "c1"), 0, "a seal stands");
    assert_eq!(lifecycle::state(&View::new(&log), "c1"), "sealed");
}

#[test]
fn a_test_changed_away_from_a_red_seal_is_still_proof_tampered() {
    let log = tests_commit(&started(), "tests_only", "failed");
    let log = tests_commit(&log, "changed_test", "passed");
    let w = whys(&log);
    assert!(w.contains(&"proof_tampered".to_string()) && w.contains(&"tests_not_red".to_string()), "{w:?}");
    assert_eq!(lifecycle::tries_not_red(&View::new(&log), "c1"), 1, "counted from the tests commit that ended the red seal");
    assert_eq!(lifecycle::attempts(&View::new(&log), "c1"), 0);
}
