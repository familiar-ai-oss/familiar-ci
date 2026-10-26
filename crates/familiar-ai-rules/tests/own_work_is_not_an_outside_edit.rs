//! Changes made by this plan's own earlier criteria are not outside edits
//! (the owner, 2026-10-11): the start check's lines differ from planning's,
//! yet no `anchor_moved` when the start check's `tip_matched` is true and
//! every commit after the planning check whose `anchor_overlap` names the
//! criterion is stamped (laptop, before the start check) for another
//! criterion of the plan that the commit's `anchor_overlap` also names.
//! `own_work_moved` names the latest such commit and its criterion.

use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::lifecycle::{own_work_moved, whys, OwnWork};
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

fn stamp(log: &Log, c: &str, id: &str) -> Log {
    ap(log, "fingerprint_stamped", json!({ "criterion_id": c, "commit": id, "proof_not_tampered": "S1", "ran_in": "laptop" }))
}

/// c2's start check: its lines at their new place, hashed `hash`.
fn start(log: &Log, hash: &str, tip_matched: bool) -> Log {
    ap(log, "anchor_sha_matched", json!({ "criterion_id": "c2", "file_path": "src/mail.py", "symbol": "reset_link", "line_start": 13, "line_end": 33, "sha": "k9", "lines_hash": hash, "matched": true, "checked_at": "start", "tip_matched": tip_matched }))
}

fn base() -> Log {
    criterion_fixture(&["c1", "c2"], true, false, AT, TENANT).expect("fixture")
}

fn moved(log: &Log) -> bool {
    whys(&View::new(log), "c2").iter().any(|w| w == "anchor_moved")
}

#[test]
fn another_criterions_stamped_commit_on_the_lines_is_not_anchor_moved() {
    let log = start(&stamp(&commit(&base(), "k1", &["c1", "c2"]), "c1", "k1"), "L1", true);
    assert_eq!(own_work_moved(&View::new(&log), "c2"), Some(OwnWork { commit: "k1".into(), criterion_id: "c1".into() }));
    assert!(!moved(&log));
}

#[test]
fn the_latest_of_several_stamped_commits_is_named() {
    let mut log = stamp(&commit(&base(), "k1", &["c1", "c2"]), "c1", "k1");
    log = stamp(&commit(&log, "k2", &["c1", "c2"]), "c1", "k2");
    let log = start(&log, "L1", true);
    assert_eq!(own_work_moved(&View::new(&log), "c2").map(|w| w.commit), Some("k2".into()));
    assert!(!moved(&log));
}

#[test]
fn an_uncommitted_edit_is_anchor_moved_even_over_own_work() {
    let log = start(&stamp(&commit(&base(), "k1", &["c1", "c2"]), "c1", "k1"), "L1", false);
    assert_eq!(own_work_moved(&View::new(&log), "c2"), None);
    assert!(moved(&log));
}

#[test]
fn a_commit_no_stamp_claims_is_anchor_moved() {
    let log = start(&commit(&base(), "k1", &["c1", "c2"]), "L1", true);
    assert_eq!(own_work_moved(&View::new(&log), "c2"), None, "no stamp");
    assert!(moved(&log));
    let one_unstamped = start(&commit(&stamp(&commit(&base(), "k1", &["c1", "c2"]), "c1", "k1"), "k2", &["c2"]), "L1", true);
    assert_eq!(own_work_moved(&View::new(&one_unstamped), "c2"), None, "every commit must be stamped");
    assert!(moved(&one_unstamped));
}

#[test]
fn a_stamp_must_be_another_criterions_that_the_commit_names_and_come_before_the_start() {
    let not_named = start(&stamp(&commit(&base(), "k1", &["c2"]), "c1", "k1"), "L1", true);
    assert_eq!(own_work_moved(&View::new(&not_named), "c2"), None, "the commit's overlap does not name c1");
    let own = start(&stamp(&commit(&base(), "k1", &["c1", "c2"]), "c2", "k1"), "L1", true);
    assert_eq!(own_work_moved(&View::new(&own), "c2"), None, "c2's own stamp is not another criterion's work");
    let late = stamp(&start(&commit(&base(), "k1", &["c1", "c2"]), "L1", true), "c1", "k1");
    assert_eq!(own_work_moved(&View::new(&late), "c2"), None, "stamped after the start check");
    let pipeline = start(&ap(&commit(&base(), "k1", &["c1", "c2"]), "fingerprint_stamped", json!({ "criterion_id": "c1", "commit": "k1", "proof_not_tampered": "S1", "ran_in": "pipeline" })), "L1", true);
    assert_eq!(own_work_moved(&View::new(&pipeline), "c2"), None, "a laptop stamp only");
}

#[test]
fn lines_that_did_not_change_are_not_own_work() {
    let log = start(&stamp(&commit(&base(), "k1", &["c1", "c2"]), "c1", "k1"), "L0", true);
    assert_eq!(own_work_moved(&View::new(&log), "c2"), None);
    assert!(!moved(&log));
}
