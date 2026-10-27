//! The cases card's words (the owner's plan of 2026-10-25, §3, wording approved by the owner) [draft
//! names]: the card asks "Before I write the test for <the part>, here's what it will check:", then
//! one line per planned check (its short line, else its given), then the found checks numbered, or
//! "Nothing else seemed worth checking." or "I couldn't finish looking for more checks."; its
//! options follow the finding: "Add all three (Recommended)" / "Just the first two" on found cases,
//! "Looks right (Recommended)" / "Look again" on none, "Test these" / "Look again" with none marked
//! Recommended on a failed finder, and after the last look "Add my own" in place of "Look again"
//! (the owner's ruling, 2026-10-25: "Keep, or add my own") [draft words]. The person's card answers
//! its hold with those choices.

mod traced;

use familiar_ai_facts::Log;
use familiar_ai_rules::{cases, criterion, terminal, View};
use serde_json::{json, Value};
use traced::*;

/// c1 planned, its promise with a whole contract: n1 with a short line, o1 without one.
fn whole() -> Log {
    raw(
        &planned(),
        "promise",
        json!({ "promise_id": "p1", "behavior_id": "b1", "accepts": "rows", "gives_back": "each row once", "refuses": "nothing", "contract_version": 1, "cases": [
            { "case_id": "n1", "kind": "normal", "short": "a row is written once", "given": "given n1", "expect": "expect n1" },
            { "case_id": "o1", "kind": "opposite", "given": "given o1", "expect": "expect o1" }
        ] }),
    )
}

// Definition change (the owner's plan of 2026-10-26, the lens panel, approved): the synthesis is
// recorded only after the five lenses answered, so a finding comes after the panel's lenses.
fn found(log: &Log, outcome: &str, n: usize) -> Log {
    let log = &lenses(log, "c1");
    let cs: Vec<Value> = (1..=n)
        .map(|i| json!({ "case_id": format!("a{i}"), "short": format!("found case {i}"), "given": format!("given a{i}"), "expect": format!("expect a{i}"), "why": "the code branches here" }))
        .collect();
    raw(log, "cases_found", json!({ "criterion_id": "c1", "outcome": outcome, "cases": cs, "why": if n == 0 { "the planned cases cover it" } else { "" }, "model": "claude-sonnet-5-5", "tokens": 4000 }))
}

fn card(log: &Log) -> terminal::Card {
    terminal::cards(&View::new(log)).into_iter().find(|c| c.hold == cases::CARD).unwrap_or_else(|| panic!("the cases card"))
}

fn labels(log: &Log) -> Vec<(String, String)> {
    card(log).choices
}

fn options(log: &Log) -> Vec<(&'static str, String, String)> {
    terminal::cases_options(&View::new(log), "c1")
}

#[test]
fn the_card_asks_before_the_test_and_shows_the_planned_checks_then_the_found_ones() {
    let log = found(&whole(), "found", 3);
    let c = card(&log);
    assert_eq!(c.question, "Before I write the test for criterion c1, here's what it will check:");
    assert_eq!(c.summary, ["✓ a row is written once", "✓ given o1", "Also worth checking:", "1. found case 1", "2. found case 2", "3. found case 3"]);
    for l in &c.summary {
        assert!(c.text.contains(l.as_str()), "{l} in {}", c.text);
    }
}

/// "Keep what fits" (the owner's ruling, 2026-10-25) [draft words]: a found finding's why, when not
/// empty, is the binary's note of what it left out of the finder's answer, and the card shows it on
/// one line after the found checks; a blank why shows nothing, and a none or failed finding's why
/// is never a line of the card (the tests below).
#[test]
fn a_found_findings_note_is_shown_after_the_found_checks() {
    let note = "the finder listed 9, the first 8 are kept: 1 left out";
    let eight: Vec<Value> = (1..=8).map(|i| json!({ "case_id": format!("a{i}"), "short": format!("found case {i}"), "given": "g", "expect": "e", "why": "w" })).collect();
    // definition change (the lens panel, approved 2026-10-26): the synthesis after the five lenses
    let with = |why: &str| raw(&lenses(&whole(), "c1"), "cases_found", json!({ "criterion_id": "c1", "outcome": "found", "cases": eight, "why": why, "model": "claude-sonnet-5-5", "tokens": 4000 }));
    let mut want: Vec<String> = ["✓ a row is written once", "✓ given o1", "Also worth checking:"].map(String::from).to_vec();
    want.extend((1..=8).map(|i| format!("{i}. found case {i}")));
    let c = card(&with(" the finder listed 9,\n the first 8 are kept:  1 left out "));
    assert_eq!(c.summary, [want.clone(), vec![note.to_string()]].concat());
    assert!(c.text.contains(&format!("\n  {note}\n")), "{}", c.text);
    assert_eq!(options(&with(note))[0], (cases::ADD_ALL, "Add all eight (Recommended)".to_string(), "the test checks all ten".to_string()));
    assert_eq!(card(&with("  ")).summary, want, "a blank why shows nothing");
}

