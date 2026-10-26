//! The start check is owed to a testing criterion only (the model,
//! 2026-10-11 18:52: `sibling_before_start` made the held-on-`not_stamped`
//! start check unneeded). A commit that names a criterion's lines alone,
//! before its turn, is that criterion's code landed before its test: it is
//! held on `not_stamped`, and no start check is offered for it.

use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::lifecycle::{holds, state};
use familiar_ai_rules::moves::criterion_admissible;
use familiar_ai_rules::sample::criterion_fixture;
use familiar_ai_rules::View;
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";

fn ap(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { panic!("fields must be an object") };
    append_raw(log, kind, m, AT, TENANT).unwrap_or_else(|e| panic!("append {kind}: {e}"))
}

/// The start checks the rules offer, by criterion.
fn start_checks_offered(log: &Log) -> Vec<String> {
    criterion_admissible(&View::new(log))
        .into_iter()
        .filter(|m| m.kind == "anchor_sha_matched" && m.fields.get("checked_at") == Some(&json!("start")))
        .filter_map(|m| m.fields.get("criterion_id").and_then(Value::as_str).map(str::to_string))
        .collect()
}

/// c2 planned alone, at its turn.
fn c2_planned() -> Log {
    criterion_fixture(&["c2"], true, false, AT, TENANT).expect("fixture")
}

/// A commit of code on c2's lines alone lands before its tests.
fn c2_code_first() -> Log {
    ap(&c2_planned(), "commit_seen", json!({ "commit": "k1", "files": ["src/mail.py"], "touches_tests": false, "touches_code": true, "anchor_overlap": ["c2"] }))
}

#[test]
fn a_criterion_held_only_on_not_stamped_is_offered_no_start_check() {
    let log = c2_code_first();
    let v = View::new(&log);
    assert_eq!(state(&v, "c2"), "held", "c2's code landed before its test");
    let whys: Vec<String> = holds(&v, "c2").iter().map(|h| h.why.clone()).collect();
    assert_eq!(whys, ["not_stamped"], "held on not_stamped only");
    let offered = start_checks_offered(&log);
    assert!(!offered.iter().any(|c| c == "c2"), "no start check for a held c2: {offered:?}");
}

#[test]
fn the_testing_criterion_is_offered_its_start_check() {
    let log = c2_planned();
    let v = View::new(&log);
    assert_eq!(state(&v, "c2"), "testing");
    assert_eq!(start_checks_offered(&log), ["c2", "c2"], "the binary's start check and the WRONG one");
}
