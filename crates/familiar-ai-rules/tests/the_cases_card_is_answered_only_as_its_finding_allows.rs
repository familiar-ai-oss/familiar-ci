//! The cases card is answered only as its finding allows (the owner's plan of 2026-10-25, the
//! cases card) [draft names]: its choices are `cases::offered`'s, add_all and keep_planned on
//! found cases, keep_planned and look_again on none (look_again while fewer than two finders
//! ran, then keep_planned or the person's own case), each offered once. The person's answers on the other cards, offered before the plan is
//! confirmed while a part's kept place stands, leave the cases card to its own moves. The world:
//! the light part c2 answered keep_going on anchor_untouched (its kept place), then the person
//! asks for more and the full-path part c1 is planned to a whole contract.

use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::sample::{apply_move, criterion_fixture, Move, BEHAVIOR_ID, BEHAVIOR_TEXT, INTENT_SHA, PROMISE_ACCEPTS, PROMISE_GIVES_BACK, PROMISE_ID, PROMISE_REFUSES};
use familiar_ai_rules::{cases, criterion, moves, plan, terminal, View};
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";

fn step(log: &Log, label: &str, outcome: Option<&str>, choice: Option<&str>) -> Log {
    let v = View::new(log);
    let m = moves::criterion_admissible(&v).into_iter().find(|m| {
        m.label.as_deref() == Some(label)
            && outcome.is_none_or(|o| m.fields.get("outcome").and_then(Value::as_str) == Some(o))
            && choice.is_none_or(|c| m.fields.get("choice").and_then(Value::as_str) == Some(c))
    });
    match m {
        Some(m) => apply_move(log, &m, AT, TENANT).expect("lands"),
        None => panic!("no {label} move among {:?} at:\n{}", moves::criterion_admissible(&v).iter().map(|m| (m.label.clone(), m.kind.clone())).collect::<Vec<_>>(), terminal::status(&v)),
    }
}

fn raw(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { unreachable!() };
    append_raw(log, kind, m, AT, TENANT).unwrap_or_else(|e| panic!("{kind}: {e}"))
}

/// c2's kept place, as a_light_parts_kept_place_carries_its_documentation_check.rs walks to it,
/// then a second ask and c1 on the full path, its promise with a normal and an opposite case and
/// the contract's version: c1's case-finder is owed.
fn owed() -> Log {
    let mut log = criterion_fixture(&["c2"], true, false, AT, TENANT).expect("fixture");
    for (label, outcome, choice) in [
        ("start_check", None, None),
        ("tests_only", None, None),
        ("test_written", None, None),
        ("pass_please_pass", Some("passed"), None),
        ("fingerprint_stamped", None, None),
        ("code", None, None),
        ("hold_answered", None, Some("re_plan")),
        ("re_anchor", None, None),
        ("start_check", None, None),
        ("changed_test", None, None),
        ("pass_please_pass", Some("failed"), None),
        ("fingerprint_stamped", None, None),
        ("code_outside_anchor", None, None),
        ("pass_please_pass", Some("passed"), None),
        ("fingerprint_stamped", None, None),
        ("hold_answered", None, Some("keep_going")),
        ("landed_place", None, None),
    ] {
        log = step(&log, label, outcome, choice);
    }
    let log = raw(&log, "intent_submitted", json!({ "text": "and let a right password open a session", "sha": INTENT_SHA }));
    let log = raw(&log, "behavior", json!({ "behavior_id": BEHAVIOR_ID, "text": BEHAVIOR_TEXT }));
    let log = raw(
        &log,
        "promise",
        json!({ "promise_id": PROMISE_ID, "behavior_id": BEHAVIOR_ID, "accepts": PROMISE_ACCEPTS, "gives_back": PROMISE_GIVES_BACK, "refuses": PROMISE_REFUSES, "contract_version": 1, "cases": [
            { "case_id": "n1", "kind": "normal", "given": "the right password", "expect": "a session" },
            { "case_id": "o1", "kind": "opposite", "given": "a wrong password", "expect": "no session" }
        ] }),
    );
    raw(&log, "criterion", json!({ "criterion_id": "c1", "text": "a right password opens a session", "path": "full", "behavior_id": BEHAVIOR_ID, "promise_id": PROMISE_ID }))
}

