//! The cases card (the owner's plan of 2026-10-25, §3) [draft names]: the person reads each
//! planned case by a short line the agent writes with it. A promise's case may carry `short`, one
//! line of text for the person; a case without one is read by its `given`.

use familiar_ai_facts::*;
use serde_json::{json, Map, Value};

const AT: f64 = 1.0;
const TENANT: &str = "_default_";

fn promise(cases: Value) -> Map<String, Value> {
    json!({ "promise_id": "p1", "behavior_id": "b1", "accepts": "a name", "gives_back": "a greeting", "refuses": "an empty name", "cases": cases, "contract_version": 1 })
        .as_object()
        .cloned()
        .unwrap_or_default()
}

#[test]
fn a_promise_case_may_carry_a_short_line_and_keeps_it() {
    let cases = json!([
        { "case_id": "n1", "kind": "normal", "short": "a plain name is greeted", "given": "the name Ada", "expect": "hello Ada" },
        { "case_id": "o1", "kind": "opposite", "given": "an empty name", "expect": "refused, no greeting" }
    ]);
    let log = append_raw(&Vec::new(), "promise", promise(cases), AT, TENANT).expect("a case with a short line, and one without");
    let typed: Vec<PromiseCase> = serde_json::from_value(log[0].fields["cases"].clone()).expect("the typed cases");
    assert_eq!(typed[0].short.as_deref(), Some("a plain name is greeted"));
    assert_eq!(typed[1].short, None, "short is optional");
    // written back, a case without a short line carries no field for it
    let back = serde_json::to_value(&typed[1]).expect("serialized");
    assert!(back.get("short").is_none(), "{back}");
}

#[test]
fn a_short_line_that_is_not_text_is_refused() {
    let cases = json!([{ "case_id": "n1", "kind": "normal", "short": 7, "given": "the name Ada", "expect": "hello Ada" }]);
    assert!(append_raw(&Vec::new(), "promise", promise(cases), AT, TENANT).is_err());
}
