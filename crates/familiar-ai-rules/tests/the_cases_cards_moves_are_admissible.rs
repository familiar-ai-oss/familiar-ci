//! The cases card's moves over the sample world (the owner's plan of 2026-10-25, step 5) [draft
//! names]: while the case-finder is owed, the binary's `cases_found` (found with a case or two,
//! none, failed) is admissible; while the person's card is open, each choice it offers and words
//! typed (numbers that pick found cases, when it found some, and a case in the person's words);
//! and once the card is answered with cases asked for, the agent's promise claimed again holding
//! them. Nothing of it is offered on a log whose promise carries no contract.

use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::sample::{apply_move, criterion_fixture, Move};
use familiar_ai_rules::{cases, moves, plan, View};
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";

fn raw(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { unreachable!() };
    append_raw(log, kind, m, AT, TENANT).unwrap_or_else(|e| panic!("{kind}: {e}"))
}

/// The sample's c1 planned, its promise claimed again with a whole contract (a normal and an
/// opposite case, the version the claim door writes).
fn whole() -> Log {
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).expect("the fixture");
    let p = View::new(&log).kind("promise").last().expect("the sample's promise").fields.clone();
    let mut f = p.clone();
    f.insert("cases".into(), json!([
        { "case_id": "n1", "kind": "normal", "given": "the trigger present", "expect": "the new result" },
        { "case_id": "o1", "kind": "opposite", "given": "the trigger absent", "expect": "the result as before" }
    ]));
    f.insert("contract_version".into(), json!(1));
    raw(&log, "promise", Value::Object(f))
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

fn all(log: &Log) -> Vec<Move> {
    moves::criterion_admissible(&View::new(log))
}

fn of_kind(log: &Log, kind: &str) -> Vec<Move> {
    all(log).into_iter().filter(|m| m.kind == kind).collect()
}

fn s<'a>(m: &'a Move, k: &str) -> &'a str {
    m.fields.get(k).and_then(Value::as_str).unwrap_or("")
}

fn findings(log: &Log) -> Vec<Move> {
    of_kind(log, cases::FOUND)
}

fn card_answers(log: &Log) -> Vec<Move> {
    of_kind(log, "hold_answered").into_iter().filter(|m| s(m, "hold") == cases::CARD).collect()
}

fn take(log: &Log, m: &Move) -> Log {
    apply_move(log, m, AT, TENANT).unwrap_or_else(|e| panic!("{}: {e}", m.kind))
}

/// The finding the binary may record with `outcome` and `n` cases.
fn finding(log: &Log, outcome: &str, n: usize) -> Move {
    findings(log)
        .into_iter()
        .find(|m| s(m, "outcome") == outcome && m.fields.get("cases").and_then(Value::as_array).map_or(0, Vec::len) == n)
        .unwrap_or_else(|| panic!("no {outcome} finding with {n} cases among {:?}", findings(log).iter().map(|m| m.fields.clone()).collect::<Vec<_>>()))
}

fn answer(log: &Log, choice: &str) -> Move {
    card_answers(log).into_iter().find(|m| s(m, "choice") == choice).unwrap_or_else(|| panic!("no {choice}"))
}

/// The typed answers offered: (words, whether they are only numbers).
fn typed(log: &Log) -> Vec<(String, bool)> {
    card_answers(log)
        .iter()
        .filter(|m| s(m, "choice") == cases::TYPED)
        .map(|m| {
            let w = s(m, "typed").to_string();
            let nums = w.split(|c: char| !c.is_alphanumeric()).filter(|t| !t.is_empty()).all(|t| t.parse::<usize>().is_ok() || t.eq_ignore_ascii_case("and"));
            (w, nums)
        })
        .collect()
}

