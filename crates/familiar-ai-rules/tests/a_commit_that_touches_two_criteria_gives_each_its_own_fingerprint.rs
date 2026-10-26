//! A commit that touches two criteria's tests gives each criterion its own
//! fingerprint over its own files, so an edit of c1's test leaves c2's seal
//! standing. See docs/decisions.md#one-stamp-per-criterion-per-run

use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::criterion::{green_raw, observed, seal_raw, tests_hash_at};
use familiar_ai_rules::lifecycle::{state, test_change};
use familiar_ai_rules::moves::criterion_admissible;
use familiar_ai_rules::sample::criterion_fixture;
use familiar_ai_rules::{what_to_read, View};
use familiar_ai_rules::criterion::Place;
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";
const T1: &str = "tests/test_c1.py";
const T2: &str = "tests/test_c2.py";

fn ap(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { panic!("fields must be an object") };
    append_raw(log, kind, m, AT, TENANT).unwrap_or_else(|e| panic!("append {kind}: {e}"))
}

/// The sample's two criteria, each the head of its own string, both checked.
fn two_heads() -> Log {
    let log = criterion_fixture(&["c1", "c2"], true, false, AT, TENANT).expect("fixture");
    let log = ap(&log, "criterion_head", json!({ "criterion_id": "c2" }));
    let log = ap(&log, "anchor_sha_matched", json!({ "criterion_id": "c1", "file_path": "src/login.py", "symbol": "check_password", "line_start": 40, "line_end": 62, "sha": "base", "lines_hash": "L0", "matched": true, "checked_at": "start" }));
    ap(&log, "anchor_sha_matched", json!({ "criterion_id": "c2", "file_path": "src/mail.py", "symbol": "reset_link", "line_start": 10, "line_end": 30, "sha": "base", "lines_hash": "L0", "matched": true, "checked_at": "start" }))
}

/// A commit and the binary's observations beside it, one per criterion.
fn commit(log: &Log, id: &str, files: &[&str], tests: bool, overlap: &[&str], seen: &[(&str, &str)]) -> Log {
    let mut log = ap(log, "commit_seen", json!({ "commit": id, "files": files, "touches_tests": tests, "touches_code": !tests, "anchor_overlap": overlap }));
    for (c, h) in seen {
        let rev = if *c == "c1" { "R1" } else { "R2" };
        log = ap(&log, "tests_observed", json!({ "criterion_id": c, "criterion_rev": rev, "commit": id, "tests_hash": h }));
    }
    log
}

/// The binary's run of `c`'s own test at `id`, stamped with the fingerprint
/// it observed there.
fn run(log: &Log, c: &str, id: &str, outcome: &str) -> Log {
    let file = if c == "c1" { T1 } else { T2 };
    let mut log = log.clone();
    if !log.iter().any(|f| f.kind == "test_written" && f.fields.get("criterion_id") == Some(&json!(c))) {
        log = ap(&log, "test_written", json!({ "criterion_id": c, "file_path": file, "test_name": "t1" }));
    }
    log = ap(&log, "pass_please_pass", json!({ "criterion_id": c, "file_path": file, "commit": id, "outcome": outcome, "ran_in": "laptop", "with_anchor_reverted": false }));
    if c == "c1" {
        log = ap(&log, "pass_please_pass", json!({ "criterion_id": c, "file_path": "tests/test_login_old.py", "commit": id, "outcome": "passed", "ran_in": "laptop", "with_anchor_reverted": false }));
    }
    let v = View::new(&log);
    let proof = observed(&v, c, id).and_then(|o| o.fields.get("tests_hash")).cloned().expect("observed");
    ap(&log, "fingerprint_stamped", json!({ "criterion_id": c, "commit": id, "proof_not_tampered": proof, "ran_in": "laptop" }))
}

fn proof_of(log: &Log, c: &str) -> Option<String> {
    seal_raw(&View::new(log), c, Place::Laptop).and_then(|s| s.fields.get("proof_not_tampered")).and_then(Value::as_str).map(str::to_string)
}

/// One commit with both criteria's tests, both red; then code on both anchors, both green.
fn both_green() -> Log {
    let log = commit(&two_heads(), "f1", &[T1, T2], true, &[], &[("c1", "S1"), ("c2", "S2")]);
    let log = run(&log, "c1", "f1", "failed");
    let log = run(&log, "c2", "f1", "failed");
    let log = commit(&log, "f2", &["src/login.py", "src/mail.py"], false, &["c1", "c2"], &[("c1", "S1"), ("c2", "S2")]);
    let log = run(&log, "c1", "f2", "passed");
    run(&log, "c2", "f2", "passed")
}

