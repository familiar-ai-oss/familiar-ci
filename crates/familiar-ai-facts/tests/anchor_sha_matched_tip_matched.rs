//! `tip_matched` on `anchor_sha_matched` (the owner, 2026-10-11): a bool on
//! the start check only, true when the working tree's lines at the mapped
//! range equal the lines at the record's tip commit; absent on the planning
//! check.

use familiar_ai_facts::*;
use serde_json::{json, Map, Value};

const AT: f64 = 1_700_000_000.0;
const TENANT: &str = "lane_a_tests";

fn fields(v: Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap_or_default()
}

fn check(at: &str) -> Value {
    json!({"criterion_id": "c2", "file_path": "src/mail.py", "symbol": "reset_link", "line_start": 10, "line_end": 30, "sha": "base", "lines_hash": "L0", "matched": true, "checked_at": at})
}

#[test]
fn the_form_registers_tip_matched_optional() {
    let row = form_row("anchor_sha_matched").expect("registered");
    assert!(row.fields.contains(&"tip_matched"));
    assert_eq!(row.optional, &["tip_matched"]);
}

#[test]
fn a_start_check_carries_tip_matched_and_a_planning_check_none() {
    let log = append_raw(&Vec::new(), "anchor_sha_matched", fields(check("planning")), AT, TENANT).expect("planning");
    let p: AnchorShaMatched = log[0].fields_as().expect("typed");
    assert_eq!(p.tip_matched, None);
    assert!(!log[0].fields.contains_key("tip_matched"));
    let mut start = check("start");
    start["tip_matched"] = json!(true);
    let log = append_raw(&log, "anchor_sha_matched", fields(start), AT, TENANT).expect("start");
    let s: AnchorShaMatched = log[1].fields_as().expect("typed");
    assert_eq!(s.tip_matched, Some(true));
}

#[test]
fn tip_matched_is_a_bool() {
    let mut start = check("start");
    start["tip_matched"] = json!("yes");
    assert!(append_raw(&Vec::new(), "anchor_sha_matched", fields(start), AT, TENANT).is_err());
}
