//! An announcement after green reseals under the new stamp, and the proof
//! re-earns green and teeth at the head under that stamp; the seal's red is
//! not re-earned. See docs/decisions.md#the-way-back

mod common;
use common::*;
use familiar_ai_facts::Log;
use familiar_ai_rules::criterion::Place;
use familiar_ai_rules::{action, criterion, lifecycle, View};

/// Green, announced after green, the edited test committed alone, passed,
/// stamped: the way back.
fn way_back() -> Log {
    let log = step(&green(), "announce_after_green", None);
    assert!(lifecycle::announced(&View::new(&log), "c1"));
    assert!(lifecycle::runnable(&View::new(&log), "c1"), "the announcement made while green opens the run again");
    let log = step(&log, "changed_test", None);
    run(&log, "passed")
}

#[test]
fn the_red_seal_stands_and_green_is_reearned_under_the_new_stamp() {
    let before = green();
    let bv = View::new(&before);
    let seal = criterion::sealed_at(&bv, "c1", Place::Laptop).map(|s| s.t);
    let tf = criterion::tests_first_commit(&bv, "c1").map(|t| t.key);
    let log = way_back();
    let v = View::new(&log);
    assert_eq!(criterion::proof(&v, "c1"), Some("S1x"));
    assert_eq!(criterion::sealed_at(&v, "c1", Place::Laptop).map(|s| s.t), seal, "the seal is the red seal");
    assert_eq!(criterion::tests_first_commit(&v, "c1").map(|t| t.key), tf, "its tests-first commit too");
    assert!(criterion::mark(&v, "c1").is_some());
    assert!(criterion::green_at_announcement(&v, "c1"));
    assert!(lifecycle::green_conditions(&v, "c1", Place::Laptop));
    assert_eq!(state(&log), "green");
    assert!(whys(&log).is_empty(), "{:?}", whys(&log));
    assert!(!lifecycle::announced(&v, "c1"), "the mark spends the announcement: the next edit needs a new one");
    assert_eq!(lifecycle::test_change(&v, "c1"), Some("announced"));
}

#[test]
fn a_second_way_back_is_the_same_on_a_longer_record() {
    let log = step(&way_back(), "announce_after_green", None);
    let log = edit_run(&log, "e2", "S1y", "passed");
    let v = View::new(&log);
    assert_eq!(criterion::proof(&v, "c1"), Some("S1y"));
    assert!(criterion::mark(&v, "c1").is_some());
    assert_eq!(state(&log), "green");
    assert!(whys(&log).is_empty(), "{:?}", whys(&log));
}

#[test]
fn a_second_edit_without_a_second_announcement_is_proof_tampered() {
    let log = edit_run(&way_back(), "e2", "S1y", "passed");
    assert!(whys(&log).contains(&"proof_tampered".to_string()), "{:?}", whys(&log));
    assert!(criterion::mark(&View::new(&log), "c1").is_none());
}

/// The pipeline after the way back: red at the red seal's tests-first
/// commit, green at the head under the new hash, then the teeth run.
fn pipeline(teeth: &str) -> Log {
    let mut log = step(&way_back(), "pipeline_run_seen", None);
    log = step(&log, "pass_please_pass", Some("failed"));
    log = step(&log, "fingerprint_stamped", None);
    log = step(&log, "pass_please_pass", Some("passed"));
    log = step(&log, "pass_please_pass", Some("passed"));
    log = step(&log, "fingerprint_stamped", None);
    step(&log, "pass_please_pass", Some(teeth))
}

#[test]
fn the_pipeline_proves_it_with_teeth_under_the_new_stamp() {
    let log = pipeline("failed");
    let v = View::new(&log);
    assert!(lifecycle::proven_conditions(&v, "c1"));
    assert_eq!(state(&log), "proven");
    assert!(action::proven(&v, "c1"));
    assert_eq!(action::seal_hash(&v, "c1"), Some("S1"));
    assert_eq!(action::proof(&v, "c1"), Some("S1x"));
    assert!(action::mark(&v, "c1").is_some());
}

#[test]
fn a_teeth_run_that_passes_is_no_teeth() {
    let log = pipeline("passed");
    let v = View::new(&log);
    assert!(whys(&log).contains(&"no_teeth".to_string()), "{:?}", whys(&log));
    assert!(action::no_teeth(&v, "c1"));
    assert!(!action::proven(&v, "c1"));
}
