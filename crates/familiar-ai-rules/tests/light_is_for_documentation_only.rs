//! Light is for documentation only (the owner, 2026-10-11): plan line 2 holds
//! for a light criterion only when the binary checked, after its anchor, that
//! every spot of the anchor is documentation (`anchor_is_documentation`,
//! matched). A false check is refused in the owner's words; the cure is the
//! criterion claimed again `--path full`. The vectors are the full judge;
//! these tests are the door's own teeth.

use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::hooks::Clears;
use familiar_ai_rules::sample::{criterion_fixture, plan_admissible};
use familiar_ai_rules::{decide, plan, terminal, Decision, Event, View};
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";
const WHY: &str = "its path is light but its anchor is not documentation: claim it --path full";

fn ap(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { panic!("fields must be an object") };
    append_raw(log, kind, m, AT, TENANT).unwrap_or_else(|e| panic!("append {kind}: {e}"))
}

const PLACE: &str = r#"{"file_path":"src/mail.py","symbol":"reset_link","line_start":10,"line_end":30}"#;

/// The intent and one light criterion c2 on the sample's place, its lines
/// checked; every other line of the plan in place.
fn light_c2() -> Log {
    let place: Value = serde_json::from_str(PLACE).expect("place");
    let mut log = ap(&Vec::new(), "intent_submitted", json!({ "text": "reset", "sha": "base" }));
    log = ap(&log, "criterion", json!({ "criterion_id": "c2", "text": "a reset link reaches the person", "path": "light" }));
    log = ap(&log, "place_chosen", json!({ "criterion_id": "c2", "places": [place.clone()] }));
    let mut check = place.as_object().expect("object").clone();
    check.insert("criterion_id".into(), json!("c2"));
    check.extend(json!({ "sha": "base", "lines_hash": "L0", "matched": true, "checked_at": "planning" }).as_object().expect("o").clone());
    log = ap(&log, "anchor_sha_matched", Value::Object(check));
    log = ap(&log, "test_planned", json!({ "criterion_id": "c2", "file_path": "tests/test_c2.py", "will": "create" }));
    log = ap(&log, "test_stack_detected", json!({ "codebase_id": "cb1", "project_root": "", "test_stack": "pytest", "test_command": "pytest -q" }));
    // definition change (the owner's plan of 2026-10-23; owner approved 2026-10-23) [draft]: a
    // pytest part on the full path holds line 4 after a red try that lists its place
    log = ap(&log, "trace_listed", json!({ "criterion_id": "c2", "run_id": "try-1", "sha": "base", "red": true, "path_size": 1, "ranker": "words", "query": "reset", "top": [{"rank": 1, "file_path": "src/mail.py", "symbol": "reset_link", "line_start": 10, "line_end": 30}] }));
    log = ap(&log, "criterion_head", json!({ "criterion_id": "c2" }));
    log = ap(&log, "criterion_assigned", json!({ "criterion_id": "c2", "agent_conversation_id": "a1" }));
    ap(&log, "fix_location_category", json!({ "criterion_id": "c2", "category": "this_repository", "where": "" }))
}

fn doc(log: &Log, matched: bool) -> Log {
    let place: Value = serde_json::from_str(PLACE).expect("place");
    let mut f = place.as_object().expect("object").clone();
    f.insert("criterion_id".into(), json!("c2"));
    f.insert("sha".into(), json!("base"));
    f.insert("matched".into(), json!(matched));
    ap(log, "anchor_is_documentation", Value::Object(f))
}

#[test]
fn line_2_holds_for_a_light_criterion_only_with_a_true_documentation_check() {
    let none = light_c2();
    assert!(!plan::line_2(&View::new(&none)), "no check: line 2 is missing");
    assert!(!plan::documented(&View::new(&none), "c2"));
    assert!(!plan::not_documentation(&View::new(&none), "c2"), "not checked is not 'not documentation'");
    let no = doc(&none, false);
    assert!(!plan::line_2(&View::new(&no)), "a false check: line 2 is missing");
    assert!(plan::not_documentation(&View::new(&no), "c2"));
    let yes = doc(&none, true);
    assert!(plan::line_2(&View::new(&yes)), "a true check: line 2 holds");
    assert!(plan::planning_confirm_complete(&View::new(&yes)), "and planning confirms without line 3");
}

#[test]
fn a_check_before_the_latest_anchor_does_not_count() {
    let yes = doc(&light_c2(), true);
    let place: Value = serde_json::from_str(PLACE).expect("place");
    let again = ap(&yes, "place_chosen", json!({ "criterion_id": "c2", "places": [place] }));
    assert!(!plan::documented(&View::new(&again), "c2"), "the check came before the exit");
}

