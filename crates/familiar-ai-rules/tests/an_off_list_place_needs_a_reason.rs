//! A place off the try's list (the owner's plan of 2026-10-23, step 5) [draft names]: about one fix
//! in seven lies off the path the failing test ran, so a place not on the list stays possible with
//! the agent's one-line `off_list_reason`, made after the place and read with it (the seal's judge
//! reads it). Until then line 4 waits, and the refusal names the reason or another place.

mod traced;

use familiar_ai_rules::hooks::Event;
use familiar_ai_rules::{plan, terminal, trace, View};
use serde_json::json;
use traced::*;

fn off() -> familiar_ai_facts::Log {
    placed(&tried(&planned(), true, top()), "src/template.py", "render_header", (3, 9))
}

fn reason(log: &familiar_ai_facts::Log, r: &str) -> familiar_ai_facts::Log {
    raw(log, trace::OFF_LIST, json!({ "criterion_id": "c1", "reason": r }))
}

#[test]
fn a_place_off_the_list_waits_for_its_reason() {
    let log = off();
    let v = View::new(&log);
    assert!(!plan::line_4(&v));
    assert_eq!(trace::off_list(&v, "c1").len(), 1);
    let words = terminal::refusal(&v, &Event::FileEdit { criterion_id: Some("c1".into()), file_path: "src/template.py".into() }).unwrap_or_default();
    assert!(words.contains("held: planning_incomplete"), "{words}");
    assert!(words.contains("not on the try's list"), "{words}");
    let log = reason(&log, "the header comes from a template the test renders through a cache it never misses");
    let v = View::new(&log);
    assert!(plan::line_4(&v));
    assert!(plan::planning_confirm_complete(&v));
    assert_eq!(trace::off_list_reason(&v, "c1").and_then(|f| f.fields.get("reason").cloned()), Some(json!("the header comes from a template the test renders through a cache it never misses")));
}

#[test]
fn a_blank_reason_says_nothing_and_the_door_refuses_it() {
    assert!(!plan::line_4(&View::new(&reason(&off(), "   "))));
    assert!(trace::refused_reason("  ").is_some());
    assert_eq!(trace::refused_reason("the header is rendered by a template"), None);
}

#[test]
fn a_reason_given_before_the_place_is_not_this_places() {
    let log = reason(&tried(&planned(), true, top()), "the header is rendered by a template");
    let log = placed(&log, "src/template.py", "render_header", (3, 9));
    assert!(!plan::line_4(&View::new(&log)));
}

#[test]
fn a_place_on_the_list_needs_no_reason() {
    let log = placed(&tried(&planned(), true, top()), CODE, "header", (32, 40));
    assert!(plan::line_4(&View::new(&log)));
    assert!(trace::off_list(&View::new(&log), "c1").is_empty());
}
