//! The owner, 2026-10-13: "k6 is a second red seal, that requires green".
//! After the way back went green, its tests put back to the red seal's (k4),
//! code (k5), then the edited tests committed alone again and stamped (k6,
//! red on the laptop), then code that passes (k7): k6 is the part's seal.
//! CI proves it from k6 (red there, green with teeth after it), the way back
//! is spent, and the lifecycle reads the same history the same way. CI reads
//! only the plan's stamps: a tests-only re-stamp of the proof after the way
//! back's green. See docs/decisions.md#the-way-back

mod common;
use common::*;
use familiar_ai_facts::Log;
use familiar_ai_rules::criterion::Place;
use familiar_ai_rules::{action, criterion, lifecycle, View};
use serde_json::json;

/// Green, announced after green, the edited test committed alone, passed,
/// stamped (kw): the way back.
fn way_back() -> Log {
    let log = step(&green(), "announce_after_green", None);
    let log = step(&log, "changed_test", None);
    run(&log, "passed")
}

fn commit(log: &Log, id: &str, tests: bool, hash: &str) -> Log {
    let (files, overlap) = if tests { (json!([TEST]), json!([])) } else { (json!(["src/login.py"]), json!(["c1"])) };
    let log = raw(log, "commit_seen", json!({ "commit": id, "files": files, "touches_tests": tests, "touches_code": !tests, "anchor_overlap": overlap }));
    raw(&log, "tests_observed", json!({ "criterion_id": "c1", "criterion_rev": "R1", "commit": id, "tests_hash": hash }))
}

fn laptop_run(log: &Log, id: &str, hash: &str, outcome: &str) -> Log {
    let mut log = log.clone();
    for (fp, o) in [(TEST, outcome), (TOUCHING, "passed")] {
        log = raw(&log, "pass_please_pass", json!({ "criterion_id": "c1", "file_path": fp, "commit": id, "outcome": o, "ran_in": "laptop", "with_anchor_reverted": false }));
    }
    raw(&log, "fingerprint_stamped", json!({ "criterion_id": "c1", "commit": id, "proof_not_tampered": hash, "ran_in": "laptop" }))
}

/// The trace's history after the way back: k4 tests back to the red seal's,
/// k5 code, neither stamped; k6 the edited tests alone, red, stamped; k7 code,
/// green, stamped.
fn second_seal() -> Log {
    let log = commit(&way_back(), "k4", true, "S1");
    let log = commit(&log, "k5", false, "S1");
    let log = commit(&log, "k6", true, "S1x");
    let log = laptop_run(&log, "k6", "S1x", "failed");
    let log = commit(&log, "k7", false, "S1x");
    laptop_run(&log, "k7", "S1x", "passed")
}

/// The red seal's tests-first commit of the way back, the commit CI used to read.
fn t1(log: &Log) -> String {
    let v = View::new(log);
    v.kind("commit_seen")
        .find(|f| f.fields.get("touches_tests").and_then(|x| x.as_bool()) == Some(true))
        .and_then(|f| f.fields.get("commit").and_then(|x| x.as_str()))
        .unwrap()
        .to_string()
}

fn pipe(log: &Log, fp: &str, id: &str, outcome: &str, teeth: bool) -> Log {
    raw(log, "pass_please_pass", json!({ "criterion_id": "c1", "file_path": fp, "commit": id, "outcome": outcome, "ran_in": "pipeline", "with_anchor_reverted": teeth }))
}

fn pipe_stamp(log: &Log, id: &str, hash: &str) -> Log {
    raw(log, "fingerprint_stamped", json!({ "criterion_id": "c1", "commit": id, "proof_not_tampered": hash, "ran_in": "pipeline" }))
}

/// A pipeline run at k7: red at `seal` (stamped with `hash`), green at k7 with
/// the touching test, stamped, the teeth run failing.
fn pipeline(seal: &str, seal_outcome: &str, hash: &str) -> Log {
    let log = second_seal();
    let mut log = raw(&log, "pipeline_run_seen", json!({ "head_commit": "k7" }));
    log = pipe(&log, TEST, seal, seal_outcome, false);
    log = pipe_stamp(&log, seal, hash);
    log = pipe(&log, TEST, "k7", "passed", false);
    log = pipe(&log, TOUCHING, "k7", "passed", false);
    log = pipe_stamp(&log, "k7", "S1x");
    pipe(&log, TEST, "k7", "failed", true)
}

#[test]
fn the_laptop_reads_k6_as_the_seal() {
    let log = second_seal();
    let v = View::new(&log);
    let seal = criterion::sealed_at(&v, "c1", Place::Laptop).expect("sealed");
    assert_eq!(seal.fields.get("commit").and_then(|x| x.as_str()), Some("k6"), "the lifecycle's seal is k6's stamp");
    assert!(criterion::mark(&v, "c1").is_none(), "the way back is spent");
    assert_eq!(state(&log), "green", "{:?}", whys(&log));
}

#[test]
fn ci_reads_k6_as_the_tests_first_commit_and_the_way_back_spent() {
    let log = second_seal();
    let v = View::new(&log);
    assert_eq!(action::tests_first(&v, "c1"), Some("k6"));
    assert_eq!(action::mark(&v, "c1"), None, "the way back is spent");
    assert_eq!(action::seal_hash(&v, "c1"), Some("S1x"));
}

#[test]
fn red_at_the_first_seal_and_green_at_k7_is_not_proven() {
    let log = pipeline(&t1(&second_seal()), "failed", "S1");
    let v = View::new(&log);
    assert!(!action::proven(&v, "c1"), "CI accepted what the laptop refuses");
    assert_eq!(action::check_state(&v), "failed");
    assert_ne!(lifecycle::state(&v, "c1"), "proven", "{:?}", whys(&log));
}

#[test]
fn red_at_k6_and_green_with_teeth_at_k7_is_proven_and_the_lifecycle_agrees() {
    let log = pipeline("k6", "failed", "S1x");
    let v = View::new(&log);
    assert!(action::red(&v, "c1"));
    assert!(action::stamped(&v, "c1"));
    assert!(action::green_at_head(&v, "c1"));
    assert!(action::teeth(&v, "c1"));
    assert!(action::proven(&v, "c1"));
    assert_eq!(action::check_state(&v), "passed");
    assert_eq!(lifecycle::state(&v, "c1"), "proven", "{:?}", whys(&log));
}

#[test]
fn green_at_k6_is_not_red_there_and_not_proven() {
    let log = pipeline("k6", "passed", "S1x");
    let v = View::new(&log);
    assert!(!action::red(&v, "c1"));
    assert!(!action::proven(&v, "c1"));
    assert_eq!(action::check_state(&v), "failed");
    assert_ne!(lifecycle::state(&v, "c1"), "proven", "{:?}", whys(&log));
}

#[test]
fn the_action_asks_for_the_result_at_k6_first() {
    let log = raw(&second_seal(), "pipeline_run_seen", json!({ "head_commit": "k7" }));
    let v = View::new(&log);
    let ms = action::action_admissible(&v);
    assert!(!ms.is_empty());
    assert!(
        // S9d: the pipeline's run is reserved first, then its result
        ms.iter().all(|m| m.kind == "run_reserved" && m.fields.get("by").and_then(|x| x.as_str()) == Some("pipeline") && m.fields.get("commit").and_then(|x| x.as_str()) == Some("k6")),
        "{:?}",
        ms.iter().map(|m| (&m.kind, m.fields.get("commit"))).collect::<Vec<_>>()
    );
}
