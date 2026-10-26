//! Confirmed once per part (the owner, 2026-10-25) [draft names]: once the person has confirmed
//! "it's the bug" (its_the_red) on a part's can't-tell run, a later can't-tell run of that part's
//! test at the tests-first commit that fails with the same kind of error (the same exception name,
//! read from the deciding line, and the same raising place, read from the run's location) counts
//! as confirmed too: its card does not open and it seals as the confirmed run did. Before the seal
//! only, never an its_the_pass, never across parts. The reading itself stays can't-tell.

mod common;

use common::{raw, step, whys, AT, TENANT, TEST, TOUCHING};
use familiar_ai_facts::Log;
use familiar_ai_rules::criterion::{sealed_at, Place};
use familiar_ai_rules::outcome::{result_family, Family};
use familiar_ai_rules::sample::criterion_fixture;
use familiar_ai_rules::{lifecycle, terminal, View};
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

fn last_run(log: &Log) -> String {
    log.iter().rev().find(|f| f.kind == "run_reserved").and_then(|f| f.fields.get("run_id")).and_then(Value::as_str).unwrap_or("").to_string()
}

/// A run of `c` at the latest commit, failed and read can't-tell, its translation carrying `extra`.
fn run_of(log: &Log, c: &str, extra: Value) -> Log {
    let id = head(log);
    let rid = format!("r-{}", log.len());
    let mut log = raw(log, "run_reserved", json!({"run_id": rid, "criterion_id": c, "criterion_rev": "R1", "commit": id, "command": "pytest -q", "resources": ["tests"], "wait": "60", "by": "binary"}));
    log = raw(&log, "run_ended", json!({"run_id": rid, "outcome": "failed", "end": "exited"}));
    let mut t = json!({"run_id": rid, "criterion_id": c, "commit": id, "by": "regex", "label": "unclassified", "evidence": "boom", "failing_tests": [], "classified_on": "full"});
    if let (Value::Object(m), Value::Object(x)) = (&mut t, extra) {
        m.extend(x);
    }
    log = raw(&log, "test_log_translation", t);
    for (fp, o) in [(TEST, "failed"), (TOUCHING, "passed")] {
        log = raw(&log, "pass_please_pass", json!({"criterion_id": c, "file_path": fp, "commit": id, "outcome": o, "ran_in": "laptop", "with_anchor_reverted": false, "run_id": rid}));
    }
    log
}

/// c1's run, stamped as the binary stamps it.
fn ran(log: &Log, extra: Value) -> Log {
    step(&run_of(log, "c1", extra), "fingerprint_stamped", None)
}

fn confirm(log: &Log, c: &str, choice: &str, run_id: &str) -> Log {
    raw(log, "hold_answered", json!({"criterion_id": c, "hold": "run_unclassified", "choice": choice, "typed": "", "run_id": run_id}))
}

fn has(log: &Log, why: &str) -> bool {
    whys(log).iter().any(|w| w == why)
}

fn card_open(log: &Log) -> bool {
    terminal::cards(&View::new(log)).iter().any(|c| c.hold == "run_unclassified")
}

fn own_result(log: &Log) -> familiar_ai_facts::Fact {
    log.iter().rev().find(|f| f.kind == "pass_please_pass" && f.fields.get("file_path").and_then(Value::as_str) == Some(TEST)).cloned().expect("a result")
}

fn error(evidence: &str, location: Value) -> Value {
    json!({"evidence": evidence, "failing_tests": ["tests/test_c1.py::test_login"], "location": location})
}

const INDEX: &str = "FAILED tests/test_c1.py::test_login - IndexError: list index out of range";

fn at_login() -> Value {
    json!({"file_path": "src/login.py", "line": 50, "function": "check_password", "kind": "exception"})
}

/// The person's its_the_red on the tests-first run failing with `first`, then the test changed and
/// committed tests-only again: the log before the new tests-first commit's run.
fn confirmed_then_changed(first: Value) -> Log {
    let log = ran(&tests_first(), first);
    assert!(has(&log, "run_unclassified"), "{:?}", whys(&log));
    let log = confirm(&log, "c1", "its_the_red", &last_run(&log));
    assert!(sealed_at(&View::new(&log), "c1", Place::Laptop).is_some(), "sealed on the confirmed run");
    assert!(!has(&log, "run_unclassified"));
    step(&log, "changed_test", None)
}

