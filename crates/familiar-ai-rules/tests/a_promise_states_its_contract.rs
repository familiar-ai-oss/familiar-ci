//! A promise may state its contract [draft names]: `cases`, each `{case_id, kind, given,
//! expect}` with `kind` one of normal, opposite or edge, and `opposite_none`, the reason a
//! promise has no opposite case. A promise with no `cases` reads as it always did, so a plan
//! made before the contract stays whole. A promise that gives `cases` holds line 3 only with
//! at least one normal case and one opposite case (the same call with the trigger absent), or
//! with `opposite_none` saying why there is none. Edge cases are optional.

use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::{plan, View};
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";

fn ap(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { panic!("fields must be an object") };
    append_raw(log, kind, m, AT, TENANT).unwrap_or_else(|e| panic!("append {kind}: {e}"))
}

fn try_ap(log: &Log, kind: &str, fields: Value) -> bool {
    let Value::Object(m) = fields else { panic!("fields must be an object") };
    append_raw(log, kind, m, AT, TENANT).is_ok()
}

/// Every plan line but the promise, for one full criterion c1 that names b1 and p1.
fn plan_without_the_promise() -> Log {
    let place = json!({"file_path": "src/app.py", "symbol": "f", "line_start": 1, "line_end": 9});
    let mut log = ap(&Vec::new(), "intent_submitted", json!({ "text": "t", "sha": "base" }));
    log = ap(&log, "behavior", json!({ "behavior_id": "b1", "text": "b" }));
    log = ap(&log, "criterion", json!({ "criterion_id": "c1", "text": "t", "path": "full", "behavior_id": "b1", "promise_id": "p1" }));
    log = ap(&log, "place_chosen", json!({ "criterion_id": "c1", "places": [place.clone()] }));
    let mut check = place.as_object().expect("object").clone();
    check.insert("criterion_id".into(), json!("c1"));
    check.extend(json!({ "sha": "base", "lines_hash": "L0", "matched": true, "checked_at": "planning" }).as_object().expect("o").clone());
    log = ap(&log, "anchor_sha_matched", Value::Object(check));
    log = ap(&log, "test_planned", json!({ "criterion_id": "c1", "file_path": "tests/test_f.py", "will": "create" }));
    log = ap(&log, "test_stack_detected", json!({ "codebase_id": "cb1", "project_root": "", "test_stack": "pytest", "test_command": "pytest -q" }));
    // definition change (the owner's plan of 2026-10-23; owner approved 2026-10-23) [draft]: a
    // pytest part's place holds line 4 after a red try that lists it
    log = ap(&log, "trace_listed", json!({ "criterion_id": "c1", "run_id": "try-1", "sha": "base", "red": true, "path_size": 1, "ranker": "words", "query": "t", "top": [{"rank": 1, "file_path": "src/app.py", "symbol": "f", "line_start": 1, "line_end": 9}] }));
    log = ap(&log, "criterion_head", json!({ "criterion_id": "c1" }));
    log = ap(&log, "criterion_assigned", json!({ "criterion_id": "c1", "agent_conversation_id": "a1" }));
    ap(&log, "fix_location_category", json!({ "criterion_id": "c1", "category": "this_repository", "where": "" }))
}

fn promise(extra: Value) -> Value {
    let mut p = json!({ "promise_id": "p1", "behavior_id": "b1", "accepts": "an input", "gives_back": "a result", "refuses": "a bad input" });
    p.as_object_mut().expect("o").extend(extra.as_object().expect("o").clone());
    p
}

fn case(id: &str, kind: &str) -> Value {
    json!({ "case_id": id, "kind": kind, "given": format!("the call for {id}"), "expect": format!("the result for {id}") })
}

fn line_3_with(extra: Value) -> bool {
    let log = ap(&plan_without_the_promise(), "promise", promise(extra));
    plan::line_3(&View::new(&log))
}

/// Line 3 with the promise `extra`, once the case-finder found nothing and the person kept the
/// plan (the cases card, the owner's plan of 2026-10-25) [draft names].
fn line_3_kept(extra: Value) -> bool {
    let mut log = ap(&plan_without_the_promise(), "promise", promise(extra));
    log = ap(&log, "cases_found", json!({ "criterion_id": "c1", "outcome": "none", "cases": [], "why": "the planned cases cover it", "model": "claude-sonnet-5-5", "tokens": 1 }));
    log = ap(&log, "hold_answered", json!({ "criterion_id": "c1", "hold": familiar_ai_rules::cases::CARD, "choice": familiar_ai_rules::cases::KEEP, "typed": "" }));
    plan::line_3(&View::new(&log))
}

#[test]
fn a_promise_without_cases_reads_as_before() {
    assert!(line_3_with(json!({})), "the old promise holds line 3");
    let log = ap(&plan_without_the_promise(), "promise", promise(json!({})));
    assert!(plan::planning_confirm_complete(&View::new(&log)));
}

