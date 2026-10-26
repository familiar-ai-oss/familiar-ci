//! The owner (2026-10-11) [reading]: another criterion's commit that also
//! names c's lines (`anchor_overlap` names more than one criterion, c among
//! them), landing before c's start check, is not c's code: c has not
//! started; its start check maps the lines or raises anchor_moved.

use familiar_ai_facts::{append_raw, Fact, Log};
use familiar_ai_rules::criterion::{overlaps, sibling_before_start};
use familiar_ai_rules::sample::criterion_fixture;
use familiar_ai_rules::View;
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";

fn ap(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { panic!("fields must be an object") };
    append_raw(log, kind, m, AT, TENANT).unwrap_or_else(|e| panic!("append {kind}: {e}"))
}

fn commit(log: &Log, id: &str, overlap: &[&str]) -> Log {
    ap(log, "commit_seen", json!({ "commit": id, "files": ["src/login.py", "src/mail.py"], "touches_tests": false, "touches_code": true, "anchor_overlap": overlap }))
}

fn start(log: &Log) -> Log {
    ap(log, "anchor_sha_matched", json!({ "criterion_id": "c2", "file_path": "src/mail.py", "symbol": "reset_link", "line_start": 10, "line_end": 30, "sha": "k9", "lines_hash": "L0", "matched": true, "checked_at": "start", "tip_matched": true }))
}

fn last_commit(log: &Log) -> &Fact {
    log.iter().rev().find(|f| f.kind == "commit_seen").expect("a commit")
}

fn base() -> Log {
    criterion_fixture(&["c1", "c2"], true, false, AT, TENANT).expect("fixture")
}

#[test]
fn a_commit_naming_two_criteria_before_c_starts_is_not_cs_code() {
    let log = commit(&base(), "k1", &["c1", "c2"]);
    let v = View::new(&log);
    let f = last_commit(&log);
    assert!(sibling_before_start(&v, "c2", f));
    assert!(!overlaps(&v, "c2", f), "not c2's code");
}

#[test]
fn after_cs_start_check_it_is_cs_code_again() {
    let log = commit(&start(&base()), "k2", &["c1", "c2"]);
    let v = View::new(&log);
    let f = last_commit(&log);
    assert!(!sibling_before_start(&v, "c2", f));
    assert!(overlaps(&v, "c2", f));
}

#[test]
fn a_commit_naming_c_alone_is_cs_code() {
    let log = commit(&base(), "k1", &["c2"]);
    let v = View::new(&log);
    let f = last_commit(&log);
    assert!(!sibling_before_start(&v, "c2", f));
    assert!(overlaps(&v, "c2", f));
}
