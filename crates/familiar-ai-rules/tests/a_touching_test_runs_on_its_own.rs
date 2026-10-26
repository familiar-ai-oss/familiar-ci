//! The touching tests run on their own (the owner's plan of 2026-10-21, step 0): the part's own
//! run is its own test files only, so H6's seal comparison reads the counts of what ran at the
//! seal; the touching tests (declared and derived) have a run of their own, and a touching result
//! is read without a seal run: green when its run passed with nothing failed, a skip or a
//! deselect beside it allowed (a touching file's skips are the project's, never the part's to
//! explain); a run of them that failed a test is a failure. A skip there never makes the part
//! can't-tell.

mod common;

use common::{raw, step, whys, AT, TENANT, TEST, TOUCHING};
use familiar_ai_facts::Log;
use familiar_ai_rules::outcome::{self, Family};
use familiar_ai_rules::sample::criterion_fixture;
use familiar_ai_rules::{criterion, lifecycle, View};
use serde_json::{json, Value};

const OWN_COMMAND: &str = "python -m pytest tests/test_c1.py";
const TOUCHING_COMMAND: &str = "python -m pytest tests/test_login_old.py";

fn tests_first() -> Log {
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).unwrap_or_default();
    let log = step(&log, "start_check", None);
    let log = step(&log, "tests_only", None);
    step(&log, "test_written", None)
}

fn head(log: &Log) -> String {
    log.iter().rev().find(|f| f.kind == "commit_seen").and_then(|f| f.fields.get("commit")).and_then(Value::as_str).unwrap_or("").to_string()
}

/// One run at the latest commit as the binary writes it: reserved with `command`, ended with
/// `word`, translated as `label` with `extra`, and a result for each of `files` naming it.
fn one_run(log: &Log, command: &str, files: &[&str], word: &str, label: &str, extra: Value) -> Log {
    let id = head(log);
    let rid = format!("r-{}", log.len());
    let mut log = raw(log, "run_reserved", json!({"run_id": rid, "criterion_id": "c1", "criterion_rev": "R1", "commit": id, "command": command, "resources": ["tests"], "wait": "60", "by": "binary"}));
    log = raw(&log, "run_ended", json!({"run_id": rid, "outcome": word, "end": "exited"}));
    let mut t = json!({"run_id": rid, "criterion_id": "c1", "commit": id, "by": "regex", "label": label, "evidence": "", "failing_tests": [], "classified_on": "full"});
    if let (Value::Object(m), Value::Object(x)) = (&mut t, extra) {
        m.extend(x);
    }
    log = raw(&log, "test_log_translation", t);
    for fp in files {
        log = raw(&log, "pass_please_pass", json!({"criterion_id": "c1", "file_path": fp, "commit": id, "outcome": word, "ran_in": "laptop", "with_anchor_reverted": false, "run_id": rid}));
    }
    log
}

/// The part's own run, then the touching run, then the stamp.
fn both(log: &Log, own: (&str, &str, Value), touching: (&str, &str, Value)) -> Log {
    let log = one_run(log, OWN_COMMAND, &[TEST], own.0, own.1, own.2);
    let log = one_run(&log, TOUCHING_COMMAND, &[TOUCHING], touching.0, touching.1, touching.2);
    step(&log, "fingerprint_stamped", None)
}

fn passed(counts: Value) -> (&'static str, &'static str, Value) {
    ("passed", "passed", json!({"counts": counts}))
}

/// The seal: the own run an honest red in the planned test, the touching tests passing beside a
/// skip of their own.
fn sealed() -> Log {
    let red = ("failed", "tests_failed", json!({"location": {"file_path": TEST, "line": 7, "kind": "assertion"}, "counts": {"passed": 1, "failed": 1}}));
    let log = both(&tests_first(), red, passed(json!({"passed": 4, "skipped": 1})));
    assert!(criterion::sealed_at(&View::new(&log), "c1", criterion::Place::Laptop).is_some(), "sealed");
    log
}

/// The seal with the touching run as it ran at the tests-first commit: the own run an honest
/// red in the planned test, the touching run `base`.
fn sealed_touching(base: (&str, &str, Value)) -> Log {
    let red = ("failed", "tests_failed", json!({"location": {"file_path": TEST, "line": 7, "kind": "assertion"}, "counts": {"passed": 1, "failed": 1}}));
    let log = both(&tests_first(), red, base);
    assert!(criterion::sealed_at(&View::new(&log), "c1", criterion::Place::Laptop).is_some(), "sealed");
    log
}

