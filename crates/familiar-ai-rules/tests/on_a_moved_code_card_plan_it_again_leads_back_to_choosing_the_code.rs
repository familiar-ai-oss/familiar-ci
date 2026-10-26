//! After `re_plan` on `anchor_moved`, the place chosen before the answer no
//! longer counts: the holder chooses a new place and the start check runs on
//! it. `keep_going` keeps the moved place and `release` ends the criterion.

use familiar_ai_facts::Log;
use familiar_ai_rules::sample::{apply_move, criterion_fixture, Move};
use familiar_ai_rules::{criterion, lifecycle, moves, plan, terminal, View};
use serde_json::Value;

const AT: f64 = 0.0;
const TENANT: &str = "_default_";

fn find(log: &Log, label: &str, choice: Option<&str>) -> Option<Move> {
    let v = View::new(log);
    moves::criterion_admissible(&v).into_iter().find(|m| {
        m.label.as_deref() == Some(label) && choice.is_none_or(|c| m.fields.get("choice").and_then(Value::as_str) == Some(c))
    })
}

fn step(log: &Log, label: &str, choice: Option<&str>) -> Log {
    match find(log, label, choice) {
        Some(m) => apply_move(log, &m, AT, TENANT).unwrap_or_default(),
        None => panic!("no {label} move at:\n{}", terminal::status(&View::new(log))),
    }
}

/// c1's start check saw its lines moved: the moved-code card is up.
fn moved() -> Log {
    let log = criterion_fixture(&["c1", "c2"], true, false, AT, TENANT).unwrap_or_default();
    let log = step(&log, "outside_edit_seen", None);
    assert!(lifecycle::whys(&View::new(&log), "c1").iter().any(|w| w == "anchor_moved"));
    log
}

fn answered(choice: &str) -> Log {
    step(&moved(), "hold_answered", Some(choice))
}

#[test]
fn after_plan_it_again_no_place_is_chosen_for_the_criterion() {
    let log = answered("re_plan");
    let v = View::new(&log);
    assert!(plan::anchor_of(&v, "c1").is_none(), "the place chosen before the answer no longer counts");
    assert!(criterion::place(&v, "c1").is_none());
    assert!(plan::anchor_of(&v, "c2").is_some(), "the other criterion keeps its place");
}

#[test]
fn after_plan_it_again_the_criterion_is_in_planning_and_a_new_place_is_admissible() {
    let log = answered("re_plan");
    let v = View::new(&log);
    assert_eq!(plan::plan_state(&v), "planning");
    assert_eq!(lifecycle::state(&v, "c1"), "unplanned");
    assert!(find(&log, "re_anchor", None).is_some_and(|m| m.kind == "place_chosen"), "a new place is the holder's next move");
    let log = step(&log, "re_anchor", None);
    let v = View::new(&log);
    assert_eq!(plan::plan_state(&v), "confirmed");
    assert!(plan::anchor_of(&v, "c1").is_some());
    assert_eq!(lifecycle::state(&v, "c1"), "testing");
    assert!(lifecycle::start_check(&v, "c1").is_none(), "the start check runs again on the new place");
    assert!(find(&log, "start_check", None).is_some());
    assert!(v.of("test_planned", "c1").next().is_some(), "its planned tests stand");
}

#[test]
fn after_keep_going_the_moved_place_stands_and_the_criterion_goes_on() {
    let before = moved();
    let log = step(&before, "hold_answered", Some("keep_going"));
    let v = View::new(&log);
    assert_eq!(plan::anchor_of(&v, "c1").map(|a| a.t), plan::anchor_of(&View::new(&before), "c1").map(|a| a.t));
    assert!(lifecycle::start_check(&v, "c1").is_some(), "the start check at the moved lines stands");
    assert_eq!(plan::plan_state(&v), "confirmed");
    assert_eq!(lifecycle::state(&v, "c1"), "testing");
    assert!(lifecycle::whys(&v, "c1").is_empty());
}

#[test]
fn the_three_answers_derive_three_different_states() {
    let states: Vec<&str> = ["re_plan", "keep_going", "release"]
        .iter()
        .map(|c| lifecycle::state(&View::new(&answered(c)), "c1"))
        .collect();
    assert_eq!(states, ["unplanned", "testing", "held"]);
}