#[test]
fn each_criterion_reads_its_own_fingerprint_at_a_commit_of_both() {
    let log = commit(&two_heads(), "f1", &[T1, T2], true, &[], &[("c1", "S1"), ("c2", "S2")]);
    let v = View::new(&log);
    assert_eq!(tests_hash_at(&v, "c1", "f1"), Some("S1"));
    assert_eq!(tests_hash_at(&v, "c2", "f1"), Some("S2"));
}

#[test]
fn each_stamp_carries_its_own_fingerprint_and_each_seal_stands() {
    let log = commit(&two_heads(), "f1", &[T1, T2], true, &[], &[("c1", "S1"), ("c2", "S2")]);
    let log = run(&log, "c1", "f1", "failed");
    // the binary's own stamp for c2, its test written and red there: it carries S2, not c1's S1
    let log = ap(&log, "test_written", json!({ "criterion_id": "c2", "file_path": T2, "test_name": "t1" }));
    let log = ap(&log, "pass_please_pass", json!({ "criterion_id": "c2", "file_path": T2, "commit": "f1", "outcome": "failed", "ran_in": "laptop", "with_anchor_reverted": false }));
    let ms = criterion_admissible(&View::new(&log));
    let s = ms.iter().find(|m| m.kind == "fingerprint_stamped").expect("the binary stamps c2 next");
    assert_eq!(s.fields.get("criterion_id"), Some(&json!("c2")));
    assert_eq!(s.fields.get("proof_not_tampered"), Some(&json!("S2")), "c2's stamp carries c2's own fingerprint");
    let log = run(&commit(&two_heads(), "f1", &[T1, T2], true, &[], &[("c1", "S1"), ("c2", "S2")]), "c1", "f1", "failed");
    let log = run(&log, "c2", "f1", "failed");
    assert_eq!(proof_of(&log, "c1").as_deref(), Some("S1"));
    assert_eq!(proof_of(&log, "c2").as_deref(), Some("S2"));
    let v = View::new(&log);
    assert_eq!(state(&v, "c1"), "sealed");
    assert_eq!(state(&v, "c2"), "sealed");
}

#[test]
fn an_edit_of_c1s_test_leaves_c2s_seal_standing() {
    let log = both_green();
    let v = View::new(&log);
    assert_eq!(state(&v, "c1"), "green");
    assert_eq!(state(&v, "c2"), "green");
    let c2_seal = seal_raw(&v, "c2", Place::Laptop).map(|s| s.t);
    // c1's test must change: announced after green, then edited alone
    let log = ap(&log, "test_change_announced", json!({ "criterion_id": "c1", "reason": "a sibling's wording broke an assertion" }));
    let log = commit(&log, "f3", &[T1], true, &[], &[("c1", "S1x"), ("c2", "S2")]);
    let v = View::new(&log);
    assert_eq!(tests_hash_at(&v, "c1", "f3"), Some("S1x"));
    assert_eq!(tests_hash_at(&v, "c2", "f3"), Some("S2"), "c1's edit is not c2's");
    assert_eq!(seal_raw(&v, "c2", Place::Laptop).map(|s| s.t), c2_seal, "c2's seal stands");
    assert!(green_raw(&v, "c2", Place::Laptop), "c2 stays green");
    assert_eq!(test_change(&v, "c2"), None);
    assert_eq!(test_change(&v, "c1"), Some("announced"));
    // the edit run and stamped: c1 green again under S1x, c2 still green under S2
    let log = run(&log, "c1", "f3", "passed");
    let v = View::new(&log);
    assert_eq!(state(&v, "c1"), "green");
    assert_eq!(state(&v, "c2"), "green");
    assert_eq!(proof_of(&log, "c2").as_deref(), Some("S2"));
}

#[test]
fn what_to_read_names_every_criterions_files() {
    let log = commit(&two_heads(), "f1", &[T1, T2], true, &[], &[("c1", "S1"), ("c2", "S2")]);
    let w = what_to_read(&log);
    assert_eq!(w.all_tests, vec![("c1".to_string(), vec![T1.to_string()]), ("c2".to_string(), vec![T2.to_string()])]);
}
