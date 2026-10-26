//! The cases card is mirrored (the owner, 2026-10-25: "Mirror them") [draft names]: a record
//! rebuilt from the committed plan (a fresh clone, a new lane, CI's verify) carries the
//! case-finder's finding (`cases_found`) and the person's answer on the cases card
//! (`hold_answered` on `cases_unconfirmed`), so it never asks again. The cases card's answers are
//! the only answers the plan carries: another card's answer stays out, on read and on write, and
//! so does an answer on the cases card that is not one of its own choices (a `release` there would
//! release the part wherever the plan is read).

mod common;

use common::fields;
use familiar_ai_facts::{append, append_raw, IntentSubmitted, Log};
use familiar_ai_plan::{claims_from_log, from_bytes, to_bytes, Claim, LineError, PlanError, PLAN_KINDS};
use serde_json::{json, Map, Value};

const CARD: &str = "cases_unconfirmed";

fn answer(hold: &str, choice: &str, typed: &str) -> Map<String, Value> {
    fields(&[("criterion_id", json!("c1")), ("hold", json!(hold)), ("choice", json!(choice)), ("typed", json!(typed))])
}

fn line_of(fields: &Map<String, Value>) -> String {
    let mut m = Map::new();
    m.insert("kind".to_string(), json!("hold_answered"));
    m.extend(fields.clone());
    format!("{}\n", Value::Object(m))
}

/// The answer is refused as a kind the plan does not carry, on write and on read.
fn refused_both_ways(fields: Map<String, Value>) {
    let text = line_of(&fields);
    match Claim::new("hold_answered", fields) {
        Err(PlanError::RefusedKind(k)) => assert_eq!(k, "hold_answered", "{text}"),
        other => panic!("expected RefusedKind on write for {text}, got {other:?}"),
    }
    match from_bytes(text.as_bytes()) {
        Err(PlanError::Line(e)) => assert_eq!(e, LineError::RefusedKind { line: 1, kind: "hold_answered".to_string() }, "{text}"),
        other => panic!("expected a refused kind on read for {text}, got {other:?}"),
    }
}

#[test]
fn the_finding_and_the_answer_on_the_cases_card_are_plan_kinds() {
    for kind in ["cases_found", "hold_answered"] {
        assert!(PLAN_KINDS.contains(&kind), "{kind}: {PLAN_KINDS:?}");
    }
}

/// Every choice of the cases card, words typed among them, is a claim the plan writes as it reads.
#[test]
fn each_answer_the_cases_card_takes_round_trips() {
    for (choice, typed) in [("add_all", ""), ("keep_planned", ""), ("look_again", ""), ("typed", "1 and 3"), ("typed", "a locked account is refused")] {
        let claim = Claim::new("hold_answered", answer(CARD, choice, typed)).unwrap_or_else(|e| panic!("{choice}: {e}"));
        let line = json!({ "kind": "hold_answered", "criterion_id": "c1", "hold": CARD, "choice": choice, "typed": typed }).to_string();
        assert_eq!(claim.line(), line, "kind first, then the form's order");
        assert_eq!(from_bytes(format!("{line}\n").as_bytes()).expect("read"), vec![claim]);
    }
}

/// The plan carries no other card's answer: a hand-written one would be read wherever the plan
/// is read as the person's (a release passing a part).
#[test]
fn another_cards_answer_is_refused_on_read_and_on_write() {
    for (hold, choice) in [("two_failures", "release"), ("crashed", "a_dependency"), ("anchor_untouched", "keep_going"), ("run_unclassified", "its_the_pass"), ("miscoped", "covered_by_dependency"), ("", "keep_planned")] {
        refused_both_ways(answer(hold, choice, ""));
    }
}

/// On the cases card itself only its own choices are mirrored: the rules read a `release` on any
/// hold as the part released.
#[test]
fn a_choice_the_cases_card_does_not_offer_is_refused_on_read_and_on_write() {
    for choice in ["release", "covered_by_dependency", "keep_going", "re_plan", ""] {
        refused_both_ways(answer(CARD, choice, ""));
    }
}