#[test]
fn a_full_criterion_asks_nothing_new() {
    let log = ap(&light_c2(), "behavior", json!({ "behavior_id": "b1", "text": "b" }));
    let log = ap(&log, "promise", json!({ "promise_id": "p1", "behavior_id": "b1", "accepts": "a", "gives_back": "g", "refuses": "r" }));
    let log = ap(&log, "criterion", json!({ "criterion_id": "c2", "text": "t", "path": "full", "behavior_id": "b1", "promise_id": "p1" }));
    let v = View::new(&log);
    assert!(plan::line_2(&v), "full: no documentation check asked");
    assert!(plan::planning_confirm_complete(&v));
}

#[test]
fn a_new_work_anchor_is_one_spot_its_file_alone() {
    let mut log = ap(&Vec::new(), "intent_submitted", json!({ "text": "reset", "sha": "base" }));
    log = ap(&log, "criterion", json!({ "criterion_id": "c2", "text": "t", "path": "light" }));
    log = ap(&log, "new_work_no_spot", json!({ "criterion_id": "c2", "integration_point": "docs/guide.md:Usage" }));
    let a = plan::anchor_of(&View::new(&log), "c2").expect("anchor").clone();
    let spots = plan::doc_spots_of(&a).expect("a file");
    assert_eq!(spots.len(), 1);
    assert_eq!(spots[0].file_path, "docs/guide.md");
    assert_eq!((spots[0].symbol.as_deref(), spots[0].line_start, spots[0].line_end), (None, None, None));
    let log = ap(&log, "anchor_is_documentation", json!({ "criterion_id": "c2", "file_path": "docs/guide.md", "sha": "base", "matched": true }));
    assert!(plan::documented(&View::new(&log), "c2"));
}

#[test]
fn the_owed_check_and_its_cure_are_offered() {
    let none = light_c2();
    let ms = plan_admissible(&View::new(&none));
    let docs: Vec<_> = ms.iter().filter(|m| m.kind == "anchor_is_documentation").collect();
    assert_eq!(docs.len(), 2, "the binary's check and the WRONG one: {docs:?}");
    assert!(docs.iter().any(|m| m.fields.get("matched") == Some(&json!(true))));
    assert!(docs.iter().any(|m| m.fields.get("matched") == Some(&json!(false))));
    assert!(!docs[0].fields.contains_key("lines_hash") && !docs[0].fields.contains_key("checked_at"));
    // the hook names the check as line 2's supply
    let Decision::Refuse(r) = decide(&none, &Event::FirstEditOrCommit, AT) else { panic!("refused") };
    assert_eq!(r.hold.as_deref(), Some("planning_incomplete"));
    let Clears::Moves(cl) = &r.clears else { panic!("moves") };
    assert!(cl.iter().any(|m| m.get("kind") == Some(&json!("anchor_is_documentation"))), "{cl:?}");
    // a false check: the re-claim --path full is offered
    let no = doc(&none, false);
    let ms = plan_admissible(&View::new(&no));
    assert!(ms.iter().any(|m| m.kind == "criterion" && m.fields.get("criterion_id") == Some(&json!("c2")) && m.fields.get("path") == Some(&json!("full"))), "{ms:?}");
}

#[test]
fn the_refusal_names_line_2_in_the_owners_words() {
    let no = doc(&light_c2(), false);
    let text = terminal::refusal(&View::new(&no), &Event::FirstEditOrCommit).expect("refused");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "refused · criterion c2 · held: planning_incomplete", "{text}");
    assert_eq!(lines[1], format!("  {WHY}"), "{text}");
    assert!(lines[2].starts_with("  to clear it: "), "{text}");
    // not checked yet: the plain planning words, never the owner's sentence
    let text = terminal::refusal(&View::new(&light_c2()), &Event::FirstEditOrCommit).expect("refused");
    assert!(!text.contains(WHY), "{text}");
}

#[test]
fn the_sample_fixture_carries_the_check_for_its_light_criterion() {
    let log = criterion_fixture(&["c1", "c2"], true, false, AT, TENANT).expect("fixture");
    let v = View::new(&log);
    assert!(plan::planning_confirm_complete(&v));
    let d: Vec<_> = v.kind("anchor_is_documentation").collect();
    assert_eq!(d.len(), 1, "c2 is light, c1 full");
    assert_eq!(d[0].criterion_id(), Some("c2"));
    let green = criterion_fixture(&["c2"], true, true, AT, TENANT).expect("greenfield");
    let v = View::new(&green);
    let d: Vec<_> = v.kind("anchor_is_documentation").collect();
    assert_eq!(d.len(), 1);
    assert!(!d[0].fields.contains_key("symbol") && !d[0].fields.contains_key("line_start"), "a new-work anchor: its file alone");
}