#[test]
fn found_cases_offer_add_all_recommended_and_just_the_planned_ones() {
    let log = found(&whole(), "found", 3);
    assert_eq!(
        options(&log),
        [
            (cases::ADD_ALL, "Add all three (Recommended)".to_string(), "the test checks all five".to_string()),
            (cases::KEEP, "Just the first two".to_string(), "the test checks the two above".to_string()),
        ]
    );
    assert_eq!(labels(&log), [(cases::ADD_ALL.to_string(), "Add all three (Recommended)".to_string()), (cases::KEEP.to_string(), "Just the first two".to_string())]);
    // [draft words] one found case: "Add it"; two: "Add both"
    assert_eq!(options(&found(&whole(), "found", 1))[0].1, "Add it (Recommended)");
    assert_eq!(options(&found(&whole(), "found", 1))[0].2, "the test checks all three");
    assert_eq!(options(&found(&whole(), "found", 2))[0].1, "Add both (Recommended)");
}

#[test]
fn none_found_says_so_and_recommends_looks_right() {
    let log = found(&whole(), "none", 0);
    let c = card(&log);
    assert_eq!(c.summary, ["✓ a row is written once", "✓ given o1", "Nothing else seemed worth checking."]);
    assert_eq!(
        options(&log),
        [(cases::KEEP, "Looks right (Recommended)".to_string(), "I'll write the test".to_string()), (cases::AGAIN, "Look again".to_string(), "one more search".to_string())]
    );
    // after the second finder, look again is no longer offered: keep, or add the person's own (the
    // owner's ruling, 2026-10-25: "Keep, or add my own") [draft words]
    let log = raw(&log, "hold_answered", json!({ "criterion_id": "c1", "hold": cases::CARD, "choice": cases::AGAIN, "typed": "" }));
    let last = found(&log, "none", 0);
    assert_eq!(
        options(&last),
        [(cases::KEEP, "Looks right (Recommended)".to_string(), "I'll write the test".to_string()), (cases::TYPED, "Add my own".to_string(), "pick Other and describe a case to add".to_string())]
    );
    assert_eq!(labels(&last), [(cases::KEEP.to_string(), "Looks right (Recommended)".to_string()), (cases::TYPED.to_string(), "Add my own".to_string())]);
    let failed = found(&log, "failed", 0);
    assert_eq!(
        options(&failed),
        [(cases::KEEP, "Test these".to_string(), "I'll write the test".to_string()), (cases::TYPED, "Add my own".to_string(), "pick Other and describe a case to add".to_string())]
    );
}

#[test]
fn a_failed_finder_says_so_and_recommends_nothing() {
    let log = found(&whole(), "failed", 0);
    let c = card(&log);
    assert_eq!(c.summary, ["✓ a row is written once", "✓ given o1", "I couldn't finish looking for more checks."]);
    let o = options(&log);
    assert_eq!(o, [(cases::KEEP, "Test these".to_string(), "I'll write the test".to_string()), (cases::AGAIN, "Look again".to_string(), "one more search".to_string())]);
    assert!(o.iter().all(|(_, l, _)| !l.contains("Recommended")));
}

#[test]
fn the_cases_card_is_answered_with_its_choices() {
    assert_eq!(criterion::answers_of(cases::CARD), Some(&[cases::ADD_ALL, cases::KEEP, cases::AGAIN][..]));
    assert_eq!(criterion::ANSWERS.len(), 18);
}