// Definition change (the owner's plan of 2026-10-26, the lens panel, approved): the synthesis's
// finding is admissible only after the five lenses answered; `lensed` takes the lens moves first.
fn lensed(log: &Log) -> Log {
    let mut log = log.clone();
    while let Some(m) = moves::criterion_admissible(&View::new(&log)).into_iter().find(|m| m.kind == "lens_found" && m.fields.get("criterion_id").and_then(Value::as_str) == Some("c1")) {
        log = apply_move(&log, &m, AT, TENANT).unwrap_or_else(|e| panic!("{}: {e}", m.kind));
    }
    log
}

fn admissible(log: &Log) -> Vec<Move> {
    moves::criterion_admissible(&View::new(log))
}

fn s<'a>(m: &'a Move, k: &str) -> &'a str {
    m.fields.get(k).and_then(Value::as_str).unwrap_or("")
}

/// The binary's finding on c1 with `outcome` and `n` cases, as admissible here.
fn finding(log: &Log, outcome: &str, n: usize) -> Move {
    admissible(log)
        .into_iter()
        .find(|m| m.kind == cases::FOUND && s(m, "criterion_id") == "c1" && s(m, "outcome") == outcome && m.fields.get("cases").and_then(Value::as_array).map_or(0, Vec::len) == n)
        .unwrap_or_else(|| panic!("no {outcome} finding with {n} cases for c1"))
}

fn take(log: &Log, m: &Move) -> Log {
    apply_move(log, m, AT, TENANT).unwrap_or_else(|e| panic!("{}: {e}", m.kind))
}

/// The choices admissible on c1's cases card, words typed aside, in order.
fn choices(log: &Log) -> Vec<String> {
    admissible(log)
        .iter()
        .filter(|m| m.kind == "hold_answered" && s(m, "criterion_id") == "c1" && s(m, "hold") == cases::CARD && s(m, "choice") != cases::TYPED)
        .map(|m| s(m, "choice").to_string())
        .collect()
}

/// The card is open on c1 before the plan is confirmed, beside c2's kept place: the world where
/// the person's other answers are offered too.
fn open_beside_a_kept_place(log: &Log) -> View<'_> {
    let v = View::new(log);
    assert!(cases::card_open(&v, "c1"), "{}", terminal::status(&v));
    assert!(!criterion::confirmed(&v), "{}", terminal::status(&v));
    let kept = plan::anchor_of(&v, "c2").is_some_and(|a| a.kind == "place_chosen" && a.fields.get("hold").and_then(Value::as_str) == Some("anchor_untouched"));
    assert!(kept, "c2's anchor is the place kept on keep_going: {}", terminal::status(&v));
    v
}

#[test]
fn on_found_cases_the_card_offers_add_all_and_keep_planned_once_each_and_no_look_again() {
    let log = lensed(&owed());
    let log = take(&log, &finding(&log, "found", 2));
    let v = open_beside_a_kept_place(&log);
    assert_eq!(cases::offered(&v, "c1"), [cases::ADD_ALL, cases::KEEP]);
    assert_eq!(choices(&log), [cases::ADD_ALL, cases::KEEP]);
}

#[test]
fn on_none_found_the_card_offers_keep_planned_and_look_again_and_no_add_all() {
    let log = lensed(&owed());
    let log = take(&log, &finding(&log, "none", 0));
    let v = open_beside_a_kept_place(&log);
    assert_eq!(cases::offered(&v, "c1"), [cases::KEEP, cases::AGAIN]);
    assert_eq!(choices(&log), [cases::KEEP, cases::AGAIN]);
}

/// The owner's ruling (2026-10-25, "Keep, or add my own"): after the last look the card offers
/// keep_planned or the person's own case, words typed (`cases::TYPED`), whose moves are the words.
#[test]
fn after_two_finders_the_card_offers_keep_planned_or_the_persons_own_case() {
    let log = lensed(&owed());
    let log = take(&log, &finding(&log, "none", 0));
    let again = admissible(&log)
        .into_iter()
        .find(|m| m.kind == "hold_answered" && s(m, "hold") == cases::CARD && s(m, "choice") == cases::AGAIN)
        .expect("look_again on the first none");
    let log = lensed(&take(&log, &again));
    let log = take(&log, &finding(&log, "failed", 0));
    let v = open_beside_a_kept_place(&log);
    assert_eq!(cases::offered(&v, "c1"), [cases::KEEP, cases::TYPED]);
    assert_eq!(choices(&log), [cases::KEEP]);
}
