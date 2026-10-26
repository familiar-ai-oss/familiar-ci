//! A sealed test edited after green with no `test_change_announced` before
//! the edit's commit never re-proves, and an announcement made after the edit
//! does not cover it.

mod common;
use common::*;
use familiar_ai_rules::criterion::Place;
use familiar_ai_rules::{criterion, lifecycle, View};
use serde_json::json;

#[test]
fn an_unannounced_edit_after_green_is_proof_tampered_and_not_green() {
    let log = edit_run(&green(), "e1", "S1x", "passed");
    let v = View::new(&log);
    assert!(whys(&log).contains(&"proof_tampered".to_string()), "{:?}", whys(&log));
    assert!(!lifecycle::green_conditions(&v, "c1", Place::Laptop));
    assert!(criterion::mark(&v, "c1").is_none());
}

#[test]
fn an_announcement_after_the_edit_covers_nothing() {
    let log = edit_run(&green(), "e1", "S1x", "passed");
    let log = raw(&log, "test_change_announced", json!({ "criterion_id": "c1", "reason": "a sibling's wording broke an assertion" }));
    let v = View::new(&log);
    assert!(whys(&log).contains(&"proof_tampered".to_string()), "{:?}", whys(&log));
    assert!(!lifecycle::green_conditions(&v, "c1", Place::Laptop));
    assert!(criterion::mark(&v, "c1").is_none(), "a later announcement never covers an earlier edit");
}

#[test]
fn an_announcement_between_the_edit_and_its_run_covers_nothing() {
    let log = raw(&green(), "commit_seen", json!({ "commit": "e1", "files": [TEST], "touches_tests": true, "touches_code": false, "anchor_overlap": [] }));
    let log = raw(&log, "tests_observed", json!({ "criterion_id": "c1", "criterion_rev": "R1", "commit": "e1", "tests_hash": "S1x" })); // the observation beside the commit
    let mut log = raw(&log, "test_change_announced", json!({ "criterion_id": "c1", "reason": "too late" }));
    for (fp, o) in [(TEST, "passed"), (TOUCHING, "passed")] {
        log = raw(&log, "pass_please_pass", json!({ "criterion_id": "c1", "file_path": fp, "commit": "e1", "outcome": o, "ran_in": "laptop", "with_anchor_reverted": false }));
    }
    let log = raw(&log, "fingerprint_stamped", json!({ "criterion_id": "c1", "commit": "e1", "proof_not_tampered": "S1x", "ran_in": "laptop" }));
    assert!(criterion::mark(&View::new(&log), "c1").is_none());
    assert!(whys(&log).contains(&"proof_tampered".to_string()), "{:?}", whys(&log));
}
