//! Test first, then pick the spot from what the test ran (the owner's plan of 2026-10-23, the
//! shared interface) [draft names]. After a try, the binary writes `trace_listed`: the part, the
//! try's run and sha, whether it was red, how many functions its failing tests ran, how they were
//! ranked, the query and at most 25 of them. The agent's `off_list_reason` names a part and its
//! one-line reason for a place that is not on the list.

use familiar_ai_facts::*;
use serde_json::{json, Map, Value};

const AT: f64 = 1.0;
const TENANT: &str = "_default_";

fn fields(v: Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap_or_default()
}

fn listed() -> Value {
    json!({
        "criterion_id": "c1",
        "run_id": "try-1",
        "sha": "abc1234",
        "red": true,
        "path_size": 40,
        "ranker": "words",
        "query": "the export repeats every row",
        "top": [
            {"rank": 1, "file_path": "src/export.py", "symbol": "Writer.write_rows", "line_start": 10, "line_end": 30},
            {"rank": 2, "file_path": "src/export.py", "symbol": "header", "line_start": 32, "line_end": 40}
        ]
    })
}

#[test]
fn trace_listed_round_trips_flat_and_typed() {
    let log = append_raw(&Vec::new(), "trace_listed", fields(listed()), AT, TENANT).expect("the binary's listing");
    let fact = &log[0];
    let typed: TraceListed = fact.fields_as().expect("its typed form");
    assert!(typed.red);
    assert_eq!(typed.top.len(), 2);
    assert_eq!(typed.top[0].symbol, "Writer.write_rows");
    assert_eq!(typed.top[1].line_end, 40);
    let again = append(&Vec::new(), &typed, AT, TENANT).expect("typed append");
    assert_eq!(again[0].fields, fact.fields, "the typed form keeps every field");
    // a try that passed, or recorded nothing, lists nothing
    let mut empty = listed();
    empty["red"] = json!(false);
    empty["top"] = json!([]);
    assert!(append_raw(&Vec::new(), "trace_listed", fields(empty), AT, TENANT).is_ok());
}

#[test]
fn trace_listed_refuses_a_missing_field_and_a_wrong_shape() {
    for k in ["criterion_id", "run_id", "sha", "red", "path_size", "ranker", "query", "top"] {
        let mut f = fields(listed());
        f.remove(k);
        assert!(append_raw(&Vec::new(), "trace_listed", f, AT, TENANT).is_err(), "{k} is required");
    }
    let mut bad = listed();
    bad["red"] = json!("yes");
    assert!(append_raw(&Vec::new(), "trace_listed", fields(bad), AT, TENANT).is_err());
    let mut bad = listed();
    bad["top"] = json!([{"rank": 1, "file_path": "src/export.py"}]);
    assert!(append_raw(&Vec::new(), "trace_listed", fields(bad), AT, TENANT).is_err(), "each listed function names its file, symbol and lines");
    let mut extra = fields(listed());
    extra.insert("guess".into(), json!("x"));
    assert!(append_raw(&Vec::new(), "trace_listed", extra, AT, TENANT).is_err());
}

#[test]
fn off_list_reason_names_the_part_and_its_reason() {
    let ok = json!({"criterion_id": "c1", "reason": "the header is written by a template the test never renders"});
    let log = append_raw(&Vec::new(), "off_list_reason", fields(ok), AT, TENANT).expect("the agent's reason");
    let typed: OffListReason = log[0].fields_as().expect("its typed form");
    assert_eq!(typed.criterion_id, "c1");
    for k in ["criterion_id", "reason"] {
        let mut f = fields(json!({"criterion_id": "c1", "reason": "r"}));
        f.remove(k);
        assert!(append_raw(&Vec::new(), "off_list_reason", f, AT, TENANT).is_err(), "{k} is required");
    }
}

#[test]
fn both_kinds_are_registered_after_one_reading() {
    let at = |k: &str| KINDS.iter().position(|x| *x == k);
    assert!(at("one_reading") < at("trace_listed") && at("trace_listed").is_some());
    assert_eq!(at("off_list_reason"), at("trace_listed").map(|i| i + 1));
    assert_eq!(form_row("trace_listed").map(|r| r.optional.len()), Some(0));
    assert_eq!(form_row("off_list_reason").map(|r| r.fields.to_vec()), Some(vec!["criterion_id", "reason"]));
    assert_eq!(kind_count(), FORM.len());
}
