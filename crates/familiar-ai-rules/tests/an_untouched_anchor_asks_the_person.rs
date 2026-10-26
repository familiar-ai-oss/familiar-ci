//! A part whose tests pass while no commit changed the code at its anchor is
//! held on `anchor_untouched`, and that hold is a card the person answers:
//! `re_plan` chooses the place again (the proof starts over there),
//! `keep_going` makes the change where it landed the part's own (the binary
//! appends that place to the plan, the proof kept, and it goes green),
//! `release` lets it go. See docs/decisions.md#holds-are-the-persons

mod common;
use common::*;
use familiar_ai_facts::Log;
use familiar_ai_rules::sample::{apply_move, Move};
use familiar_ai_rules::{lifecycle, moves, plan, terminal, View};
use serde_json::Value;

/// c1 sealed red; the fix lands outside its anchor and the tests pass.
fn untouched() -> Log {
    let log = step(&sealed(), "code_outside_anchor", None);
    let log = run(&log, "passed");
    assert!(whys(&log).contains(&"anchor_untouched".to_string()), "{:?}", whys(&log));
    log
}

fn answers(log: &Log) -> Vec<Move> {
    moves::criterion_admissible(&View::new(log))
        .into_iter()
        .filter(|m| m.kind == "hold_answered" && m.fields.get("criterion_id").and_then(Value::as_str) == Some("c1"))
        .collect()
}

fn answered(choice: &str) -> Log {
    let log = untouched();
    let m = answers(&log)
        .into_iter()
        .find(|m| m.fields.get("hold").and_then(Value::as_str) == Some("anchor_untouched") && m.fields.get("choice").and_then(Value::as_str) == Some(choice));
    match m {
        Some(m) => apply_move(&log, &m, AT, TENANT).unwrap_or_default(),
        None => panic!("no {choice} answer at:\n{}", terminal::status(&View::new(&log))),
    }
}

#[test]
fn held_on_anchor_untouched_the_person_has_one_card_with_its_three_answers() {
    let log = untouched();
    let v = View::new(&log);
    assert_eq!(state(&log), "held");
    let at = plan::anchor_of(&v, "c1")
        .and_then(|a| plan::places_of(a).first().map(|p| format!("{} {}", p.file_path, p.symbol)))
        .expect("an anchor");
    let cards: Vec<_> = terminal::cards(&v).into_iter().filter(|c| c.criterion_id == "c1").collect();
    assert_eq!(cards.len(), 1, "{cards:?}");
    assert_eq!(cards[0].hold, "anchor_untouched");
    assert_eq!(cards[0].question, format!("criterion c1: its tests pass, but no commit changed the code at {at}. What now?"));
    let offered: Vec<&str> = cards[0].choices.iter().map(|(c, _)| c.as_str()).collect();
    assert_eq!(offered, ["re_plan", "keep_going", "release"]);
    let mut admissible: Vec<String> = answers(&log)
        .iter()
        .map(|m| format!("{}:{}", m.fields.get("hold").and_then(Value::as_str).unwrap_or(""), m.fields.get("choice").and_then(Value::as_str).unwrap_or("")))
        .collect();
    admissible.sort();
    assert_eq!(admissible, ["anchor_untouched:keep_going", "anchor_untouched:re_plan", "anchor_untouched:release"]);
}

#[test]
fn keep_going_makes_the_change_where_it_landed_the_parts_own_and_it_goes_green() {
    // Definition change (F12, the owner 2026-10-13): the change where it landed is the part's
    // through the place the binary appends right after the answer, not through any code.
    let log = answered("keep_going");
    let kept = moves::binary_moves(&View::new(&log)).into_iter().find(|m| m.kind == "place_chosen").expect("the binary appends the place where the change landed");
    let log = apply_move(&log, &kept, AT, TENANT).unwrap_or_default();
    assert!(whys(&log).is_empty(), "{:?}", whys(&log));
    assert_eq!(state(&log), "green");
    assert!(terminal::cards(&View::new(&log)).iter().all(|c| c.criterion_id != "c1"));
}

#[test]
fn re_plan_chooses_the_place_again_and_the_proof_starts_over_there() {
    let log = answered("re_plan");
    let v = View::new(&log);
    assert!(plan::anchor_of(&v, "c1").is_none(), "the place chosen before the answer no longer counts");
    assert_eq!(state(&log), "unplanned");
    let log = step(&log, "re_anchor", None);
    let v = View::new(&log);
    assert!(plan::anchor_of(&v, "c1").is_some());
    assert!(whys(&log).is_empty(), "{:?}", whys(&log));
    assert_eq!(state(&log), "testing");
}

#[test]
fn release_lets_the_part_go() {
    let log = answered("release");
    assert!(lifecycle::released(&View::new(&log), "c1"));
    assert_eq!(whys(&log), ["released"]);
}
