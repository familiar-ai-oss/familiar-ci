//! Test first, then pick the spot (the owner's plan of 2026-10-23, change A) [draft names]. A part
//! whose tests run under pytest may have its planned test files written once lines 1–3 and 6–10
//! hold for it (`test_may_be_written`), before its place (line 4) is chosen, while no card of the
//! person's is open on it. Any other edit, and any commit, still waits for all eleven lines, and its
//! refusal names the next step: write the test, then run familiar-ai try. A part under any other
//! stack keeps today's order.

mod common;
mod traced;

use familiar_ai_rules::hooks::{decide, Event};
use familiar_ai_rules::{plan, terminal, trace, View};
use serde_json::json;
use traced::*;

#[test]
fn lines_one_to_three_and_six_to_ten_let_the_test_be_written_and_the_plan_is_not_yet_complete() {
    let log = planned();
    let v = View::new(&log);
    assert!(plan::test_may_be_written(&v, "c1"));
    assert!(!plan::planning_confirm_complete(&v), "all eleven lines still wait for the place");
    assert!(!plan::line_4(&v));
    assert!(trace::traced(&v, "c1"));
    // each of the lines the test needs, missing, keeps it from being written
    let no_test = {
        let mut l = log.clone();
        l.retain(|f| f.kind != "test_planned");
        l
    };
    assert!(!plan::test_may_be_written(&View::new(&no_test), "c1"), "line 6");
    let unassigned = {
        let mut l = log.clone();
        l.retain(|f| f.kind != "criterion_assigned");
        l
    };
    assert!(!plan::test_may_be_written(&View::new(&unassigned), "c1"), "line 10");
    let no_stack = {
        let mut l = log.clone();
        l.retain(|f| f.kind != "test_stack_detected");
        l
    };
    assert!(!plan::test_may_be_written(&View::new(&no_stack), "c1"), "line 7: a command to run the test");
    assert!(!plan::test_may_be_written(&View::new(&log), "c2"), "no such part");
}

#[test]
fn the_planned_test_file_may_be_edited_and_the_code_waits_naming_the_try() {
    let log = planned();
    assert!(edit(&log, TEST).allowed(), "{:?}", edit(&log, TEST));
    // an edit of the test whatever part the conversation works on, or none
    let v = View::new(&log);
    assert!(decide(&v, &Event::FileEdit { criterion_id: None, file_path: TEST.into() }).allowed());
    let code = edit(&log, CODE);
    assert_eq!(hold_of(&code).as_deref(), Some(trace::TRY_OWED), "{code:?}");
    let commit = decide(&v, &Event::FirstEditOrCommit);
    assert_eq!(hold_of(&commit).as_deref(), Some(trace::TRY_OWED));
    let words = terminal::refusal(&v, &Event::FileEdit { criterion_id: Some("c1".into()), file_path: CODE.into() }).unwrap_or_default();
    assert!(words.contains("held: try_owed"), "{words}");
    assert!(words.contains("write the test, then run familiar-ai try"), "{words}");
    assert!(whys(&log).contains(&trace::TRY_OWED.to_string()), "{:?}", whys(&log));
}

#[test]
fn before_lines_six_to_ten_hold_the_test_waits_as_today() {
    let mut log = planned();
    log.retain(|f| f.kind != "criterion_assigned");
    let d = edit(&log, TEST);
    assert_eq!(hold_of(&d).as_deref(), Some("planning_incomplete"), "{d:?}");
    assert!(!whys(&log).contains(&trace::TRY_OWED.to_string()), "no try is owed before the test may be written");
}

#[test]
fn a_part_under_another_stack_keeps_todays_order() {
    for stack in ["cargo", "python_pytest", "sh"] {
        let log = planned_with(Some(SHOWN), stack);
        let v = View::new(&log);
        assert!(!trace::traced(&v, "c1"), "{stack}");
        assert_eq!(hold_of(&edit(&log, TEST)).as_deref(), Some("planning_incomplete"), "{stack}: the test waits for the place");
        assert_eq!(hold_of(&edit(&log, CODE)).as_deref(), Some("planning_incomplete"), "{stack}");
        assert!(!whys(&log).contains(&trace::TRY_OWED.to_string()), "{stack}");
        // its place is picked with no try, as today
        let log = placed(&log, CODE, "write_rows", (10, 30));
        assert!(plan::planning_confirm_complete(&View::new(&log)), "{stack}");
    }
}

#[test]
fn a_file_another_part_plans_is_not_this_parts_to_write() {
    let log = planned();
    // c2's planned test, with c2 not planned up to its test: refused as today
    let log = raw(&log, "criterion", json!({ "criterion_id": "c2", "text": "the export names its columns", "path": "full", "behavior_id": "b1", "promise_id": "p1" }));
    let log = raw(&log, "test_planned", json!({ "criterion_id": "c2", "file_path": "tests/test_columns.py", "will": "create" }));
    assert_eq!(hold_of(&edit(&log, "tests/test_columns.py")).as_deref(), Some("planning_incomplete"));
    // c1's own test is still c1's to write
    assert!(edit(&log, TEST).allowed());
}

#[test]
fn another_parts_sealed_test_is_not_written_before_its_announcement() {
    // c1 sealed (the sample's, under python_pytest); then c2, under pytest, plans to modify c1's test
    let log = common::sealed();
    assert!(familiar_ai_rules::criterion::sealed_at(&View::new(&log), "c1", familiar_ai_rules::criterion::Place::Laptop).is_some());
    let log = raw(&log, "criterion", json!({ "criterion_id": "c2", "text": "the export writes each row once", "path": "full", "behavior_id": "b1", "promise_id": "p1" }));
    let log = raw(&log, "test_planned", json!({ "criterion_id": "c2", "file_path": common::TEST, "will": "modify" }));
    let log = raw(&log, "test_stack_detected", json!({ "codebase_id": "cb1", "project_root": "tests/", "test_stack": "pytest", "test_command": "python -m pytest" }));
    let log = raw(&log, "criterion_strung_on_head", json!({ "criterion_id": "c2", "head": "c1" }));
    let log = raw(&log, "criterion_assigned", json!({ "criterion_id": "c2", "agent_conversation_id": "a1" }));
    let v = View::new(&log);
    assert!(trace::needs_try(&v, "c2") && plan::test_may_be_written(&v, "c2"), "c2 may write its tests");
    assert!(!plan::planning_confirm_complete(&v));
    let d = decide(&v, &Event::FileEdit { criterion_id: Some("c2".into()), file_path: common::TEST.into() });
    assert_eq!(hold_of(&d).as_deref(), Some("proof_tampered"), "{d:?}");
}