#[test]
fn the_facts_door_takes_the_cases_in_their_shape_only() {
    let log = plan_without_the_promise();
    assert!(try_ap(&log, "promise", promise(json!({ "cases": [case("n1", "normal"), case("o1", "opposite"), case("e1", "edge")] }))));
    assert!(try_ap(&log, "promise", promise(json!({ "cases": [case("n1", "normal")], "opposite_none": "the trigger cannot be absent" }))));
    assert!(!try_ap(&log, "promise", promise(json!({ "cases": [case("n1", "sideways")] }))), "kind is a closed set");
    assert!(!try_ap(&log, "promise", promise(json!({ "cases": [{ "case_id": "n1", "kind": "normal", "given": "g" }] }))), "expect is required");
    assert!(!try_ap(&log, "promise", promise(json!({ "cases": "n1 normal" }))), "a list, not text");
    assert!(!try_ap(&log, "promise", promise(json!({ "opposite_none": 3 }))), "the reason is text");
}

#[test]
fn line_3_wants_a_normal_case_and_an_opposite_case_or_the_reason_there_is_none() {
    assert!(line_3_with(json!({ "cases": [case("n1", "normal"), case("o1", "opposite")] })));
    assert!(line_3_with(json!({ "cases": [case("n1", "normal"), case("o1", "opposite"), case("e1", "edge")] })), "edges are optional, and welcome");
    assert!(line_3_with(json!({ "cases": [case("n1", "normal")], "opposite_none": "the trigger cannot be absent" })));
    assert!(!line_3_with(json!({ "cases": [case("n1", "normal")] })), "no opposite and no reason");
    assert!(!line_3_with(json!({ "cases": [case("n1", "normal")], "opposite_none": "  " })), "a blank reason is none");
    assert!(!line_3_with(json!({ "cases": [case("o1", "opposite")] })), "no normal case");
    assert!(!line_3_with(json!({ "cases": [case("e1", "edge"), case("o1", "opposite")] })), "an edge is not the normal case");
    assert!(!line_3_with(json!({ "cases": [] })), "cases given, none in them");
    assert!(!line_3_with(json!({ "cases": [case("n1", "normal"), case("n1", "opposite")] })), "each case_id once");
    let blank = json!({ "case_id": "n1", "kind": "normal", "given": "", "expect": "r" });
    assert!(!line_3_with(json!({ "cases": [blank, case("o1", "opposite")] })), "a case says what is given and what is expected");
}

#[test]
fn the_latest_promise_counts_and_its_cases_are_read_back() {
    let log = ap(&plan_without_the_promise(), "promise", promise(json!({ "cases": [case("n1", "normal")] })));
    assert!(!plan::line_3(&View::new(&log)));
    let log = ap(&log, "promise", promise(json!({ "cases": [case("n1", "normal"), case("o1", "opposite")] })));
    let v = View::new(&log);
    assert!(plan::line_3(&v), "the promise claimed again with its opposite");
    let cases = plan::cases_of(&v, "c1");
    assert_eq!(cases.iter().map(|k| (k.case_id.as_str(), k.kind.as_str())).collect::<Vec<_>>(), [("n1", "normal"), ("o1", "opposite")]);
    assert_eq!(cases[0].given, "the call for n1");
    assert_eq!(cases[0].expect, "the result for n1");
    assert_eq!(plan::contract_short(&v, "c1"), None, "a whole contract wants nothing");
    let short = ap(&log, "promise", promise(json!({ "cases": [case("n1", "normal")] })));
    assert_eq!(plan::contract_short(&View::new(&short), "c1").as_deref(), Some("an opposite case (the same call with the trigger absent), or --opposite_none with the reason there is none"));
    let none = ap(&log, "promise", promise(json!({ "cases": [case("o1", "opposite")] })));
    assert_eq!(plan::contract_short(&View::new(&none), "c1").as_deref(), Some("a normal case (the call that shows the behavior asked for)"));
}

// definition change (the coordinator's correction of 2026-10-21, step 1): the contract is not
// optional for new work. Every promise the claim door takes carries contract_version [draft name],
// and a promise that carries it holds line 3 only with its cases whole; a promise with no version
// and no cases (an old record, a committed plan) reads as before.
#[test]
fn a_promise_made_under_the_contract_needs_its_cases() {
    let version = json!({ "contract_version": plan::CONTRACT_VERSION });
    assert!(!line_3_with(version.clone()), "made under the contract, no cases: line 3 is missing");
    let log = ap(&plan_without_the_promise(), "promise", promise(version.clone()));
    assert_eq!(plan::contract_short(&View::new(&log), "c1").as_deref(), Some("a normal case (the call that shows the behavior asked for)"));
    // definition change (the owner's plan of 2026-10-25, the cases card; the owner's word 2026-10-25) [draft]:
    // a whole contract on a full-path part waits for the case-finder and the person's card, then holds line 3
    let mut whole = version.clone();
    whole["cases"] = json!([case("n1", "normal"), case("o1", "opposite")]);
    assert!(!line_3_with(whole.clone()), "a whole contract waits for the case-finder");
    assert!(line_3_kept(whole));
    let mut reason = version;
    reason["cases"] = json!([case("n1", "normal")]);
    reason["opposite_none"] = json!("the trigger cannot be absent");
    assert!(line_3_kept(reason));
    assert!(line_3_with(json!({})), "a promise written before the contract");
    assert!(!try_ap(&plan_without_the_promise(), "promise", promise(json!({ "contract_version": "one" }))), "a whole number");
}
