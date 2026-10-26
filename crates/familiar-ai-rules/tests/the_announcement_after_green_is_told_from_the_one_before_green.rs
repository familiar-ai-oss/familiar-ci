//! The two announcement paths are told apart by whether the criterion was
//! green when the announcement landed; before green, the edited test is a new
//! seal that must go red.

mod common;
use common::*;
use familiar_ai_rules::criterion::Place;
use familiar_ai_rules::{criterion, lifecycle, View};

#[test]
fn announced_before_green_the_edited_test_red_is_a_new_seal() {
    let before = sealed();
    let tf = criterion::tests_first_commit(&View::new(&before), "c1").map(|t| t.key);
    let log = step(&before, "test_change_announced", None);
    let log = step(&log, "changed_test", None);
    let log = run(&log, "failed");
    let v = View::new(&log);
    assert_eq!(state(&log), "sealed");
    assert!(criterion::mark(&v, "c1").is_none());
    assert!(!criterion::green_at_announcement(&v, "c1"));
    assert_ne!(criterion::tests_first_commit(&v, "c1").map(|t| t.key), tf, "the new seal's commit");
    assert!(!lifecycle::announced(&v, "c1"), "the new seal spends it");
}

#[test]
fn announced_before_green_the_edited_test_passing_never_reproves() {
    let log = step(&sealed(), "test_change_announced", None);
    let log = step(&log, "changed_test", None);
    let log = run(&log, "passed");
    assert!(criterion::mark(&View::new(&log), "c1").is_none());
    assert!(whys(&log).contains(&"proof_tampered".to_string()), "{:?}", whys(&log));
}

#[test]
fn announced_after_green_the_edited_test_red_is_a_new_seal_too() {
    let log = step(&green(), "announce_after_green", None);
    let log = step(&log, "changed_test", None);
    let log = run(&log, "failed");
    let v = View::new(&log);
    assert!(criterion::mark(&v, "c1").is_none(), "a red run is read first: a new seal");
    assert_eq!(state(&log), "sealed");
    assert!(criterion::sealed_at(&v, "c1", Place::Laptop).is_some());
}
