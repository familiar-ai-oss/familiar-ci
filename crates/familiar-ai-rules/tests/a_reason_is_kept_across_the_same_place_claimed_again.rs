//! A refusal's way out never undoes a line that already holds (ports 2026-10-24, piece 2) [draft
//! names]: the agent's `off_list_reason`, made after its place, stays that place's reason when the
//! agent claims the same place again, identical, as the bench's agents did by sending the reason
//! and the place in one command (bench-c2v at 1df36806: click-3299 and pytest-14931, their places
//! and reasons as recorded). A reason given before a different place is not that place's, and the
//! order of `an_off_list_place_needs_a_reason.rs` stands: a reason before the place was first
//! chosen is not its reason. The person's re_plan between the two claims ends the run: the same place
//! claimed after it owes a reason of its own (the owner, 2026-10-25).

mod traced;

use familiar_ai_facts::Log;
use familiar_ai_rules::{plan, trace, View};
use serde_json::{json, Value};
use traced::*;

/// click-3299's reason, as recorded.
const CLICK_REASON: &str = "get_help_extra builds the help default text and holds the == comparisons; the failing test reaches it only as a late help-render step";
/// pytest-14931's reason, as recorded.
const PYTEST_REASON: &str = "the failing test ran check_ispytest in deprecated.py, which warns with the shared PRIVATE instance; it is not in the top of the list";

/// The first three of click-3299's red try's listing, as recorded.
fn click_listing() -> Value {
    json!([
        { "rank": 1, "file_path": "src/click/parser.py", "symbol": "__init__", "line_start": 237, "line_end": 259 },
        { "rank": 2, "file_path": "src/click/core.py", "symbol": "get_default", "line_start": 2258, "line_end": 2292 },
        { "rank": 3, "file_path": "src/click/core.py", "symbol": "main", "line_start": 1338, "line_end": 1449 }
    ])
}

/// The first three of pytest-14931's red try's listing, as recorded.
fn pytest_listing() -> Value {
    json!([
        { "rank": 1, "file_path": "src/_pytest/runner.py", "symbol": "pytest_runtest_call", "line_start": 173, "line_end": 194 },
        { "rank": 2, "file_path": "src/_pytest/python.py", "symbol": "pytest_pyfunc_call", "line_start": 180, "line_end": 198 },
        { "rank": 3, "file_path": "src/_pytest/runner.py", "symbol": "_update_current_test_var", "line_start": 203, "line_end": 217 }
    ])
}

/// click-3299's place, off its try's list.
fn get_help_extra(log: &Log) -> Log {
    placed(log, "src/click/core.py", "get_help_extra", (3052, 3134))
}

/// pytest-14931's place, off its try's list.
fn check_ispytest(log: &Log) -> Log {
    placed(log, "src/_pytest/deprecated.py", "check_ispytest", (153, 155))
}

fn reason(log: &Log, r: &str) -> Log {
    raw(log, trace::OFF_LIST, json!({ "criterion_id": "c1", "reason": r }))
}

fn reason_of(v: &View) -> Option<Value> {
    trace::off_list_reason(v, "c1").and_then(|f| f.fields.get("reason").cloned())
}

#[test]
fn a_reason_between_two_identical_place_claims_counts() {
    // click-3299: the place, its reason, then the same place again in the same command
    let log = get_help_extra(&tried(&planned(), true, click_listing()));
    assert!(!plan::line_4(&View::new(&log)), "off the list, the place waits for its reason");
    let log = reason(&log, CLICK_REASON);
    assert!(plan::line_4(&View::new(&log)), "the reason holds line 4");
    let log = get_help_extra(&log);
    let v = View::new(&log);
    assert!(plan::line_4(&v), "the same place claimed again keeps its reason");
    assert!(plan::planning_confirm_complete(&v));
    assert_eq!(reason_of(&v), Some(json!(CLICK_REASON)));
    // pytest-14931: the reason sent before the place in one command, then both again
    let log = check_ispytest(&reason(&tried(&planned(), true, pytest_listing()), PYTEST_REASON));
    assert!(!plan::line_4(&View::new(&log)), "a reason before the place is not this place's");
    let log = check_ispytest(&reason(&log, PYTEST_REASON));
    let v = View::new(&log);
    assert!(plan::line_4(&v), "a reason after the place, then the same place again, counts");
    assert!(plan::planning_confirm_complete(&v));
    assert_eq!(reason_of(&v), Some(json!(PYTEST_REASON)));
}

#[test]
fn a_reason_before_a_different_place_does_not_count() {
    let log = reason(&get_help_extra(&tried(&planned(), true, click_listing())), CLICK_REASON);
    assert!(plan::line_4(&View::new(&log)));
    // another function of the same file, off the list too: the reason was given for the place before it
    let log = placed(&log, "src/click/core.py", "Option.prompt_for_value", (2900, 2950));
    let v = View::new(&log);
    assert!(!plan::line_4(&v), "a reason before a different place is not its reason");
    assert_eq!(reason_of(&v), None);
    assert_eq!(trace::off_list(&v, "c1").len(), 1);
    // its own reason, after it, counts
    let log = reason(&log, "the fix lies in prompt_for_value, which the failing test never ran");
    assert!(plan::line_4(&View::new(&log)));
}

#[test]
fn a_re_plan_between_two_identical_place_claims_ends_the_reason() {
    // the place, its reason, the person's re_plan, then the same place again
    let log = reason(&get_help_extra(&tried(&planned(), true, click_listing())), CLICK_REASON);
    assert!(plan::line_4(&View::new(&log)));
    let log = raw(&log, "hold_answered", json!({ "criterion_id": "c1", "hold": "two_tries_not_red", "choice": "re_plan", "typed": "" }));
    let v = View::new(&log);
    assert!(plan::anchor_of(&v, "c1").is_none() && !plan::line_4(&v), "the re_plan unchooses the place");
    let log = get_help_extra(&log);
    let v = View::new(&log);
    assert!(!plan::line_4(&v), "the same place claimed again after the re_plan owes a reason of its own");
    assert_eq!(reason_of(&v), None);
    assert!(trace::reason_owed(&v, "c1"));
    let log = reason(&log, CLICK_REASON);
    assert!(plan::line_4(&View::new(&log)), "the reason given again holds line 4");
}