/// The touching file's failed run, the run's per-test ids.
fn failed(ids: Vec<String>) -> (&'static str, &'static str, Value) {
    (
        "failed",
        "tests_failed",
        json!({"location": {"file_path": TOUCHING, "line": 12, "kind": "assertion"}, "counts": {"passed": 3, "failed": 1}, "failing_tests": ids}),
    )
}

fn result<'a>(log: &'a Log, file: &str) -> &'a familiar_ai_facts::Fact {
    log.iter().rev().find(|f| f.kind == "pass_please_pass" && f.fields["file_path"] == file).expect("the result")
}

#[test]
fn a_skip_in_a_touching_file_leaves_the_part_green() {
    let log = both(&step(&sealed(), "code", None), passed(json!({"passed": 2})), passed(json!({"passed": 4, "skipped": 1})));
    let v = View::new(&log);
    assert_eq!(outcome::result_family(&v, "c1", result(&log, TEST)), Family::Green, "the own run against the seal run's counts");
    assert_eq!(outcome::result_family(&v, "c1", result(&log, TOUCHING)), Family::Green, "a touching pass with a skip beside it");
    assert!(!whys(&log).contains(&"run_unclassified".to_string()), "{:?}", whys(&log));
    assert_eq!(lifecycle::state(&v, "c1"), "green");
}

#[test]
fn a_touching_run_counts_never_reach_the_own_runs_seal_comparison() {
    // more skipped and fewer passed in the touching run than in the seal run: the own reading
    // reads the own run alone
    let log = both(&step(&sealed(), "code", None), passed(json!({"passed": 2})), passed(json!({"passed": 1, "skipped": 3, "deselected": 2})));
    let v = View::new(&log);
    assert_eq!(outcome::result_family(&v, "c1", result(&log, TEST)), Family::Green);
    assert_eq!(outcome::result_family(&v, "c1", result(&log, TOUCHING)), Family::Green);
    assert_eq!(lifecycle::state(&v, "c1"), "green");
}

#[test]
fn a_touching_failure_keeps_the_part_from_green_and_is_its_regression() {
    let failed = ("failed", "tests_failed", json!({"location": {"file_path": TOUCHING, "line": 12, "kind": "assertion"}, "counts": {"passed": 3, "failed": 1}}));
    let log = both(&step(&sealed(), "code", None), passed(json!({"passed": 2})), failed);
    let v = View::new(&log);
    assert_eq!(outcome::result_family(&v, "c1", result(&log, TEST)), Family::Green, "the own tests pass");
    assert_ne!(outcome::result_family(&v, "c1", result(&log, TOUCHING)), Family::Green);
    assert_ne!(lifecycle::state(&v, "c1"), "green");
    // it passed at the tests-first commit, so its failure now is the change's regression
    assert!(whys(&log).contains(&"regression".to_string()), "{:?}", whys(&log));
    // a pass that counted a failure beside it is no pass
    let log = both(&step(&sealed(), "code", None), passed(json!({"passed": 2})), passed(json!({"passed": 3, "failed": 1})));
    let v = View::new(&log);
    assert_ne!(outcome::result_family(&v, "c1", result(&log, TOUCHING)), Family::Green);
    assert_ne!(lifecycle::state(&v, "c1"), "green");
}

#[test]
fn the_own_command_runs_the_own_files_and_the_touching_command_the_touching_ones() {
    let log = step(&sealed(), "code", None);
    let log = raw(&log, criterion::TOUCHING_DERIVED, json!({"criterion_id": "c1", "file_path": "tests/test_login.py", "imports": "src/login.py"}));
    let v = View::new(&log);
    assert_eq!(criterion::derived_command(&v, "c1").as_deref(), Some(OWN_COMMAND), "a derived touching file never joins the own run");
    assert_eq!(criterion::command(&v, "c1").as_deref(), Some(OWN_COMMAND));
    assert_eq!(
        criterion::touching_command(&v, "c1").as_deref(),
        Some("python -m pytest tests/test_login_old.py tests/test_login.py"),
        "the declared, then the derived, under the stack's runner"
    );
    // a touching test that is also one of the part's own runs in the own run only
    let both = raw(&log, "touching_test_declared", json!({"criterion_id": "c1", "file_path": TEST, "test_name": "t1"}));
    assert_eq!(criterion::touching_command(&View::new(&both), "c1").as_deref(), Some("python -m pytest tests/test_login_old.py tests/test_login.py"));
    // with no touching test there is no touching run
    let none = criterion_fixture(&["c2"], true, false, AT, TENANT).unwrap_or_default();
    assert_eq!(criterion::touching_command(&View::new(&none), "c2"), None);
}

