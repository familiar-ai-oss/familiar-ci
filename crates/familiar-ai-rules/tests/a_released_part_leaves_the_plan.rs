//! A released part leaves the plan (the owner, 2026-10-28): the plan's lines 2–11 read the parts
//! not released, so a part the person released, on a card or with `familiar-ai clear`, never keeps
//! the plan from being confirmed (the Duet session: four released parts, one anchored in a
//! git-ignored file, held its new parts' plan for good). With every part released no part is
//! left in it: line 2 asks for a part claimed, ever, and lines 3–11 hold with no part to hold them,
//! as a plan released whole after its confirmation always did.

mod traced;

use familiar_ai_facts::Log;
use familiar_ai_rules::{plan, sample, View};
use serde_json::json;
use traced::*;

fn confirmed(log: &Log) -> bool {
    plan::plan_state(&View::new(log)) == "confirmed"
}

fn released(log: &Log, c: &str) -> Log {
    raw(log, "hold_answered", json!({ "criterion_id": c, "hold": "planning_incomplete", "choice": "release", "typed": "", "from": "clear" }))
}

#[test]
fn a_released_part_no_longer_holds_the_plan() {
    let log = sample::criterion_fixture(&["c1"], true, false, AT, TENANT).expect("fixture");
    assert!(confirmed(&log), "c1 planned whole");
    let log = raw(&log, "criterion", json!({ "criterion_id": "c9", "text": "a part never planned past its claim", "path": "full" }));
    assert!(!confirmed(&log), "c9's lines do not hold");
    let log = released(&log, "c9");
    assert!(confirmed(&log), "c9 released leaves the plan");
    assert_eq!(plan::planned(&View::new(&log)), ["c1"]);
}

#[test]
fn with_every_part_released_no_part_is_left_and_the_plan_stays_as_it_was() {
    let log = sample::criterion_fixture(&["c1"], true, false, AT, TENANT).expect("fixture");
    let log = released(&log, "c1");
    assert!(plan::planned(&View::new(&log)).is_empty());
    assert!(confirmed(&log), "a plan released whole stays confirmed");
    assert!(!plan::line_2(&View::new(&Vec::new())), "line 2 still asks for a part claimed");
}

#[test]
fn a_released_part_owes_no_try() {
    // c1 planned up to its test, its red try owed
    let log = planned();
    assert!(familiar_ai_rules::trace::needs_try(&View::new(&log), "c1"));
    let log = released(&log, "c1");
    assert!(!familiar_ai_rules::trace::needs_try(&View::new(&log), "c1"), "released, it owes no try");
    assert_eq!(familiar_ai_rules::trace::try_hold(&View::new(&log), "c1"), None);
}
