//! Each place of the anchor is checked at the start, and compared with its
//! own planning check; the anchor moved when any place's lines changed
//! (lane two-places, F7). Before it, only the first place got a start check,
//! and the latest start check was compared with the latest planning check
//! whatever place each was for, so a two-place anchor opened a false
//! `anchor_moved` at once.

use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::lifecycle::whys;
use familiar_ai_rules::moves::criterion_admissible;
use familiar_ai_rules::sample::{criterion_fixture, LINES_PLANNING};
use familiar_ai_rules::View;
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";
/// The second place's lines at planning, hashed apart from the first's.
const SECOND: &str = "L0b";

fn ap(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { panic!("fields must be an object") };
    append_raw(log, kind, m, AT, TENANT).unwrap_or_else(|e| panic!("append {kind}: {e}"))
}

fn first() -> Value {
    json!({ "file_path": "src/login.py", "symbol": "check_password", "line_start": 40, "line_end": 62 })
}

fn second() -> Value {
    json!({ "file_path": "src/session.py", "symbol": "open_session", "line_start": 5, "line_end": 20 })
}

fn with(place: &Value, more: Value) -> Value {
    let mut m = place.as_object().cloned().unwrap_or_default();
    m.insert("criterion_id".into(), json!("c1"));
    m.extend(more.as_object().cloned().unwrap_or_default());
    Value::Object(m)
}

/// c1 anchored on two places, each with its own planning check, their hashes apart.
fn two_places() -> Log {
    let mut log = criterion_fixture(&["c1"], true, false, AT, TENANT).expect("fixture");
    log = ap(&log, "place_chosen", json!({ "criterion_id": "c1", "places": [first(), second()] }));
    log = ap(&log, "anchor_sha_matched", with(&first(), json!({ "sha": "k0", "lines_hash": LINES_PLANNING, "matched": true, "checked_at": "planning" })));
    ap(&log, "anchor_sha_matched", with(&second(), json!({ "sha": "k0", "lines_hash": SECOND, "matched": true, "checked_at": "planning" })))
}

fn start(log: &Log, place: &Value, hash: &str) -> Log {
    ap(log, "anchor_sha_matched", with(place, json!({ "sha": "k0", "lines_hash": hash, "matched": true, "checked_at": "start", "tip_matched": true })))
}

/// The files the rules offer the binary's start check for (label `start_check`).
fn start_checks_offered(log: &Log) -> Vec<String> {
    criterion_admissible(&View::new(log))
        .into_iter()
        .filter(|m| m.kind == "anchor_sha_matched" && m.fields.get("checked_at") == Some(&json!("start")))
        .filter(|m| m.label.as_deref() == Some("start_check"))
        .filter_map(|m| m.fields.get("file_path").and_then(Value::as_str).map(str::to_string))
        .collect()
}

fn moved(log: &Log) -> bool {
    whys(&View::new(log), "c1").iter().any(|w| w == "anchor_moved")
}

#[test]
fn both_places_are_offered_a_start_check() {
    let offered = start_checks_offered(&two_places());
    assert!(offered.iter().any(|f| f == "src/login.py"), "the first place: {offered:?}");
    assert!(offered.iter().any(|f| f == "src/session.py"), "the second place: {offered:?}");
}

#[test]
fn with_only_the_first_place_checked_the_second_is_still_owed() {
    let log = start(&two_places(), &first(), LINES_PLANNING);
    assert_eq!(start_checks_offered(&log), ["src/session.py"], "the second place is never left unchecked");
    assert!(!moved(&log), "the first place's lines did not change: {:?}", whys(&View::new(&log), "c1"));
}

#[test]
fn both_matched_is_no_hold() {
    let log = start(&start(&two_places(), &first(), LINES_PLANNING), &second(), SECOND);
    assert!(start_checks_offered(&log).is_empty(), "nothing more is owed");
    assert!(!moved(&log), "each place is compared with its own planning check: {:?}", whys(&View::new(&log), "c1"));
}

#[test]
fn a_moved_hash_on_the_second_place_only_is_anchor_moved() {
    let log = start(&start(&two_places(), &first(), LINES_PLANNING), &second(), "LX");
    assert!(moved(&log), "the second place's lines changed");
}

#[test]
fn a_moved_hash_on_the_first_place_only_is_anchor_moved() {
    let log = start(&start(&two_places(), &first(), "LX"), &second(), SECOND);
    assert!(moved(&log), "the first place's lines changed, though the latest start check matched");
}