#[test]
fn the_same_error_at_the_same_place_at_the_new_tests_first_commit_is_confirmed_and_seals() {
    let log = ran(&confirmed_then_changed(error(INDEX, at_login())), error("FAILED tests/test_c1.py::test_login - IndexError: tuple index out of range", at_login()));
    assert!(!has(&log, "run_unclassified"), "no card: {:?}", whys(&log));
    assert!(!card_open(&log), "no card");
    let v = View::new(&log);
    let seal = sealed_at(&v, "c1", Place::Laptop).expect("sealed as the confirmed run did");
    assert_eq!(seal.fields.get("commit").and_then(Value::as_str), Some(head(&log).as_str()), "sealed at the new tests-first commit");
    assert!(!has(&log, "proof_tampered"), "{:?}", whys(&log));
    assert_eq!(lifecycle::state(&v, "c1"), "sealed");
    // the reading itself stays can't-tell: the teeth read it as strictly as before
    assert_eq!(result_family(&v, "c1", &own_result(&log)), Family::Unknown);
}

#[test]
fn with_no_function_the_place_is_the_file_and_line() {
    let at = |line: u64| json!({"file_path": "src/login.py", "line": line, "kind": "exception"});
    let log = ran(&confirmed_then_changed(error(INDEX, at(50))), error(INDEX, at(50)));
    assert!(!has(&log, "run_unclassified"), "same file and line: {:?}", whys(&log));
    assert!(sealed_at(&View::new(&log), "c1", Place::Laptop).is_some());
    let log = ran(&confirmed_then_changed(error(INDEX, at(50))), error(INDEX, at(51)));
    assert!(has(&log, "run_unclassified"), "another line: {:?}", whys(&log));
}

#[test]
fn another_exception_or_another_place_or_a_missing_one_asks_again() {
    let other_place = json!({"file_path": "src/login.py", "line": 50, "function": "other", "kind": "exception"});
    let other_file = json!({"file_path": "src/util.py", "line": 50, "function": "check_password", "kind": "exception"});
    let cases = [
        ("another exception", error(INDEX, at_login()), error("FAILED tests/test_c1.py::test_login - KeyError: 'id'", at_login())),
        ("another function", error(INDEX, at_login()), error(INDEX, other_place)),
        ("another file", error(INDEX, at_login()), error(INDEX, other_file)),
        ("no exception now", error(INDEX, at_login()), error("FAILED tests/test_c1.py::test_login - boom", at_login())),
        ("no place now", error(INDEX, at_login()), json!({"evidence": INDEX})),
        ("no exception then", error("boom", at_login()), error(INDEX, at_login())),
        ("no place then", json!({"evidence": INDEX}), error(INDEX, at_login())),
    ];
    for (name, first, then) in cases {
        let log = ran(&confirmed_then_changed(first), then);
        assert!(has(&log, "run_unclassified"), "{name}: {:?}", whys(&log));
        assert!(card_open(&log), "{name}: the card asks");
    }
}

#[test]
fn with_no_confirmation_before_the_card_asks_as_today() {
    let log = ran(&tests_first(), error(INDEX, at_login()));
    assert!(has(&log, "run_unclassified"));
    // the person's try_again is no confirmation: the same error run again still asks
    let log = raw(&log, "hold_answered", json!({"criterion_id": "c1", "hold": "run_unclassified", "choice": "try_again", "typed": ""}));
    assert!(!has(&log, "run_unclassified"));
    // the re-run at the same commit (already stamped there)
    let log = run_of(&log, "c1", error(INDEX, at_login()));
    assert!(has(&log, "run_unclassified"), "{:?}", whys(&log));
    assert!(sealed_at(&View::new(&log), "c1", Place::Laptop).is_none());
}

#[test]
fn after_the_seal_nothing_is_inherited_and_no_pass_is_ever_carried() {
    let log = ran(&tests_first(), error(INDEX, at_login()));
    let log = confirm(&log, "c1", "its_the_red", &last_run(&log));
    assert!(sealed_at(&View::new(&log), "c1", Place::Laptop).is_some());
    // a run at a later code commit failing the same way: the card asks, offering its_the_pass
    let log = ran(&step(&log, "code", None), error(INDEX, at_login()));
    assert!(has(&log, "run_unclassified"), "{:?}", whys(&log));
    let v = View::new(&log);
    let card = terminal::cards(&v).into_iter().find(|c| c.hold == "run_unclassified").expect("the card");
    let choices: Vec<String> = card.choices.into_iter().map(|(ch, _)| ch).collect();
    assert_eq!(choices, ["try_again", "its_the_pass", "re_plan", "release"]);
    assert_ne!(lifecycle::state(&v, "c1"), "green");
}

#[test]
fn another_parts_confirmation_does_not_count() {
    // c2's own can't-tell run, failing the same way, confirmed by the person on c2's card
    let log = run_of(&tests_first(), "c2", error(INDEX, at_login()));
    let log = confirm(&log, "c2", "its_the_red", &last_run(&log));
    let log = ran(&log, error(INDEX, at_login()));
    assert!(has(&log, "run_unclassified"), "{:?}", whys(&log));
    assert!(sealed_at(&View::new(&log), "c1", Place::Laptop).is_none());
}
