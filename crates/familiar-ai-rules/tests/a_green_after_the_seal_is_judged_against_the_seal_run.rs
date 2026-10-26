//! Lane H6 (plan of record 2026-10-19, H-3, D3, D4, D6, D7): after the seal a new-style pass is
//! green only against the seal run's counts: no test failed, no more skipped or deselected than at
//! the seal, and at least as many passed as ran there (passed + failed). With no seal run to judge
//! against, green only with nothing skipped; a seal run with counts and none here can't tell. An
//! absent count field is 0: the count rows write a field only where the summary printed it, and a
//! summary that leaves a field out leaves it out at zero (pytest's `1 failed` prints no `0 passed`).
//! A red after the seal whose reason isn't shown is still a `tests_failed` attempt (D3); a run with
//! the anchor put back (teeth) stays strict.

mod common;

use common::{raw, step, whys, AT, TENANT, TEST, TOUCHING};
use familiar_ai_facts::Log;
use familiar_ai_rules::outcome::{self, Family};
use familiar_ai_rules::sample::criterion_fixture;
use familiar_ai_rules::{criterion, lifecycle, View};
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

/// A run at the latest commit as the binary writes it, translated as `label` with `extra`; the
/// own test's result names the run, the touching test passed, the stamp.
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

/// An honest red in the planned test, as the binary reads it now, with the seal run's `counts`.
fn red(counts: Value) -> Value {
    json!({"location": {"file_path": TEST, "line": 7, "kind": "assertion"}, "counts": counts})
}

fn sealed_with(counts: Value) -> Log {
    let log = ran(&tests_first(), "failed", "tests_failed", red(counts));
    assert!(criterion::sealed_at(&View::new(&log), "c1", criterion::Place::Laptop).is_some(), "sealed");
    log
}

/// The family of the own test's result of a pass at a code commit after `sealed`.
fn green_after(sealed: &Log, extra: Value) -> (Family, &'static str) {
    let log = ran(&step(sealed, "code", None), "passed", "passed", extra);
    let v = View::new(&log);
    let r = log.iter().rev().find(|f| f.kind == "pass_please_pass" && f.fields["file_path"] == TEST).expect("the result");
    (outcome::result_family(&v, "c1", r), lifecycle::state(&v, "c1"))
}

#[test]
fn a_pass_after_the_seal_is_green_only_against_the_seal_runs_counts() {
    let sealed = sealed_with(json!({"passed": 1, "failed": 1}));
    let rows: Vec<(Value, Family, &str)> = vec![
        (json!({"counts": {"passed": 2}}), Family::Green, "every test that ran at the seal passed"),
        (json!({"counts": {"passed": 3}}), Family::Green, "a test added since"),
        (json!({"counts": {"passed": 2, "deselected": 0, "skipped": 0}}), Family::Green, "zeros printed"),
        (json!({"counts": {"passed": 1, "skipped": 1}}), Family::Unknown, "a test newly skipped asks (H-3)"),
        (json!({"counts": {"passed": 1, "deselected": 1}}), Family::Unknown, "a test newly deselected asks (H-3)"),
        (json!({"counts": {"passed": 1}}), Family::Unknown, "fewer passed than ran at the seal (D6)"),
        (json!({"counts": {"passed": 2, "failed": 1}}), Family::Unknown, "a failure beside the pass"),
        (json!({"counts": {}}), Family::Unknown, "the seal run counted, this one did not"),
    ];
    for (extra, want, why) in rows {
        let (got, state) = green_after(&sealed, extra.clone());
        assert_eq!(got, want, "{extra}: {why}");
        assert_eq!(state == "green", want == Family::Green, "{extra}: {why} (state {state})");
    }
}

#[test]
fn the_seal_runs_own_skips_and_deselects_are_the_persons_and_stay_allowed() {
    let sealed = sealed_with(json!({"passed": 1, "failed": 1, "skipped": 1, "deselected": 2}));
    assert_eq!(green_after(&sealed, json!({"counts": {"passed": 2, "skipped": 1, "deselected": 2}})).0, Family::Green);
    assert_eq!(green_after(&sealed, json!({"counts": {"passed": 2, "skipped": 2, "deselected": 1}})).0, Family::Green, "skipped + deselected compared together");
    assert_eq!(green_after(&sealed, json!({"counts": {"passed": 2, "skipped": 2, "deselected": 2}})).0, Family::Unknown, "one more left out than at the seal");
}

#[test]
fn the_card_of_a_pass_the_seal_run_cannot_vouch_for_names_that_run() {
    let sealed = sealed_with(json!({"passed": 1, "failed": 1}));
    let log = ran(&step(&sealed, "code", None), "passed", "passed", json!({"counts": {"passed": 1, "deselected": 1}}));
    assert!(whys(&log).contains(&"run_unclassified".to_string()), "{:?}", whys(&log));
    let v = View::new(&log);
    let run = outcome::card_run(&v, "c1", "run_unclassified").expect("the card's run");
    assert_eq!(run.fields.get("counts"), Some(&json!({"passed": 1, "deselected": 1})), "the card shows the run it asks about");
}

