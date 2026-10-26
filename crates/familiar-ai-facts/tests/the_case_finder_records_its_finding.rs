//! The cases card (the owner's plan of 2026-10-25) [draft names]. Once a full-path part's contract
//! is whole, the case-finder subagent reads the person's words and the code and lists the cases the
//! plan's own do not cover; the binary records its answer as `cases_found`: the part, the outcome
//! (`found`, `none` or `failed`), the cases (each its id, a short line for the person, what is given,
//! what is expected and why), the why of an empty list, the model that ran and the tokens it used.

use familiar_ai_facts::*;
use serde_json::{json, Map, Value};

const AT: f64 = 1.0;
const TENANT: &str = "_default_";

fn fields(v: Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap_or_default()
}

fn found() -> Value {
    json!({
        "criterion_id": "c1",
        "outcome": "found",
        "cases": [
            {"case_id": "a1", "short": "the same with the other --tb styles", "given": "an all-hidden group under --tb=short, line and native", "expect": "the hidden-entries message in each", "why": "the person's report names no style; the code branches on each"},
            {"case_id": "a2", "short": "a group inside another group", "given": "a nested group whose frames are all hidden", "expect": "the message, no IndexError", "why": "repr_excinfo recurses into nested groups"}
        ],
        "why": "",
        "model": "claude-sonnet-5-5",
        "tokens": 5320
    })
}

#[test]
fn cases_found_round_trips_flat_and_typed() {
    let log = append_raw(&Vec::new(), "cases_found", fields(found()), AT, TENANT).expect("the case-finder's finding");
    let fact = log.last().unwrap();
    let typed: CasesFound = fact.fields_as().expect("its typed form");
    assert_eq!(typed.outcome, "found");
    assert_eq!(typed.cases.len(), 2);
    assert_eq!(typed.cases[0].case_id, "a1");
    assert_eq!(typed.cases[1].short, "a group inside another group");
    let mut none = found();
    none["outcome"] = json!("none");
    none["cases"] = json!([]);
    none["why"] = json!("the planned cases already cover every path");
    assert!(append_raw(&Vec::new(), "cases_found", fields(none), AT, TENANT).is_ok());
}

#[test]
fn cases_found_refuses_a_missing_field_and_a_wrong_shape() {
    for k in ["criterion_id", "outcome", "cases", "why", "model", "tokens"] {
        let mut f = fields(found());
        f.remove(k);
        assert!(append_raw(&Vec::new(), "cases_found", f, AT, TENANT).is_err(), "{k} is required");
    }
    let mut bad = found();
    bad["cases"] = json!("a1");
    assert!(append_raw(&Vec::new(), "cases_found", fields(bad), AT, TENANT).is_err());
    let mut bad = found();
    bad["cases"] = json!([{"case_id": "a1", "short": "s"}]);
    assert!(append_raw(&Vec::new(), "cases_found", fields(bad), AT, TENANT).is_err(), "each case names its id, short line, given, expect and why");
    let mut extra = fields(found());
    extra.insert("verdict".into(), json!("yes"));
    assert!(append_raw(&Vec::new(), "cases_found", extra, AT, TENANT).is_err());
}

#[test]
fn cases_found_comes_last_in_the_form() {
    let at = |k: &str| KINDS.iter().position(|x| *x == k);
    assert_eq!(at("cases_found"), Some(KINDS.len() - 1));
    assert_eq!(at("off_list_reason").map(|i| i + 1), at("cases_found"));
    assert_eq!(form_row("cases_found").map(|r| r.optional.len()), Some(0));
}
