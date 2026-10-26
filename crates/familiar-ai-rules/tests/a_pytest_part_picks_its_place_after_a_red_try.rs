//! Test first, then pick the spot from what the test ran (the owner's plan of 2026-10-23, the
//! shared interface) [draft names]. For a part whose tests run under pytest, line 4 holds when its
//! place is chosen, the latest `trace_listed` of the part is red and newer than its latest
//! `test_planned`, and the place names a function on the list (or carries a later
//! `off_list_reason`). New work is exempt from the list, not from the try. A try that passed holds
//! `try_not_red`; no try holds `try_owed`; both are the agent's. A red try that listed nothing
//! leaves the place free, as today. A part whose tests-first commit was taken (a record from before
//! the try) is read as before.

mod traced;

use familiar_ai_rules::{plan, trace, View};
use serde_json::json;
use traced::*;

fn line_4(log: &familiar_ai_facts::Log) -> bool {
    plan::line_4(&View::new(log))
}

#[test]
fn a_place_chosen_before_any_try_does_not_hold_line_four() {
    let log = placed(&planned(), CODE, "Writer.write_rows", (10, 30));
    assert!(!line_4(&log), "the place waits for a red try");
    assert!(!plan::planning_confirm_complete(&View::new(&log)));
    assert!(whys(&log).contains(&trace::TRY_OWED.to_string()), "{:?}", whys(&log));
    // the code still waits, naming the try
    assert_eq!(hold_of(&edit(&log, CODE)).as_deref(), Some(trace::TRY_OWED));
}

#[test]
fn a_try_that_passed_holds_try_not_red_and_the_place_waits() {
    let log = tried(&planned(), false, json!([]));
    assert!(whys(&log).contains(&trace::TRY_NOT_RED.to_string()), "{:?}", whys(&log));
    assert!(!whys(&log).contains(&trace::TRY_OWED.to_string()));
    let log = placed(&log, CODE, "Writer.write_rows", (10, 30));
    assert!(!line_4(&log));
    assert_eq!(hold_of(&edit(&log, CODE)).as_deref(), Some(trace::TRY_NOT_RED));
    // the test may still be changed so it fails
    assert!(edit(&log, TEST).allowed());
}

#[test]
fn a_red_try_then_a_place_on_its_list_holds_line_four_and_completes_the_plan() {
    let log = tried(&planned(), true, top());
    assert!(!whys(&log).contains(&trace::TRY_OWED.to_string()) && !whys(&log).contains(&trace::TRY_NOT_RED.to_string()), "{:?}", whys(&log));
    assert!(!line_4(&log), "no place yet");
    let log = placed(&log, CODE, "Writer.write_rows", (10, 30));
    assert!(line_4(&log));
    assert!(plan::planning_confirm_complete(&View::new(&log)), "all eleven lines");
    assert!(edit(&log, CODE).allowed());
}

#[test]
fn a_red_try_older_than_the_latest_test_planned_is_no_try_of_the_tests_planned_now() {
    let log = tried(&planned(), true, top());
    let log = raw(&log, "test_planned", json!({ "criterion_id": "c1", "file_path": "tests/test_export_more.py", "will": "create" }));
    let log = placed(&log, CODE, "Writer.write_rows", (10, 30));
    assert!(!line_4(&log));
    assert!(whys(&log).contains(&trace::TRY_OWED.to_string()), "{:?}", whys(&log));
    let log = tried(&log, true, top());
    assert!(line_4(&log));
}

#[test]
fn the_latest_try_is_the_one_read() {
    let red_then_passed = tried(&tried(&planned(), true, top()), false, json!([]));
    let log = placed(&red_then_passed, CODE, "Writer.write_rows", (10, 30));
    assert!(!line_4(&log), "the latest try passed");
    let log = tried(&log, true, top());
    assert!(line_4(&log));
}