#[test]
fn a_touching_run_that_exited_zero_with_nothing_failed_counted_is_green_whatever_the_table_called_it() {
    // pytest's table reads a pass beside a skip as unclassified (no positive marker); the run
    // exited 0 and its counts show nothing failed: for a touching test that is a pass
    let unread = |word: &'static str, counts: Value| (word, "unclassified", json!({"counts": counts}));
    let log = both(&step(&sealed(), "code", None), passed(json!({"passed": 2})), unread("passed", json!({"passed": 4, "skipped": 1})));
    let v = View::new(&log);
    assert_eq!(outcome::result_family(&v, "c1", result(&log, TOUCHING)), Family::Green);
    assert_eq!(lifecycle::state(&v, "c1"), "green");
    assert!(!whys(&log).contains(&"run_unclassified".to_string()), "{:?}", whys(&log));
    // every test of the touching file skipped: nothing failed, nothing to explain
    let all_skipped = ("passed", "no_tests_collected", json!({"counts": {"skipped": 2}}));
    let log = both(&step(&sealed(), "code", None), passed(json!({"passed": 2})), all_skipped);
    assert_eq!(outcome::result_family(&View::new(&log), "c1", result(&log, TOUCHING)), Family::Green);
    // no counts, or a run that did not exit 0, is no pass
    for (touching, why) in [(unread("passed", json!({})), "nothing counted"), (unread("failed", json!({"passed": 4})), "a non-zero exit")] {
        let log = both(&step(&sealed(), "code", None), passed(json!({"passed": 2})), touching);
        let v = View::new(&log);
        assert_ne!(outcome::result_family(&v, "c1", result(&log, TOUCHING)), Family::Green, "{why}");
        assert_ne!(lifecycle::state(&v, "c1"), "green", "{why}");
    }
}

#[test]
fn a_touching_failure_that_failed_at_the_tests_first_commit_does_not_block() {
    // t_old failed at the tests-first commit and fails no more at the fix commit: pre-existing
    let base = failed(vec![format!("{TOUCHING}::t_old")]);
    let log = both(&step(&sealed_touching(base.clone()), "code", None), passed(json!({"passed": 2})), base);
    let v = View::new(&log);
    assert_eq!(outcome::result_family(&v, "c1", result(&log, TEST)), Family::Green, "the own tests pass");
    assert_eq!(outcome::result_family(&v, "c1", result(&log, TOUCHING)), Family::Green, "every test failing now failed at the tests-first commit");
    assert_eq!(lifecycle::state(&v, "c1"), "green");
}

#[test]
fn a_new_failing_test_at_the_fix_commit_blocks_the_part() {
    // t_old failed at the tests-first commit, and t_new fails at the fix commit: the new one is the change's
    let log = both(
        &step(&sealed_touching(failed(vec![format!("{TOUCHING}::t_old")])), "code", None),
        passed(json!({"passed": 2})),
        failed(vec![format!("{TOUCHING}::t_old"), format!("{TOUCHING}::t_new")]),
    );
    let v = View::new(&log);
    assert_ne!(outcome::result_family(&v, "c1", result(&log, TOUCHING)), Family::Green, "t_new was not failing at the tests-first commit");
    assert_eq!(outcome::result_family(&v, "c1", result(&log, TOUCHING)), Family::ValidRed);
    assert_ne!(lifecycle::state(&v, "c1"), "green");
}

#[test]
fn a_touching_failure_with_no_failures_at_the_tests_first_commit_blocks_the_part() {
    // the touching file passed at the tests-first commit, so t_new failing now is the change's
    let log = both(&step(&sealed(), "code", None), passed(json!({"passed": 2})), failed(vec![format!("{TOUCHING}::t_new")]));
    let v = View::new(&log);
    assert_eq!(outcome::result_family(&v, "c1", result(&log, TOUCHING)), Family::ValidRed, "nothing was failing at the tests-first commit to stand behind it");
    assert_ne!(lifecycle::state(&v, "c1"), "green");
}

#[test]
fn a_touching_failure_without_per_test_ids_blocks_the_part() {
    // the run failed but named no failing tests: it reads as it always did
    let failed = ("failed", "tests_failed", json!({"location": {"file_path": TOUCHING, "line": 12, "kind": "assertion"}, "counts": {"passed": 3, "failed": 1}}));
    let log = both(&step(&sealed(), "code", None), passed(json!({"passed": 2})), failed);
    let v = View::new(&log);
    assert_eq!(outcome::result_family(&v, "c1", result(&log, TOUCHING)), Family::ValidRed);
    assert_ne!(lifecycle::state(&v, "c1"), "green");
}
