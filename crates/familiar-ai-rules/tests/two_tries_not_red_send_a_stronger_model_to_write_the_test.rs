//! Two tests-first runs not red give `two_tries_not_red`, which pauses like
//! `two_failures`. D5 (the owner's announced definition change, 2026-10-16): the
//! investigator ladder is for a valid red only, so the way out is the person's
//! card alone: try_again, re_plan or release. re_plan unchooses the place.

use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::sample::{apply_move, criterion_fixture, Move};
use familiar_ai_rules::{lifecycle, moves, plan, terminal, View};
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";

fn fact(log: &Log, kind: &str, fields: Value) -> Log {
    append_raw(log, kind, fields.as_object().cloned().unwrap_or_default(), AT, TENANT).unwrap_or_default()
}

fn find(log: &Log, label: &str, outcome: Option<&str>) -> Option<Move> {
    let v = View::new(log);
    moves::criterion_admissible(&v).into_iter().find(|m| {
        m.label.as_deref() == Some(label) && outcome.is_none_or(|o| m.fields.get("outcome").and_then(Value::as_str) == Some(o))
    })
}

fn step(log: &Log, label: &str, outcome: Option<&str>) -> Log {
    match find(log, label, outcome) {
        Some(m) => apply_move(log, &m, AT, TENANT).unwrap_or_default(),
        None => panic!("no {label} move at:\n{}", terminal::status(&View::new(log))),
    }
}

/// One tests commit run through by the binary: its test with `outcome`, the
/// touching test passed, the stamp.
fn tests_commit(log: &Log, label: &str, outcome: &str) -> Log {
    let mut log = step(log, label, None);
    if label == "tests_only" {
        log = step(&log, "test_written", None);
    }
    log = step(&log, "pass_please_pass", Some(outcome));
    log = step(&log, "pass_please_pass", Some("passed"));
    step(&log, "fingerprint_stamped", None)
}

fn whys(log: &Log) -> Vec<String> {
    lifecycle::whys(&View::new(log), "c1")
}

/// c1 held on two_tries_not_red: its holder `a1` on haiku, two tests-first runs that passed.
fn held() -> Log {
    let log = criterion_fixture(&["c1", "c2"], true, false, AT, TENANT).unwrap_or_default();
    let log = fact(&log, "model_seen", json!({ "agent_conversation_id": "a1", "model": "claude-haiku-4-5-20251001" }));
    let log = step(&log, "start_check", None);
    let log = tests_commit(&log, "tests_only", "passed");
    tests_commit(&log, "changed_test", "passed")
}

#[test]
fn two_tries_not_red_pause_the_holder_and_offer_only_the_persons_card() {
    let log = held();
    let v = View::new(&log);
    assert_eq!(lifecycle::tries_not_red(&v, "c1"), 2);
    assert_eq!(whys(&log), ["tests_not_red", "two_tries_not_red"]);
    assert!(lifecycle::paused(&v, "c1"), "the default model waits");
    assert!(find(&log, "changed_test", None).is_none(), "no commit while paused");
    let hs = lifecycle::holds(&v, "c1");
    let h = hs.iter().find(|h| h.why == "two_tries_not_red").expect("held");
    let clears: Vec<Value> = h.clears.iter().cloned().map(Value::Object).collect();
    let want: Vec<Value> = ["try_again", "re_plan", "release"]
        .iter()
        .map(|ch| json!({ "kind": "hold_answered", "hold": "two_tries_not_red", "choice": ch }))
        .collect();
    assert_eq!(clears, want, "no investigator: the person's card only");
    assert!(find(&log, "investigator", None).is_none(), "no investigator move");
    let card = terminal::cards(&v).into_iter().find(|k| k.hold == "two_tries_not_red").expect("a card");
    let choices: Vec<&str> = card.choices.iter().map(|(c, _)| c.as_str()).collect();
    assert_eq!(choices, ["try_again", "re_plan", "release"]);
    assert!(card.question.contains("two tries to go red failed"), "{}", card.question);
}

#[test]
fn try_again_gives_two_more_tries() {
    let log = fact(&held(), "hold_answered", json!({ "criterion_id": "c1", "hold": "two_tries_not_red", "choice": "try_again", "typed": "" }));
    let v = View::new(&log);
    assert!(!whys(&log).iter().any(|w| w == "two_tries_not_red"), "{:?}", whys(&log));
    assert!(!lifecycle::paused(&v, "c1"));
    let log = tests_commit(&log, "changed_test", "passed");
    assert!(!whys(&log).iter().any(|w| w == "two_tries_not_red"), "one more try: {:?}", whys(&log));
    let log = tests_commit(&log, "changed_test", "passed");
    assert!(whys(&log).iter().any(|w| w == "two_tries_not_red"), "the second: {:?}", whys(&log));
}

#[test]
fn re_plan_unchooses_the_place() {
    let log = fact(&held(), "hold_answered", json!({ "criterion_id": "c1", "hold": "two_tries_not_red", "choice": "re_plan", "typed": "" }));
    let v = View::new(&log);
    assert!(plan::unchosen(&v, "c1").is_some(), "the place is chosen again");
    assert!(!whys(&log).iter().any(|w| w == "two_tries_not_red"));
}