#[test]
fn a_place_names_a_listed_function_by_its_file_and_its_name_or_lines() {
    let at = |file: &str, symbol: &str, lines: (u64, u64)| line_4(&placed(&tried(&planned(), true, top()), file, symbol, lines));
    // the listed name, its bare name, or a longer qualified name ending in it
    assert!(at(CODE, "Writer.write_rows", (10, 30)));
    assert!(at(CODE, "write_rows", (12, 14)));
    assert!(at(CODE, "export.Writer.write_rows", (10, 30)));
    assert!(at(CODE, "Writer::write_rows", (10, 30)));
    assert!(at(CODE, "header", (32, 40)));
    // lines inside a listed function, whatever the symbol says
    assert!(at(CODE, "the loop over rows", (15, 18)));
    // another class's method of the same name, another file, a name not listed
    assert!(!at(CODE, "Reader.write_rows", (50, 60)));
    assert!(!at("src/other.py", "write_rows", (10, 30)));
    assert!(!at(CODE, "footer", (50, 60)));
    // lines that only overlap a listed function are not inside it
    assert!(!at(CODE, "footer", (25, 45)));
    assert!(trace::names_match("Writer.write_rows", "write_rows"));
    assert!(trace::names_match("write_rows()", "Writer.write_rows"));
    assert!(!trace::names_match("Reader.write_rows", "Writer.write_rows"));
    assert!(!trace::names_match("", "write_rows"));
}

#[test]
fn every_place_of_the_anchor_is_on_the_list_or_the_reason_is_owed() {
    let log = tried(&planned(), true, top());
    let two = json!([
        { "file_path": CODE, "symbol": "Writer.write_rows", "line_start": 10, "line_end": 30 },
        { "file_path": "src/template.py", "symbol": "render", "line_start": 1, "line_end": 9 }
    ]);
    let log = raw(&log, "place_chosen", json!({ "criterion_id": "c1", "places": two }));
    assert!(!line_4(&log), "the second place is off the list");
    assert_eq!(trace::off_list(&View::new(&log), "c1").len(), 1);
}

#[test]
fn a_red_try_that_listed_nothing_leaves_the_place_free() {
    let log = placed(&tried(&planned(), true, json!([])), "src/anywhere.py", "anything", (1, 2));
    assert!(line_4(&log));
}

#[test]
fn new_work_needs_the_red_try_and_not_the_list() {
    let spot = |log: &familiar_ai_facts::Log| raw(log, "new_work_spot_chosen", json!({ "criterion_id": "c1", "file_path": "src/new.py", "symbol": "dedupe", "beside": "write_rows" }));
    assert!(!line_4(&spot(&planned())), "no try yet");
    assert!(line_4(&spot(&tried(&planned(), true, top()))), "not on the list, and need not be");
    let none = raw(&tried(&planned(), true, top()), "new_work_no_spot", json!({ "criterion_id": "c1", "integration_point": "src/new.py:main" }));
    assert!(line_4(&none));
}

#[test]
fn a_part_whose_tests_were_committed_reads_as_before() {
    let log = placed(&planned(), CODE, "Writer.write_rows", (10, 30));
    assert!(!line_4(&log));
    let log = raw(&log, "test_written", json!({ "criterion_id": "c1", "file_path": TEST, "test_name": "test_rows_once" }));
    assert!(line_4(&log), "a record from before the try keeps its place");
    assert!(!whys(&log).contains(&trace::TRY_OWED.to_string()));
}

#[test]
fn a_light_part_documentation_only_needs_no_try() {
    let mut log = planned();
    for f in log.iter_mut().filter(|f| f.kind == "criterion") {
        f.fields.insert("path".into(), json!("light"));
    }
    let v = View::new(&log);
    assert!(trace::traced(&v, "c1") && !trace::needs_try(&v, "c1"));
    assert!(!whys(&log).contains(&trace::TRY_OWED.to_string()));
    let log = placed(&log, "docs/export.md", "Rows", (1, 9));
    assert!(line_4(&log), "its place is picked as today");
}
