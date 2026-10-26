//! A commit's content is its files, its flags, its anchor overlap and each
//! criterion's fingerprint as observed there. Two tests-only commits of one
//! file with different contents differ; a rebase, observed afresh, lands the
//! same contents.

mod common;

use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::lifecycle::state;
use familiar_ai_rules::moves::criterion_admissible;
use familiar_ai_rules::sample::{apply_move, criterion_fixture};
use familiar_ai_rules::View;
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";
const TEST: &str = "tests/test_c1.py";

fn ap(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { panic!("fields must be an object") };
    append_raw(log, kind, m, AT, TENANT).unwrap_or_else(|e| panic!("append {kind}: {e}"))
}

fn tests_commit(log: &Log, id: &str, seen: &[(&str, &str)]) -> Log {
    let mut log = ap(log, "commit_seen", json!({ "commit": id, "files": [TEST], "touches_tests": true, "touches_code": false, "anchor_overlap": [] }));
    for (c, h) in seen {
        log = ap(&log, "tests_observed", json!({ "criterion_id": c, "criterion_rev": "R", "commit": id, "tests_hash": h }));
    }
    log
}

fn key(log: &Log, id: &str) -> Option<String> {
    View::new(log).idx().key_of(id).map(str::to_string)
}

#[test]
fn two_tests_only_commits_of_one_file_with_different_fingerprints_are_different_contents() {
    let log = criterion_fixture(&["c1", "c2"], true, false, AT, TENANT).expect("fixture");
    let log = tests_commit(&log, "k1", &[("c1", "S1")]);
    let log = tests_commit(&log, "k2", &[("c1", "S1x")]);
    assert_ne!(key(&log, "k1"), key(&log, "k2"));
}

#[test]
fn the_same_files_and_fingerprints_are_the_same_content_in_any_order() {
    let log = criterion_fixture(&["c1", "c2"], true, false, AT, TENANT).expect("fixture");
    let log = tests_commit(&log, "k1", &[("c1", "S1"), ("c2", "S2")]);
    let log = tests_commit(&log, "k2", &[("c2", "S2"), ("c1", "S1")]);
    assert_eq!(key(&log, "k1"), key(&log, "k2"));
    // the same file, another criterion's fingerprint differs: another content
    let log = tests_commit(&log, "k3", &[("c1", "S1"), ("c2", "S2x")]);
    assert_ne!(key(&log, "k1"), key(&log, "k3"));
}

#[test]
fn a_rebase_lands_the_same_contents_with_their_observations() {
    let log = common::sealed();
    let before = state(&View::new(&log), "c1");
    let m = criterion_admissible(&View::new(&log)).into_iter().find(|m| m.label.as_deref() == Some("rebase")).expect("rebase");
    assert!(m.more.as_ref().is_some_and(|fs| fs.iter().any(|f| f.kind == "tests_observed")), "the rewritten commit observed afresh");
    let after = apply_move(&log, &m, AT, TENANT).expect("applies");
    assert_eq!(key(&after, "k1"), key(&after, "k2"), "the same content under the new id");
    assert_eq!(state(&View::new(&after), "c1"), before);
}