/// The mirror keeps every finding and every answer on the cases card in log order, the promise
/// claimed again after them, and leaves out the other cards' answers.
#[test]
fn the_mirror_keeps_the_findings_and_the_cases_cards_answers_in_log_order_and_no_other_answer() {
    let (at, tenant) = (1_700_000_000.0, "_default_");
    let fact = |log: &Log, kind: &str, v: Value| append_raw(log, kind, v.as_object().cloned().unwrap_or_default(), at, tenant).unwrap_or_else(|e| panic!("{kind}: {e}"));
    let case = |id: &str, kind: &str| json!({ "case_id": id, "kind": kind, "given": format!("given {id}"), "expect": format!("expect {id}") });
    let mut log: Log = append(&Vec::new(), &IntentSubmitted { text: "lock out after three tries".into(), sha: "s0".into(), for_t: None }, at, tenant).unwrap();
    log = fact(&log, "behavior", json!({ "behavior_id": "b1", "text": "the lock" }));
    log = fact(&log, "promise", json!({ "promise_id": "p1", "behavior_id": "b1", "accepts": "a login", "gives_back": "a session", "refuses": "a locked account", "cases": [case("n1", "normal")], "contract_version": 1 }));
    log = fact(&log, "criterion", json!({ "criterion_id": "c1", "text": "lock out after three tries", "path": "full", "behavior_id": "b1", "promise_id": "p1" }));
    log = fact(&log, "cases_found", json!({ "criterion_id": "c1", "outcome": "none", "cases": [], "why": "the planned cases cover it", "model": "sonnet", "tokens": 900 }));
    log = fact(&log, "hold_answered", answer(CARD, "look_again", "").into());
    log = fact(&log, "cases_found", json!({ "criterion_id": "c1", "outcome": "found", "cases": [{ "case_id": "a1", "short": "a locked account is refused", "given": "a locked account", "expect": "refused", "why": "the code checks the lock first" }], "why": "", "model": "sonnet", "tokens": 1500 }));
    log = fact(&log, "hold_answered", answer(CARD, "typed", "1").into());
    log = fact(&log, "promise", json!({ "promise_id": "p1", "behavior_id": "b1", "accepts": "a login", "gives_back": "a session", "refuses": "a locked account", "cases": [case("n1", "normal"), case("a1", "edge")], "contract_version": 1 }));
    // another card's answers, and the told of a pick: none is a plan line
    log = fact(&log, "hold_answered", answer("two_failures", "release", "").into());
    log = fact(&log, "hold_answered", answer("crashed", "a_dependency", "").into());
    log = fact(&log, "told", json!({ "about": "7" }));

    let claims = claims_from_log(&log);
    let read: Vec<String> = claims
        .iter()
        .map(|c| {
            let s = |k: &str| c.fields().get(k).and_then(Value::as_str).unwrap_or("").to_string();
            match c.kind() {
                "cases_found" => format!("cases_found {}", s("outcome")),
                "hold_answered" => format!("hold_answered {} {} {:?}", s("hold"), s("choice"), s("typed")),
                kind => kind.to_string(),
            }
        })
        .collect();
    assert_eq!(
        read,
        [
            "intent_submitted",
            "behavior",
            "promise",
            "criterion",
            "cases_found none",
            "hold_answered cases_unconfirmed look_again \"\"",
            "cases_found found",
            "hold_answered cases_unconfirmed typed \"1\"",
            "promise",
        ],
    );
    let mirrored: Vec<&Map<String, Value>> = log.iter().filter(|f| matches!(f.kind, "cases_found") || (f.kind == "hold_answered" && f.fields.get("hold") == Some(&json!(CARD)))).map(|f| &f.fields).collect();
    let lines: Vec<&Map<String, Value>> = claims.iter().filter(|c| matches!(c.kind(), "cases_found" | "hold_answered")).map(|c| c.fields()).collect();
    assert_eq!(lines, mirrored, "each mirrored as it was recorded, every field kept");
    assert_eq!(from_bytes(&to_bytes(&claims)).expect("read back"), claims);
}
