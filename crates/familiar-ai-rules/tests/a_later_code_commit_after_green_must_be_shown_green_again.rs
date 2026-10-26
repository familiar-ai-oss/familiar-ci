//! A later change of the part after green, a revert included, must be shown green
//! again: green holds only at the part's latest commit that changes its place or
//! its own tests, not at a commit that was green once. A change of another
//! part's place keeps it green.

mod common;
use common::*;
use familiar_ai_facts::Log;
use familiar_ai_rules::criterion::Place;
use familiar_ai_rules::{lifecycle, View};
use serde_json::json;

const ANCHOR: &str = "src/login.py";
const OTHER: &str = "src/helper.py";
const DOCS: &str = "docs/CHANGELOG.md";

/// A code commit c1's binary lands after green, changing the anchor beside a
/// helper of it and leaving the tests as they stand.
fn code_commit(log: &Log) -> Log {
    let log = raw(
        log,
        "commit_seen",
        json!({ "commit": "k3", "files": [ANCHOR, OTHER], "touches_tests": false, "touches_code": true, "anchor_overlap": ["c1"] }),
    );
    raw(
        &log,
        "tests_observed",
        json!({ "criterion_id": "c1", "criterion_rev": "R1", "commit": "k3", "tests_hash": "S1" }),
    )
}

/// The binary's run of c1 at the code commit: the test with `outcome`, the
/// touching test passed, the stamp under the standing proof.
fn code_run(log: &Log, outcome: &str) -> Log {
    let mut log = raw(
        log,
        "pass_please_pass",
        json!({ "criterion_id": "c1", "file_path": TEST, "commit": "k3", "outcome": outcome, "ran_in": "laptop", "with_anchor_reverted": false }),
    );
    log = raw(
        &log,
        "pass_please_pass",
        json!({ "criterion_id": "c1", "file_path": TOUCHING, "commit": "k3", "outcome": "passed", "ran_in": "laptop", "with_anchor_reverted": false }),
    );
    raw(
        &log,
        "fingerprint_stamped",
        json!({ "criterion_id": "c1", "commit": "k3", "proof_not_tampered": "S1", "ran_in": "laptop" }),
    )
}

/// A commit that changes neither code nor tests of c1.
fn docs_commit(log: &Log) -> Log {
    raw(
        log,
        "commit_seen",
        json!({ "commit": "k3", "files": [DOCS], "touches_tests": false, "touches_code": false, "anchor_overlap": [] }),
    )
}

#[test]
fn a_later_code_commit_with_no_run_breaks_green_and_owes_a_run() {
    let log = code_commit(&green());
    let v = View::new(&log);
    assert!(!lifecycle::green_conditions(&v, "c1", Place::Laptop), "the fix commit was green, but a later code commit moved the head");
    assert!(lifecycle::runnable(&v, "c1"), "a run is owed at the code commit");
}

#[test]
fn a_passing_run_at_the_later_code_commit_earns_green_again() {
    let log = code_run(&code_commit(&green()), "passed");
    let v = View::new(&log);
    assert!(lifecycle::green_conditions(&v, "c1", Place::Laptop), "green is shown again at the latest code commit");
    assert_eq!(state(&log), "green");
    assert!(whys(&log).is_empty(), "{:?}", whys(&log));
}

#[test]
fn a_later_commit_that_touches_no_code_keeps_green() {
    let log = docs_commit(&green());
    let v = View::new(&log);
    assert!(lifecycle::green_conditions(&v, "c1", Place::Laptop), "the docs commit does not move the latest code commit");
    assert_eq!(state(&log), "green");
    assert!(whys(&log).is_empty(), "{:?}", whys(&log));
}

/// A later code commit of another part: it touches code but leaves c1's anchor
/// and c1's tests as they stand.
fn other_code_commit(log: &Log) -> Log {
    raw(
        log,
        "commit_seen",
        json!({ "commit": "k4", "files": [OTHER], "touches_tests": false, "touches_code": true, "anchor_overlap": [] }),
    )
}

#[test]
fn a_later_code_commit_of_another_part_keeps_green() {
    let log = other_code_commit(&green());
    let v = View::new(&log);
    assert!(lifecycle::green_conditions(&v, "c1", Place::Laptop), "the commit does not touch c1's place or its tests");
    assert_eq!(state(&log), "green");
    assert!(whys(&log).is_empty(), "{:?}", whys(&log));
}
