//! A legacy `commit_seen.tests_hash` still resolves an old record's
//! tests-first commit, read unverified; the binary never stamps from it, since
//! there is no observation, and it is never proof.

use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::criterion::{current_tests, observed, tests_first_commit, tests_hash_at, Place};
use familiar_ai_rules::moves::criterion_admissible;
use familiar_ai_rules::sample::criterion_fixture;
use familiar_ai_rules::View;
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";
const TEST: &str = "tests/test_c1.py";

fn ap(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { panic!("fields must be an object") };
    append_raw(log, kind, m, AT, TENANT).unwrap_or_else(|e| panic!("append {kind}: {e}"))
}

/// c1's tests-first commit as the old binary wrote it: one hash on the commit, no observation.
fn old_record() -> Log {
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).expect("fixture");
    let log = ap(&log, "anchor_sha_matched", json!({ "criterion_id": "c1", "file_path": "src/login.py", "symbol": "check_password", "line_start": 40, "line_end": 62, "sha": "base", "lines_hash": "L0", "matched": true, "checked_at": "start" }));
    let log = ap(&log, "commit_seen", json!({ "commit": "k1", "files": [TEST], "touches_tests": true, "touches_code": false, "anchor_overlap": [], "tests_hash": "S1" }));
    let log = ap(&log, "test_written", json!({ "criterion_id": "c1", "file_path": TEST, "test_name": "t1" }));
    let log = ap(&log, "pass_please_pass", json!({ "criterion_id": "c1", "file_path": TEST, "commit": "k1", "outcome": "failed", "ran_in": "laptop", "with_anchor_reverted": false }));
    ap(&log, "pass_please_pass", json!({ "criterion_id": "c1", "file_path": "tests/test_login_old.py", "commit": "k1", "outcome": "passed", "ran_in": "laptop", "with_anchor_reverted": false }))
}

#[test]
fn a_legacy_hash_is_read_when_nothing_was_observed() {
    let log = old_record();
    let v = View::new(&log);
    assert!(observed(&v, "c1", "k1").is_none());
    assert_eq!(tests_hash_at(&v, "c1", "k1"), Some("S1"), "read unverified");
    assert_eq!(current_tests(&v, "c1"), Some("S1"));
}

#[test]
fn an_old_record_still_finds_its_tests_first_commit() {
    let log = ap(&old_record(), "fingerprint_stamped", json!({ "criterion_id": "c1", "commit": "k1", "proof_not_tampered": "S1", "ran_in": "laptop" }));
    let v = View::new(&log);
    let tf = tests_first_commit(&v, "c1").expect("found by content");
    assert_eq!(tf.ids, vec!["k1".to_string()]);
    assert!(familiar_ai_rules::criterion::seal_raw(&v, "c1", Place::Laptop).is_some());
}

#[test]
fn the_binary_never_stamps_from_a_legacy_hash() {
    let log = old_record();
    let ms = criterion_admissible(&View::new(&log));
    assert!(!ms.iter().any(|m| m.kind == "fingerprint_stamped"), "no observation, no stamp");
}

#[test]
fn an_observation_wins_over_the_legacy_hash() {
    let log = ap(&old_record(), "tests_observed", json!({ "criterion_id": "c1", "criterion_rev": "R1", "commit": "k1", "tests_hash": "H1" }));
    let v = View::new(&log);
    assert_eq!(tests_hash_at(&v, "c1", "k1"), Some("H1"));
    assert_eq!(current_tests(&v, "c1"), Some("H1"));
    let ms = criterion_admissible(&v);
    let s = ms.iter().find(|m| m.kind == "fingerprint_stamped").expect("observed: the binary stamps");
    assert_eq!(s.fields.get("proof_not_tampered"), Some(&json!("H1")));
}