#[test]
fn while_the_finder_is_owed_the_binarys_finding_is_admissible_found_none_or_failed() {
    assert!(findings(&whole()).is_empty(), "no synthesis before the five lenses answered");
    let log = lensed(&whole());
    assert!(cases::owed(&View::new(&log), "c1"));
    let fs = findings(&log);
    let shape: Vec<(String, usize, usize)> = fs
        .iter()
        .map(|m| (s(m, "outcome").to_string(), m.fields.get("cases").and_then(Value::as_array).map_or(0, Vec::len), m.fields.get("questions").and_then(Value::as_array).map_or(0, Vec::len)))
        .collect();
    let f = |o: &str, n: usize, q: usize| (o.to_string(), n, q);
    assert_eq!(shape, [f("found", 1, 0), f("found", 2, 0), f("none", 0, 0), f("failed", 0, 0), f("found", 1, 2)], "{shape:?}");
    for m in &fs {
        assert_eq!(s(m, "criterion_id"), "c1");
        // each lands as the binary writes it, and clears the finder's hold
        let after = take(&log, m);
        assert!(!cases::owed(&View::new(&after), "c1"));
        assert!(cases::card_open(&View::new(&after), "c1"));
    }
    // no answer on the card before a finding
    assert!(card_answers(&log).is_empty());
}

#[test]
fn on_found_cases_the_card_offers_its_choices_numbers_and_words() {
    let log = take(&lensed(&whole()), &finding(&lensed(&whole()), "found", 2));
    assert!(findings(&log).is_empty(), "no finder while the card is open");
    let choices: Vec<String> = card_answers(&log).iter().filter(|m| s(m, "choice") != cases::TYPED).map(|m| s(m, "choice").to_string()).collect();
    assert_eq!(choices, [cases::ADD_ALL, cases::KEEP]);
    let t = typed(&log);
    assert!(t.iter().any(|(_, nums)| *nums), "numbers typed: {t:?}");
    assert!(t.iter().any(|(w, nums)| !nums && !w.trim().is_empty()), "a case typed: {t:?}");
    for m in card_answers(&log) {
        let after = take(&log, &m);
        assert!(!cases::card_open(&View::new(&after), "c1"), "{:?} closes the card", m.fields);
    }
}

#[test]
fn on_none_or_failed_the_card_offers_keep_look_again_and_words_but_no_numbers() {
    for outcome in ["none", "failed"] {
        let log = take(&lensed(&whole()), &finding(&lensed(&whole()), outcome, 0));
        let choices: Vec<String> = card_answers(&log).iter().filter(|m| s(m, "choice") != cases::TYPED).map(|m| s(m, "choice").to_string()).collect();
        assert_eq!(choices, [cases::KEEP, cases::AGAIN], "{outcome}");
        let t = typed(&log);
        assert!(!t.iter().any(|(_, nums)| *nums), "{outcome}: {t:?}");
        assert!(t.iter().any(|(w, nums)| !nums && !w.trim().is_empty()), "{outcome}: {t:?}");
        // look again owes one more finder, offered again; after the second, no look again
        let again = take(&log, &answer(&log, cases::AGAIN));
        assert!(cases::owed(&View::new(&again), "c1"));
        let again = lensed(&again);
        let second = take(&again, &finding(&again, "none", 0));
        let choices: Vec<String> = card_answers(&second).iter().filter(|m| s(m, "choice") != cases::TYPED).map(|m| s(m, "choice").to_string()).collect();
        assert_eq!(choices, [cases::KEEP], "{outcome}: two finders at most");
    }
}

#[test]
fn cases_asked_for_are_claimed_into_the_promise_and_line_3_then_holds() {
    let found = take(&lensed(&whole()), &finding(&lensed(&whole()), "found", 2));
    for m in card_answers(&found) {
        let log = take(&found, &m);
        let v = View::new(&log);
        let missing = cases::missing(&v, "c1");
        let promises = of_kind(&log, "promise");
        if missing.is_empty() {
            assert!(promises.is_empty(), "{:?}: nothing to claim", m.fields);
            assert!(plan::line_3_of(&v, "c1"), "{:?}", m.fields);
            continue;
        }
        assert!(!plan::line_3_of(&v, "c1"), "{:?}", m.fields);
        assert_eq!(promises.len(), 1, "{:?}", m.fields);
        let after = take(&log, &promises[0]);
        let v = View::new(&after);
        assert!(cases::missing(&v, "c1").is_empty(), "{:?}", m.fields);
        assert!(!cases::holds(&v, "c1"), "a promise claimed again owes no new finder");
        assert!(plan::line_3_of(&v, "c1"), "{:?}", m.fields);
        assert_eq!(plan::plan_state(&v), "confirmed");
    }
}

#[test]
fn nothing_of_the_card_on_a_promise_with_no_contract() {
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).expect("the fixture");
    assert!(findings(&log).is_empty());
    assert!(card_answers(&log).is_empty());
}