#[test]
fn an_absent_count_field_reads_as_zero() {
    // pytest's seal prints `1 failed` (no `0 passed`), its green `1 passed`
    let sealed = sealed_with(json!({"failed": 1}));
    assert_eq!(green_after(&sealed, json!({"counts": {"passed": 1}})), (Family::Green, "green"));
}

#[test]
fn with_no_seal_run_to_judge_against_green_needs_nothing_skipped() {
    // a seal whose run counted nothing (sh): no baseline
    let sealed = sealed_with(json!({}));
    assert_eq!(green_after(&sealed, json!({"counts": {}})).0, Family::Green, "the script's exit 0 is its green (H-2)");
    assert_eq!(green_after(&sealed, json!({"counts": {"passed": 1}})).0, Family::Green);
    assert_eq!(green_after(&sealed, json!({"counts": {"passed": 1, "skipped": 1}})).0, Family::Unknown);
    // a legacy seal (its translation carries no counts) is no baseline either
    let legacy = ran(&tests_first(), "failed", "tests_failed", json!({}));
    assert_eq!(green_after(&legacy, json!({"counts": {"passed": 1}})).0, Family::Green);
    assert_eq!(green_after(&legacy, json!({"counts": {"passed": 1, "skipped": 1}})).0, Family::Unknown);
    // and a legacy pass reads as it always did
    assert_eq!(green_after(&legacy, json!({})).0, Family::Green);
}

#[test]
fn a_red_after_the_seal_whose_reason_is_not_shown_is_still_a_tests_failed_attempt() {
    let sealed = sealed_with(json!({"passed": 1, "failed": 1}));
    let unread = || json!({"counts": {"failed": 1, "passed": 1}});
    let log = ran(&step(&sealed, "code", None), "failed", "tests_failed", unread());
    let v = View::new(&log);
    let r = log.iter().rev().find(|f| f.kind == "pass_please_pass" && f.fields["file_path"] == TEST).expect("the result");
    assert_eq!(outcome::result_family(&v, "c1", r), Family::ValidRed, "D3: the planned test did fail");
    assert_eq!(lifecycle::attempts_of(&v, "c1", "tests_failed"), 1);
    assert!(!whys(&log).contains(&"run_unclassified".to_string()), "{:?}", whys(&log));
    let log = ran(&step(&log, "code", None), "failed", "tests_failed", unread());
    assert_eq!(lifecycle::attempts_of(&View::new(&log), "c1", "tests_failed"), 2);
    assert!(whys(&log).contains(&"two_failures".to_string()), "{:?}", whys(&log));
    // an exception elsewhere in the project reads can't-tell (the owner, 2026-10-19), so at a fix
    // commit it is the agent's failed try like any unread red (D3)
    let log = ran(&step(&sealed, "code", None), "failed", "tests_failed", json!({"location": {"file_path": "src/db.py", "line": 3, "function": "def connect():", "kind": "exception"}, "counts": {"failed": 1}}));
    assert_eq!(lifecycle::attempts_of(&View::new(&log), "c1", "test_broken"), 0);
    assert_eq!(lifecycle::attempts_of(&View::new(&log), "c1", "tests_failed"), 1);
}

#[test]
fn a_teeth_run_stays_strict() {
    let sealed = sealed_with(json!({"passed": 1, "failed": 1}));
    let log = step(&sealed, "code", None);
    let id = head(&log);
    let log = raw(&log, "run_reserved", json!({"run_id": "r-teeth", "criterion_id": "c1", "criterion_rev": "R1", "commit": id, "command": "pytest -q", "resources": ["tests"], "wait": "60", "by": "binary"}));
    let log = raw(&log, "test_log_translation", json!({"run_id": "r-teeth", "criterion_id": "c1", "commit": id, "by": "regex", "label": "tests_failed", "evidence": "", "failing_tests": [], "classified_on": "full", "counts": {"failed": 1}}));
    let log = raw(&log, "pass_please_pass", json!({"criterion_id": "c1", "file_path": TEST, "commit": id, "outcome": "failed", "ran_in": "laptop", "with_anchor_reverted": true, "run_id": "r-teeth"}));
    let v = View::new(&log);
    let r = log.last().expect("the teeth result");
    assert_eq!(outcome::result_family(&v, "c1", r), Family::Unknown, "a red with the anchor put back must show its reason");
}

#[test]
fn before_the_seal_a_skip_beside_the_pass_asks_and_a_deselect_does_not() {
    let log = ran(&tests_first(), "passed", "passed", json!({"counts": {"passed": 1, "skipped": 1}}));
    assert_eq!(lifecycle::tries_not_red(&View::new(&log), "c1"), 0, "no try: can't tell (D4)");
    assert!(whys(&log).contains(&"run_unclassified".to_string()), "{:?}", whys(&log));
    let log = ran(&tests_first(), "passed", "passed", json!({"counts": {"passed": 1, "deselected": 4}}));
    assert_eq!(lifecycle::tries_not_red(&View::new(&log), "c1"), 1, "a pass: tests not red");
    assert_eq!(whys(&log), ["tests_not_red"]);
}
